//! Functions, constants, types, and whatever else comes along, which are required by
//! more than one of the tools in this crate.
//!
use crate::util::types::Mount;
use crate::util::zfs_info::dataset_root;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashSet;

/// Given a path and a list of ZFS mounts, works out which, if any, filesystem owns the path.
///
pub fn file_to_dataset(file: &Utf8Path, mounts: &[Mount]) -> Option<String> {
    file.ancestors().find_map(|f| {
        mounts.iter().find_map(|m| {
            if f.starts_with(&m.mountpoint) {
                Some(m.device.clone())
            } else {
                None
            }
        })
    })
}

pub fn files_to_datasets(file_list: &[String], zfs_mounts: &[Mount]) -> Vec<String> {
    let filesystems: HashSet<_> = file_list
        .iter()
        .filter_map(|f| file_to_dataset(&Utf8PathBuf::from(f), zfs_mounts))
        .collect();

    filesystems.into_iter().collect()
}

pub fn snapshot_dir_from_file(file: &Utf8Path) -> Option<Utf8PathBuf> {
    let snapdir = dataset_root(file).ok()?.join(".zfs").join("snapshot");
    snapdir.exists().then_some(snapdir)
}

#[cfg(test)]
mod test {
    use super::*;

    // You'll have to trust that these tests pass on my illumos box. They're skipped in Github
    // Actions.
    #[cfg(target_os = "illumos")]
    #[test]
    fn test_snapshot_dir() {
        assert_eq!(
            Some("/.zfs/snapshot".into()),
            snapshot_dir_from_file("/etc/passwd".into())
        );

        assert_eq!(None, snapshot_dir_from_file("/tmp".into()));

        assert_eq!(
            Some("/build/.zfs/snapshot".into()),
            snapshot_dir_from_file("/build/omnios-extra/build/".into())
        );
    }

    #[test]
    fn test_file_to_dataset() {
        let mounts = vec![
            Mount {
                mountpoint: "/zones/serv-build".into(),
                device: "rpool/zones/serv-build".into(),
            },
            Mount {
                mountpoint: "/build/configs".into(),
                device: "fast/zone/build/config".into(),
            },
            Mount {
                mountpoint: "/build".into(),
                device: "fast/zone/build/build".into(),
            },
            Mount {
                mountpoint: "/rpool".into(),
                device: "rpool".into(),
            },
            Mount {
                mountpoint: "/zones".into(),
                device: "rpool/zones".into(),
            },
        ];

        assert_eq!(None, file_to_dataset("/etc/passwd".into(), &mounts));

        assert_eq!(
            Some("fast/zone/build/build".into()),
            file_to_dataset("/build/file".into(), &mounts)
        );

        assert_eq!(
            Some("fast/zone/build/config".into()),
            file_to_dataset("/build/configs/file".into(), &mounts)
        );
    }

    #[test]
    fn test_files_to_datasets() {
        let arg_list = &[
            "/build/f1".to_string(),
            "/build/f2".to_string(),
            "/rpool/f3".to_string(),
        ];

        let mount_list = vec![
            Mount {
                mountpoint: "/build".into(),
                device: "fast/zone/build/build".into(),
            },
            Mount {
                mountpoint: "/build/configs".into(),
                device: "fast/zone/build/config".into(),
            },
            Mount {
                mountpoint: "/rpool".into(),
                device: "rpool".into(),
            },
        ];

        let mut expected = vec![
            Utf8PathBuf::from("fast/zone/build/build"),
            Utf8PathBuf::from("rpool"),
        ];
        let mut actual = files_to_datasets(arg_list, &mount_list);

        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);
        assert!(files_to_datasets(&["/where/is/this".into()], &mount_list).is_empty());
    }
}
