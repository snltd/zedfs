use crate::util::types::Noop;
use crate::util::{rules, zfs_file, zfs_info};
use crate::zfs_cmd;
use anyhow::{Context, ensure};
use regex::Regex;

pub struct RemoveSnapOpts {
    pub files: bool,
    pub snaps: bool,
    pub omit_fs: Option<Vec<String>>,
    pub omit_snap: Option<Vec<String>>,
    pub recurse: bool,
    pub all: bool,
    pub noop: Noop,
}

enum FilterType {
    FilesystemName,
    SnapshotName,
}

pub fn run(targets: &[String], opts: &RemoveSnapOpts) -> anyhow::Result<bool> {
    let mut snapshot_list = if opts.snaps {
        snapshot_list_from_snap_names(targets)
    } else if opts.all {
        snapshot_list_from_dataset_names(targets)
    } else if opts.files {
        let targets = zfs_file::files_to_datasets(targets, &zfs_info::get_mounted_filesystems()?);
        snapshot_list_from_dataset_paths(&targets)
    } else if opts.recurse {
        let targets = zfs_info::dataset_list_recursive(targets, &zfs_info::all_filesystems()?);
        snapshot_list_from_dataset_paths(&targets)
    } else {
        snapshot_list_from_dataset_paths(targets)
    }?;

    if let Some(omit_rules) = &opts.omit_snap {
        snapshot_list = filter_list(&snapshot_list, omit_rules, FilterType::SnapshotName);
    }

    if let Some(omit_rules) = &opts.omit_fs {
        snapshot_list = filter_list(&snapshot_list, omit_rules, FilterType::FilesystemName);
    }

    if snapshot_list.is_empty() {
        println!("No snapshots to remove.");
    } else {
        remove_snaps(snapshot_list, opts.noop)?;
    }

    Ok(true)
}

// // Not to be confused with snapshot_list_from_dataset_names(), which only expects
// // the last segment of the name. This uses the whole path.
fn snapshot_list_from_dataset_paths(paths: &[String]) -> anyhow::Result<Vec<String>> {
    Ok(zfs_info::all_snapshots()
        .context("failed to list snapshots")?
        .iter()
        .filter_map(|line| {
            if paths
                .iter()
                .any(|dataset| line.starts_with(&format!("{}@", dataset)))
            {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect())
}

// If any removal fails, fail the whole lot.
fn remove_snaps(list: Vec<String>, noop: Noop) -> anyhow::Result<()> {
    for snap in list {
        // Double check that we aren't going to remove a dataset
        ensure!(snap.contains("@"), "refusing to remove {snap}");

        let mut cmd = zfs_cmd!("destroy", &snap);
        tracing::info!("removing {snap}");

        if noop == Noop::False {
            cmd.status()?;
        }
    }

    Ok(())
}

fn filter_list(snapshots: &[String], omit_rules: &[String], filter_on: FilterType) -> Vec<String> {
    snapshots
        .iter()
        .filter(|f| {
            if let Some((fs_name, snap_name)) = f.split_once("@") {
                let item = match filter_on {
                    FilterType::FilesystemName => fs_name,
                    FilterType::SnapshotName => snap_name,
                };
                rules::omit_rules_match(item, omit_rules)
            } else {
                false
            }
        })
        .map(|s| s.to_string())
        .collect()
}

// // All snapshots whose dataset name (final part) is one of those given.
fn snapshot_list_from_dataset_names(dataset_list: &[String]) -> anyhow::Result<Vec<String>> {
    let patterns: Vec<Regex> = dataset_list
        .iter()
        .map(|dataset| {
            Regex::new(&format!(r"/{}@", regex::escape(dataset)))
                .with_context(|| format!("invalid dataset regex: {dataset:?}"))
        })
        .collect::<anyhow::Result<Vec<Regex>>>()?;

    Ok(zfs_info::all_snapshots()?
        .iter()
        .filter_map(|line| {
            if patterns.iter().any(|pattern| pattern.is_match(line)) {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect())
}

// All snapshots with the names given in list
fn snapshot_list_from_snap_names(snaplist: &[String]) -> anyhow::Result<Vec<String>> {
    Ok(zfs_info::all_snapshots()
        .context("failed to list all snapshots")?
        .iter()
        .filter_map(|line| {
            if snaplist
                .iter()
                .any(|snap| line.ends_with(&format!("@{}", snap)))
            {
                Some(line.to_string())
            } else {
                None
            }
        })
        .collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_filter_by_snap_name() {
        let input = vec![
            "rpool/test@snap1".to_string(),
            "rpool/test@snap2".to_string(),
            "rpool/test@mysnap1".to_string(),
            "rpool/test@other".to_string(),
        ];

        let expected_1 = vec!["rpool/test@mysnap1".to_string()];

        assert_eq!(
            expected_1,
            filter_list(
                &input,
                &["snap*".to_string(), "other".to_string()],
                FilterType::SnapshotName,
            )
        );

        let expected_2 = vec![
            "rpool/test@snap2".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(
            expected_2,
            filter_list(&input, &["*1".to_string()], FilterType::SnapshotName)
        );

        let expected_3 = vec![
            "rpool/test@snap1".to_string(),
            "rpool/test@snap2".to_string(),
            "rpool/test@mysnap1".to_string(),
        ];

        assert_eq!(
            expected_3,
            filter_list(&input, &["*t*".to_string()], FilterType::SnapshotName)
        );

        assert_eq!(
            input,
            filter_list(
                &input,
                &["nothing,matches".to_string(), "*these".to_string()],
                FilterType::SnapshotName
            )
        );
    }

    #[test]
    fn test_filter_by_fs_name() {
        let input = vec![
            "rpool/test1@snap1".to_string(),
            "rpool/test2@snap2".to_string(),
            "rpool/test1@mysnap1".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        let expected_1 = vec![
            "rpool/test1@snap1".to_string(),
            "rpool/test2@snap2".to_string(),
            "rpool/test1@mysnap1".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(
            expected_1,
            filter_list(&input, &["test/*".to_string()], FilterType::FilesystemName)
        );

        let expected_2 = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(
            expected_2,
            filter_list(&input, &["*1".to_string()], FilterType::FilesystemName)
        );

        let expected_3 = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(
            expected_3,
            filter_list(
                &input,
                &["*test1".to_string(), "test2".to_string()],
                FilterType::FilesystemName
            )
        );

        let expected_4: Vec<String> = Vec::new();

        assert_eq!(
            expected_4,
            filter_list(&input, &["*t*".to_string()], FilterType::FilesystemName)
        );

        assert_eq!(
            input,
            filter_list(&input, &["snap".to_string()], FilterType::FilesystemName)
        );
    }
}
