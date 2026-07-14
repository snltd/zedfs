use crate::util::types::Noop;
use crate::util::{zfs_file, zfs_info};
use crate::zfs_cmd;
use anyhow::Context;
use clap::ValueEnum;
use regex::Regex;
use std::collections::BTreeSet;

pub struct RemoveSnapOpts {
    pub omit_fs: Option<Vec<String>>,
    pub omit_snap: Option<Vec<String>>,
    pub recurse: bool,
    pub target_type: TargetType,
    pub noop: Noop,
}

enum FilterType {
    FilesystemName,
    SnapshotName,
}

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum TargetType {
    FsName,
    SnapName,
    FileName,
    AllSnaps,
}

type Snapshot = (String, String);

pub fn run(targets: &[String], opts: &RemoveSnapOpts) -> anyhow::Result<bool> {
    let all_snaps =
        all_snaps(&zfs_info::all_snapshots().context("cannot get a list of snapshots")?);

    let snapshot_list =
        snapshot_list(targets, opts, all_snaps).context("failed to generate snapshot list")?;

    if snapshot_list.is_empty() {
        println!("No snapshots to remove.");
    } else {
        remove_snaps(snapshot_list, opts.noop)?;
    }

    Ok(true)
}

fn snapshot_list(
    targets: &[String],
    opts: &RemoveSnapOpts,
    all_snaps: BTreeSet<Snapshot>,
) -> anyhow::Result<BTreeSet<Snapshot>> {
    let all_for_recurse = if opts.recurse {
        Some(all_snaps.clone())
    } else {
        None
    };

    let mut snapshot_list = match opts.target_type {
        TargetType::SnapName => {
            make_snaplist(all_snaps, &matchlist(targets)?, FilterType::SnapshotName)
        }
        TargetType::FsName => {
            make_snaplist(all_snaps, &matchlist(targets)?, FilterType::FilesystemName)
        }
        TargetType::FileName => {
            let fses = zfs_file::files_to_datasets(targets, &zfs_info::get_mounted_filesystems()?);
            make_snaplist(all_snaps, &matchlist(&fses)?, FilterType::FilesystemName)
        }
        TargetType::AllSnaps => all_snaps,
    };

    if let Some(all) = all_for_recurse {
        snapshot_list = recurse(all, snapshot_list);
    }

    if let Some(rules) = &opts.omit_snap {
        snapshot_list =
            filter_snaplist(snapshot_list, &matchlist(rules)?, FilterType::SnapshotName);
    }

    if let Some(rules) = &opts.omit_fs {
        snapshot_list = filter_snaplist(
            snapshot_list,
            &matchlist(rules)?,
            FilterType::FilesystemName,
        );
    }

    Ok(snapshot_list)
}

fn all_snaps(raw: &[String]) -> BTreeSet<Snapshot> {
    raw.iter()
        .filter_map(|d| {
            d.split_once('@')
                .map(|(fs, snap)| (fs.to_owned(), snap.to_owned()))
        })
        .collect()
}

fn matchlist(targets: &[String]) -> anyhow::Result<Vec<Regex>> {
    targets
        .iter()
        .map(|t| Regex::new(t).with_context(|| format!("could not create regex from {t}")))
        .collect()
}

/// Returns all snapshots whose snapshot name matches anything in `required`.
fn make_snaplist(
    all: BTreeSet<Snapshot>,
    required: &[Regex],
    ft: FilterType,
) -> BTreeSet<Snapshot> {
    all.into_iter()
        .filter(|(fs, snap)| {
            required.iter().any(|r| {
                r.is_match(match ft {
                    FilterType::FilesystemName => fs,
                    FilterType::SnapshotName => snap,
                })
            })
        })
        .collect()
}

fn filter_snaplist(
    all: BTreeSet<Snapshot>,
    required: &[Regex],
    ft: FilterType,
) -> BTreeSet<Snapshot> {
    all.into_iter()
        .filter(|(fs, snap)| {
            required.iter().all(|r| {
                !r.is_match(match ft {
                    FilterType::FilesystemName => fs,
                    FilterType::SnapshotName => snap,
                })
            })
        })
        .collect()
}

fn recurse(all: BTreeSet<Snapshot>, snaplist: BTreeSet<Snapshot>) -> BTreeSet<Snapshot> {
    let mut ret = BTreeSet::new();
    let filesystems: BTreeSet<_> = snaplist.into_iter().map(|(fs, _snap)| fs).collect();

    for fs in filesystems {
        let mut x: BTreeSet<_> = all
            .iter()
            .filter(|(allfs, _allsnap)| allfs.starts_with(&fs))
            .map(|s| s.to_owned())
            .collect();

        ret.append(&mut x);
    }

    ret
}

// If any removal fails, fail the whole lot.
fn remove_snaps(list: BTreeSet<Snapshot>, noop: Noop) -> anyhow::Result<()> {
    for (fs, snap) in list {
        let snap = format!("{fs}@{snap}");

        let mut cmd = zfs_cmd!("destroy", &snap);
        tracing::info!("removing {snap}");

        if noop == Noop::True {
            println!("would remove {snap}");
        } else {
            cmd.status()?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    fn all_test_snaps() -> BTreeSet<Snapshot> {
        let raw = indoc::indoc! { r#"
            big@monday
            big@tuesday
            big/work@monday
            big/work@tuesday
            big/work/src@monday
            big/work/src@tuesday
            rpool@january
            rpool/var@february
            "#};

        let lines: Vec<_> = raw.lines().map(|s| s.to_owned()).collect();
        all_snaps(lines.as_slice())
    }

    fn test_result(targets: &[String], opts: &RemoveSnapOpts) -> String {
        let set = snapshot_list(targets, opts, all_test_snaps()).unwrap();

        set.iter()
            .map(|(fs, snap)| format!("{fs}@{snap}\n"))
            .collect()
    }

    #[test]
    fn test_01() {
        let opts = &RemoveSnapOpts {
            omit_fs: None,
            omit_snap: None,
            recurse: false,
            target_type: TargetType::SnapName,
            noop: Noop::False,
        };

        let expected = indoc::indoc! { r#"
                big@monday
                big@tuesday
                big/work@monday
                big/work@tuesday
                big/work/src@monday
                big/work/src@tuesday
            "#};

        let targets = [".*day".to_owned()];
        assert_eq!(test_result(&targets, opts), expected);

        let targets = ["monday".to_owned(), "tuesday".to_owned()];
        assert_eq!(test_result(&targets, opts), expected);

        let targets = ["big".to_owned()];
        let opts = &RemoveSnapOpts {
            omit_fs: None,
            omit_snap: None,
            recurse: true,
            target_type: TargetType::FsName,
            noop: Noop::False,
        };
        assert_eq!(test_result(&targets, opts), expected);

        let targets = [".*[ra]y$".to_owned()];
        let opts = &RemoveSnapOpts {
            omit_fs: Some(vec!["rpool/var".to_owned()]),
            omit_snap: Some(vec!["january".to_owned()]),
            recurse: false,
            target_type: TargetType::SnapName,
            noop: Noop::False,
        };
        assert_eq!(test_result(&targets, opts), expected);
    }

    #[test]
    fn test_02() {
        let targets = [".*/work.*".to_owned()];
        let opts = &RemoveSnapOpts {
            omit_fs: Some(vec![".*src".to_owned()]),
            omit_snap: Some(vec!["monday".to_owned(), "wednesday".to_owned()]),
            recurse: false,
            target_type: TargetType::FsName,
            noop: Noop::False,
        };
        assert_eq!(test_result(&targets, opts), "big/work@tuesday\n".to_owned());
    }
}
