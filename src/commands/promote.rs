use crate::util::file_copier;
use crate::util::types::ZpZrOpts;
use anyhow::Context;
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use std::fs;

pub fn run(files: Vec<Utf8PathBuf>, opts: &ZpZrOpts) -> anyhow::Result<()> {
    for file in files {
        promote_file(
            &file
                .canonicalize_utf8()
                .with_context(|| format!("file does not exist: {file}"))?,
            opts,
        )?;
    }

    Ok(())
}

fn promote_file(path: &Utf8Path, opts: &ZpZrOpts) -> anyhow::Result<()> {
    let target_file =
        target_file(path).with_context(|| format!("cannot derive target for {path}"))?;

    let target_dir = target_file
        .parent()
        .with_context(|| format!("cannot get parent for of {path}"))?;

    if !target_dir.exists() {
        tracing::info!("creating {target_dir}");

        if !opts.noop {
            fs::create_dir_all(target_dir)
                .with_context(|| format!("failed to create dir: {target_dir}"))?;
        }
    }

    file_copier::copy_file(path, &target_file, opts)
}

// Error if path is not in a ZFS snapshot
fn target_file(path: &Utf8Path) -> anyhow::Result<Utf8PathBuf> {
    let components: Vec<_> = path.components().collect();

    let zfs_index = components
        .iter()
        .position(|&c| c == Utf8Component::Normal(".zfs"))
        .with_context(|| format!("no .zfs directory in path of {path}"))?;

    // remove .zfs/snapshots/<snap_name> from the path
    let target_path = components
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            if i < zfs_index || i > (zfs_index + 2) {
                Some(c)
            } else {
                None
            }
        })
        .collect();

    Ok(target_path)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_target_file() {
        assert_eq!(
            Utf8PathBuf::from("/test/dir/file"),
            target_file("/test/.zfs/snapshot/monday/dir/file".into()).unwrap()
        );

        assert_eq!(
            Utf8PathBuf::from("/test/u01/u02/mtpt/deep/dir/file"),
            target_file("/test/u01/u02/mtpt/.zfs/snapshot/test/deep/dir/file".into()).unwrap()
        );

        assert!(target_file("/etc/passwd".into()).is_err());
    }
}
