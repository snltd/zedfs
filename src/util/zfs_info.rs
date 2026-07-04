use crate::util::types::Mount;
use crate::zfs_output;
use anyhow::bail;
use camino::{Utf8Path, Utf8PathBuf};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::MetadataExt;

/// Returns a Vec of all the snapshots ZFS can see, each being a string.
pub fn all_snapshots() -> anyhow::Result<Vec<String>> {
    Ok(zfs_output!("list", "-Ho", "name", "-t", "snapshot")?
        .lines()
        .map(|l| l.to_owned())
        .collect())
}

/// Returns a Vec of all the ZFS filesystems on the host, each being a string.
pub fn all_filesystems() -> anyhow::Result<Vec<String>> {
    Ok(zfs_output!("list", "-Ho", "name", "-t", "filesystem")?
        .lines()
        .map(|l| l.to_owned())
        .collect())
}

/// Returns a Vec of all mounted ZFS filesystems, described as Strings.
pub fn all_zfs_mounts() -> anyhow::Result<Vec<String>> {
    Ok(zfs_output!("list", "-Ho", "mountpoint,name")?
        .lines()
        .map(|l| l.to_owned())
        .collect())
}

/// Returns a vec of all the ZFS mounts which are not 'legacy', sorted by the
/// length of the path
pub fn mounted_filesystems(mounts: &[String]) -> anyhow::Result<Vec<Mount>> {
    let mut ret: Vec<_> = mounts
        .iter()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();

            match (parts.next(), parts.next()) {
                (Some(mountpoint), Some(device))
                    if mountpoint != "none" && mountpoint != "legacy" =>
                {
                    Some(Mount {
                        mountpoint: Utf8PathBuf::from(mountpoint),
                        device: device.to_string(),
                    })
                }
                _ => None,
            }
        })
        .collect();

    ret.sort_by_key(|m| std::cmp::Reverse(m.mountpoint.to_string().len()));
    Ok(ret)
}

pub fn get_mounted_filesystems() -> anyhow::Result<Vec<Mount>> {
    let all_mounts = all_zfs_mounts()?;
    mounted_filesystems(&all_mounts)
}

pub fn is_mountpoint(file: &Utf8Path) -> anyhow::Result<bool> {
    if file == "/" {
        Ok(true)
    } else {
        let path_metadata = fs::metadata(file)?;
        let parent_metadata = fs::metadata(file.parent().unwrap_or(file))?;
        Ok(path_metadata.dev() != parent_metadata.dev())
    }
}

pub fn dataset_root(path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    if is_mountpoint(path)? {
        Ok(path.into())
    } else if let Some(parent) = path.parent() {
        dataset_root(parent)
    } else {
        bail!("failed to find root for {path}")
    }
}

/// Given a list of ZFS filesystems and knowledge of all ZFS filesystems, returns the subset
/// of all filesystems under any of the given ones.
pub fn dataset_list_recursive(from_user: &[String], all_filesystems: &[String]) -> Vec<String> {
    let unique_datasets: HashSet<String> = from_user
        .iter()
        .flat_map(|path| {
            let formatted_path = ensure_trailing_slash(path);

            all_filesystems
                .iter()
                .filter(move |fs| fs == &path || fs.starts_with(&formatted_path))
                .map(|fs| fs.to_owned())
        })
        .collect();

    unique_datasets.into_iter().collect()
}

fn ensure_trailing_slash(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{}/", path)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_zfs_mounts() {
        let expected = vec![
            Mount {
                mountpoint: Utf8PathBuf::from("/zones/serv-build"),
                device: "rpool/zones/serv-build".to_string(),
            },
            Mount {
                mountpoint: Utf8PathBuf::from("/build/configs"),
                device: "fast/zone/build/config".to_string(),
            },
            Mount {
                mountpoint: Utf8PathBuf::from("/build"),
                device: "fast/zone/build/build".to_string(),
            },
            Mount {
                mountpoint: Utf8PathBuf::from("/rpool"),
                device: "rpool".to_string(),
            },
            Mount {
                mountpoint: Utf8PathBuf::from("/zones"),
                device: "rpool/zones".to_string(),
            },
        ];

        let raw_list: Vec<String> = fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test/resources/mountpoint_list.txt"
        ))
        .unwrap()
        .lines()
        .map(String::from)
        .collect();

        assert_eq!(expected, mounted_filesystems(&raw_list).unwrap());
    }

    #[test]
    fn test_dataset_list_recursive() {
        let arg_list = ["build".to_string(), "rpool/test".to_string()];

        let all_filesystems = [
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
            "build".to_string(),
            "build/test".to_string(),
            "build/test/a".to_string(),
            "rpool/test".to_string(),
        ];

        let mut actual = dataset_list_recursive(&arg_list, &all_filesystems);

        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);
    }
}
