use crate::util::{rules, zfs_file, zfs_info};
use crate::zfs_cmd;
use anyhow::{Context, ensure};
use regex::Regex;

pub struct RemoveSnapOpts {
    pub files: bool,
    pub snaps: bool,
    pub omit_fs: Option<String>,
    pub omit_snaps: Option<String>,
    pub recurse: bool,
    pub all: bool,
    pub noop: bool,
}

pub fn run(targets: &[String], opts: &RemoveSnapOpts) -> anyhow::Result<()> {
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

    if let Some(omit_snaps) = &opts.omit_snaps {
        snapshot_list = filter_by_snap_name(&snapshot_list, omit_snaps);
    }

    if let Some(omit_fs) = &opts.omit_fs {
        snapshot_list = filter_by_fs_name(&snapshot_list, omit_fs);
    }

    if snapshot_list.is_empty() {
        println!("No snapshots to remove.");
    }

    remove_snaps(snapshot_list, opts.noop)?;
    Ok(())
}

// // Not to be confused with snapshot_list_from_dataset_names(), which only expects
// // the last segment of the name. This uses the whole path.
fn snapshot_list_from_dataset_paths(paths: &[String]) -> anyhow::Result<Vec<String>> {
    Ok(zfs_info::all_snapshots()?
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
fn remove_snaps(list: Vec<String>, noop: bool) -> anyhow::Result<()> {
    for snap in list {
        // Double check that we aren't going to remove a dataset
        ensure!(snap.contains("@"), "refusing to remove {snap}");

        let mut cmd = zfs_cmd!("destroy", &snap);
        tracing::info!("removing {snap}");

        if !noop {
            cmd.status()?;
        }
    }

    Ok(())
}

fn filter_list(snapshot_list: &[String], omit_rules: &str, is_snapshot: bool) -> Vec<String> {
    let rules: Vec<_> = omit_rules.split(',').map(|s| s.to_string()).collect();

    snapshot_list
        .iter()
        .filter(|f| {
            if let Some((fs_name, snap_name)) = f.split_once("@") {
                let item = if is_snapshot { snap_name } else { fs_name };
                rules::omit_rules_match(item, &rules)
            } else {
                false
            }
        })
        .map(|s| s.to_string())
        .collect()
}

fn filter_by_snap_name(snapshot_list: &[String], omit_rules: &str) -> Vec<String> {
    filter_list(snapshot_list, omit_rules, true)
}

fn filter_by_fs_name(snapshot_list: &[String], omit_rules: &str) -> Vec<String> {
    filter_list(snapshot_list, omit_rules, false)
}

// // All snapshots whose dataset name (final part) is one of those given.
fn snapshot_list_from_dataset_names(dataset_list: &[String]) -> anyhow::Result<Vec<String>> {
    let patterns: Vec<Regex> = dataset_list
        .iter()
        .map(|dataset| {
            Regex::new(&format!(r"/{}@", regex::escape(dataset)))
                .with_context(|| format!("invalid regex for dataset {dataset:?}"))
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
    Ok(zfs_info::all_snapshots()?
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

        assert_eq!(expected_1, filter_by_snap_name(&input, "snap*,other"));

        let expected_2 = vec![
            "rpool/test@snap2".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected_2, filter_by_snap_name(&input, "*1"));

        let expected_3 = vec![
            "rpool/test@snap1".to_string(),
            "rpool/test@snap2".to_string(),
            "rpool/test@mysnap1".to_string(),
        ];

        assert_eq!(expected_3, filter_by_snap_name(&input, "*t*"));
        assert_eq!(input, filter_by_snap_name(&input, "nothing,matches,*this"));
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

        assert_eq!(expected_1, filter_by_fs_name(&input, "test/*"));

        let expected_2 = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected_2, filter_by_fs_name(&input, "*1"));

        let expected_3 = vec![
            "rpool/test2@snap2".to_string(),
            "test/data@snap".to_string(),
            "rpool/test@other".to_string(),
        ];

        assert_eq!(expected_3, filter_by_fs_name(&input, "*test1,test2"));
        let expected_4: Vec<String> = Vec::new();
        assert_eq!(expected_4, filter_by_fs_name(&input, "*t*"));
        assert_eq!(input, filter_by_fs_name(&input, "snap"));
    }
}
