use crate::util::{zfs_file, zfs_info};
use anyhow::{bail, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use filetime::FileTime;
use glob::glob;
use jiff::{Timestamp, Zoned};
use std::collections::BTreeMap;
use std::fs::{File, metadata};
use std::io;
use std::time::SystemTime;

pub struct TouchFromSnapOpts {
    pub snapname: Option<String>,
    pub noop: bool,
}

type MTimeMap = BTreeMap<Utf8PathBuf, SystemTime>;

pub fn run(dirs: Vec<Utf8PathBuf>, opts: &TouchFromSnapOpts) -> anyhow::Result<()> {
    let snapname = opts
        .snapname
        .clone()
        .unwrap_or_else(|| default_snapname(Zoned::now()));

    for path in dirs {
        let path = path.canonicalize_utf8()?;

        if !path.is_dir() {
            tracing::warn!("{path} is not a directory");
            continue;
        }

        touch_directory(&path, &snapname, opts.noop)?;
    }

    Ok(())
}

fn touch_directory(dir: &Utf8Path, snapname: &str, noop: bool) -> anyhow::Result<()> {
    let snapshot_top_level = match zfs_file::snapshot_dir_from_file(dir) {
        Some(snapshot_root) => snapshot_root.join(snapname),
        None => bail!("{} does not appear to be a ZFS filesystem", dir),
    };

    ensure!(
        snapshot_top_level.exists(),
        "No readable ZFS snapshot directory. (Expected '{}')",
        snapshot_top_level
    );
    tracing::debug!(snapshot_top_level = snapshot_top_level.to_string());

    let dataset_root = zfs_info::dataset_root(dir)?;
    tracing::debug!(dataset_root = dataset_root.to_string());

    let snapshot_dir = if dir == dataset_root {
        snapshot_top_level
    } else {
        let relative_path = dir.to_string().replace(&format!("{dataset_root}/"), "");
        snapshot_top_level.join(&relative_path)
    };
    tracing::debug!(snapshot_dir = snapshot_dir.to_string());

    ensure!(
        snapshot_dir.exists(),
        "No snapshot source directory: {}",
        snapshot_dir
    );

    let live_timestamps = timestamps_for(dir);
    let snapshot_timestamps = timestamps_for(&snapshot_dir);
    let mut errs = 0;

    for (file, ts) in snapshot_timestamps {
        if let Some(live_ts) = live_timestamps.get(&file) {
            let target_file = dir.join(&file);
            if &ts != live_ts {
                tracing::info!("{target_file} -> {}", formatted_time(ts)?);

                if !noop && set_timestamp(&target_file, ts).is_err() {
                    errs += 1;
                }
            } else {
                tracing::debug!("{file} : correct");
            }
        } else {
            tracing::debug!("{file} : no source in snapshot");
        }
    }

    ensure!(errs == 0, "Failed to set times in {} files", errs);

    Ok(())
}

fn default_snapname(ts: Zoned) -> String {
    ts.yesterday()
        .expect("cannot remember yesterday")
        .strftime("%A")
        .to_string()
        .to_ascii_lowercase()
}

fn timestamps_for(dir: &Utf8Path) -> MTimeMap {
    tracing::debug!("collecting timestamps for {dir}");
    let pattern = format!("{}/**/*", dir);

    glob(&pattern)
        .expect("failed to read glob pattern")
        .filter_map(Result::ok)
        .filter_map(|path| {
            let metadata = metadata(&path).ok()?;
            let relative_path = path.strip_prefix(dir).ok()?;
            let modified_time = metadata.modified().ok()?;
            let utf8_path = Utf8PathBuf::from_path_buf(relative_path.to_path_buf()).ok()?;
            Some((utf8_path, modified_time))
        })
        .collect()
}

fn set_timestamp(file: &Utf8Path, ts: SystemTime) -> io::Result<()> {
    let mtime = FileTime::from_system_time(ts);
    File::open(file)?;
    filetime::set_file_times(file, mtime, mtime)
}

fn formatted_time(ts: SystemTime) -> anyhow::Result<String> {
    let ts = Timestamp::try_from(ts)?;
    let zoned: Zoned = ts.to_zoned(jiff::tz::TimeZone::system());
    Ok(zoned.strftime("%a, %d %b %Y %H:%M:%S %z").to_string())
}

#[cfg(test)]
mod test {
    use super::*;
    use snltest::fixture;

    #[test]
    fn test_timestamps_for() {
        let result = timestamps_for(&fixture!("touch-from-snap"));

        let mut expected_files: Vec<Utf8PathBuf> = vec![
            "dir1".into(),
            "dir1/file3".into(),
            "dir2".into(),
            "dir2/dir3".into(),
            "dir2/dir3/file5".into(),
            "dir2/file4".into(),
            "file1".into(),
            "file2".into(),
        ];

        let mut actual_files: Vec<Utf8PathBuf> = result.keys().cloned().collect();

        expected_files.sort();
        actual_files.sort();

        assert_eq!(expected_files, actual_files);
    }

    #[test]
    fn test_default_snapname() {
        let ts = jiff::civil::date(2024, 10, 27)
            .at(9, 45, 0, 0)
            .in_tz("UTC")
            .unwrap();

        assert_eq!("saturday".to_string(), default_snapname(ts));
    }
}
