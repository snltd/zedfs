use crate::util::types::Noop;
use crate::util::{rules, zfs_file, zfs_info};
use crate::{zfs_cmd, zfs_success};
use anyhow::Context;
use clap::ValueEnum;
use jiff::{Unit, Zoned};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum SnapType {
    Day,
    Month,
    Date,
    Time,
    Now,
}

pub struct SnapOpts {
    pub snap_type: SnapType,
    pub files: bool,
    pub noop: Noop,
    pub recurse: bool,
    pub omit: Option<Vec<String>>,
}

pub fn run(targets: Option<Vec<String>>, opts: &SnapOpts) -> anyhow::Result<bool> {
    let mut dataset_list = if opts.files {
        // Given a list of files: map them to parent datasets. Clap has checked we've got args.
        zfs_file::files_to_datasets(
            &targets.context("no files given")?,
            &zfs_info::get_mounted_filesystems()?,
        )
    } else if opts.recurse {
        // Given a list of datasets which we must recurse down.
        // Clap ensures the args are datasets.
        zfs_info::dataset_list_recursive(
            &targets.context("no filesystems given")?,
            &zfs_info::all_filesystems().context("no ZFS filesystems found")?,
        )
    } else if let Some(targets) = targets {
        // Given a list of datasets
        targets
    } else {
        // Not given any args, so snapshot everything
        zfs_info::all_filesystems().context("no ZFS filesystems found")?
    };

    if let Some(omit_rules) = &opts.omit {
        dataset_list = omit_filesystems(&dataset_list, omit_rules);
    }

    if dataset_list.is_empty() {
        println!("Nothing to snapshot.");
    } else {
        let now = Zoned::now().round(Unit::Second)?;
        let snapname = snapname(opts.snap_type, &now);
        do_the_snapshotting(&dataset_list, &snapname, opts.noop)?;
    }

    Ok(true)
}

fn snapname(snap_type: SnapType, ts: &Zoned) -> String {
    let formatted = match snap_type {
        SnapType::Date => ts.strftime("%F"),
        SnapType::Month => ts.strftime("%B"),
        SnapType::Day => ts.strftime("%A"),
        SnapType::Now => ts.strftime("%F_%H:%M"),
        SnapType::Time => ts.strftime("%H:%M"),
    };

    formatted.to_string().to_lowercase()
}

fn snapshot_exists(snapshot: &str) -> anyhow::Result<bool> {
    zfs_success!(Noop::False, "list", snapshot)
}

fn destroy_snapshot(snapshot: &str, noop: Noop) -> anyhow::Result<()> {
    tracing::info!("removing old {}", &snapshot);
    let mut cmd = zfs_cmd!("destroy", snapshot);

    if noop == Noop::False {
        cmd.status()?;
    }

    Ok(())
}

fn take_snapshot(snapshot: &str, noop: Noop) -> anyhow::Result<()> {
    tracing::info!("snapshotting {}", &snapshot);
    let mut cmd = zfs_cmd!("snapshot", snapshot);

    if noop == Noop::False {
        cmd.status()?;
    }

    Ok(())
}

fn do_the_snapshotting(datasets: &[String], snapname: &str, noop: Noop) -> anyhow::Result<()> {
    for dataset in datasets {
        let snapshot = format!("{}@{}", dataset, snapname);

        if snapshot_exists(&snapshot)? {
            destroy_snapshot(&snapshot, noop)?;
        }

        take_snapshot(&snapshot, noop)?;
    }

    Ok(())
}

fn omit_filesystems(filesystem_list: &[String], rules: &[String]) -> Vec<String> {
    filesystem_list
        .iter()
        .filter(|item| rules::omit_rules_match(item, rules))
        .map(|s| s.to_owned())
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_omit_filesystems() {
        let filesystem_list = vec![
            "build".to_string(),
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool".to_string(),
            "rpool/test".to_string(),
            "rpool/test_a".to_string(),
            "other".to_string(),
            "other/test".to_string(),
        ];

        let mut expected = vec![
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool".to_string(),
            "rpool/test_a".to_string(),
        ];

        let mut actual = omit_filesystems(
            &filesystem_list,
            &[
                "build".to_string(),
                "other".to_string(),
                "rpool/test".to_string(),
                "other/test".to_string(),
            ],
        );

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        expected = vec![
            "rpool".to_string(),
            "rpool/test".to_string(),
            "other".to_string(),
            "other/test".to_string(),
        ];

        actual = omit_filesystems(&filesystem_list, &["build*".to_string(), "*a".to_string()]);

        expected.sort();
        actual.sort();
        assert_eq!(expected, actual);

        expected = vec![
            "build".to_string(),
            "rpool".to_string(),
            "other".to_string(),
        ];

        actual = omit_filesystems(&filesystem_list, &["*test*".to_string()]);

        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_snapname() {
        let ts = jiff::civil::date(2024, 10, 27)
            .at(9, 45, 0, 0)
            .in_tz("UTC")
            .unwrap();

        assert_eq!("sunday".to_string(), snapname(SnapType::Day, &ts));
        assert_eq!("09:45".to_string(), snapname(SnapType::Time, &ts));
        assert_eq!("october".to_string(), snapname(SnapType::Month, &ts));
        assert_eq!("2024-10-27".to_string(), snapname(SnapType::Date, &ts));
        assert_eq!("2024-10-27_09:45".to_string(), snapname(SnapType::Now, &ts));
    }
}
