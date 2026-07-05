use super::user_interaction;
use crate::util::types::ZpZrOpts;
use crate::util::{file_copier, zfs_info};
use anyhow::{Context, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use std::os::unix::fs::MetadataExt;
use std::process::Command;
use std::{fs, io};

pub const DIFF: &str = "/usr/bin/diff";

#[derive(Clone, Debug)]
pub struct Candidate {
    pub snapname: String,
    pub path: Utf8PathBuf,
    pub size: u64,
    pub mtime: i64, // epoch seconds
}

impl Candidate {
    pub fn from(path: &Utf8Path, snapdir: &Utf8Path) -> anyhow::Result<Self> {
        let metadata =
            fs::metadata(path).with_context(|| format!("cannot get metadata for {path}"))?;

        Ok(Self {
            snapname: snapdir
                .file_name()
                .with_context(|| format!("cannot get filename of {snapdir}"))?
                .to_string(),
            path: path.to_owned(),
            mtime: metadata.mtime(),
            size: metadata.size(),
        })
    }
}

#[derive(Debug)]
pub struct CopyAction {
    src: Utf8PathBuf,
    dest: Utf8PathBuf,
}

pub fn run(files: Vec<Utf8PathBuf>, auto: bool, opts: &ZpZrOpts) -> anyhow::Result<()> {
    for file in files {
        restore_file(&file.canonicalize_utf8()?, auto, opts)?;
    }

    Ok(())
}

fn restore_file(file: &Utf8Path, auto: bool, opts: &ZpZrOpts) -> anyhow::Result<()> {
    match restore_action(file, auto, opts)? {
        Some(action) => file_copier::copy_file(&action.src, &action.dest, opts),
        None => Ok(()),
    }
}

fn restore_action(
    file: &Utf8Path,
    auto: bool,
    opts: &ZpZrOpts,
) -> anyhow::Result<Option<CopyAction>> {
    // file may well not exist, so let's assume user error if its PARENT isn't there
    let parent = file.parent().unwrap();
    let target_dir = parent.canonicalize_utf8()?;
    let fs_root = zfs_info::dataset_root(&target_dir)?;
    let mut candidates = candidates(&fs_root, file)?;

    if candidates.is_empty() {
        println!("No matches found.");
        return Ok(None);
    }

    candidates.sort_by_key(|c| std::cmp::Reverse(c.mtime));

    let original_file = original_details(file)?;

    let choice_tuple = if auto {
        Some((0_usize, None))
    } else {
        user_interaction::print_options(original_file, &candidates);
        let user_input = user_interaction::get_choice()?;
        user_interaction::parse_choice(&user_input)
    };

    if choice_tuple.is_none() {
        return Ok(None);
    }

    let (candidate_index, command_option) = choice_tuple.unwrap();

    let candidate_object = match candidates.get(candidate_index) {
        Some(obj) => obj,
        None => {
            tracing::error!("cannot find requested item");
            return Ok(None);
        }
    };

    if let Some(command) = command_option {
        match command.as_str() {
            "k" => backup_target(file, opts.noop)?,
            "d" => {
                diff_files(&candidate_object.path, file);
                return Ok(None);
            }
            &_ => (),
        }
    };

    Ok(Some(CopyAction {
        src: candidate_object.path.clone(),
        dest: file.to_path_buf(),
    }))
}

fn all_snapshot_dirs(dataset_root: &Utf8Path) -> Option<Vec<Utf8PathBuf>> {
    let snapshot_root = dataset_root.join(".zfs").join("snapshot");

    if snapshot_root.exists() {
        snapshot_root
            .read_dir_utf8()
            .ok()?
            .map(|entry| entry.ok().map(|f| f.path().to_owned()))
            .collect()
    } else {
        None
    }
}

fn diff_files(source_file: &Utf8Path, target_file: &Utf8Path) {
    let mut cmd = Command::new(DIFF);
    cmd.arg(source_file).arg(target_file);
    match cmd.output() {
        Ok(out) => println!("{}", String::from_utf8_lossy(&out.stdout)),
        Err(e) => {
            eprintln!("Failed to run `/bin/diff {source_file}, {target_file}`: {e}");
            std::process::exit(3);
        }
    }
}

fn backup_target(src: &Utf8Path, noop: bool) -> anyhow::Result<()> {
    let dest = src.with_extension("backup");
    tracing::info!("{src} -> {dest}");

    ensure!(!dest.exists(), "Backup target exists: {dest}");

    if !noop {
        fs::rename(src, dest)?;
    }

    Ok(())
}

fn candidates(fs_root: &Utf8Path, file: &Utf8Path) -> anyhow::Result<Vec<Candidate>> {
    let snapshot_dirs = all_snapshot_dirs(fs_root).context("no snapshots under {fs_root}")?;

    tracing::info!("Found {} snapshots.", snapshot_dirs.len());

    let relative_path = path_relative_to_fs_root(file, fs_root)
        .context("Failed to calculate path for {file} relative to {fs_root}")?;

    Ok(snapshot_dirs
        .iter()
        .filter_map(|snapdir| {
            let path = snapdir.join(&relative_path);
            if path.exists() {
                tracing::debug!("found candidate: {path}: ");
                Candidate::from(&path, snapdir).ok()
            } else {
                tracing::debug!("no candidate at {path}");
                None
            }
        })
        .collect())
}

fn original_details(file: &Utf8Path) -> io::Result<Option<Candidate>> {
    let ret = if file.exists() {
        let metadata = fs::metadata(file)?;

        Some(Candidate {
            snapname: ".".to_string(),
            path: file.to_owned(),
            mtime: metadata.mtime(),
            size: metadata.size(),
        })
    } else {
        None
    };

    Ok(ret)
}

fn path_relative_to_fs_root(file: &Utf8Path, fs_root: &Utf8Path) -> Option<Utf8PathBuf> {
    file.strip_prefix(fs_root).ok().map(|p| p.to_owned())
}

// // We need to canonicalize the source file, whether it exists or not.
// fn canonical_file(file: Utf8PathBuf) -> anyhow::Result<Utf8PathBuf> {
//     if file.is_absolute() {
//         return Ok(file.canonicalize_utf8()?);
//     }

//     let pwd = match Utf8PathBuf::from_path_buf(env::current_dir()?) {
//         Ok(path) => path.canonicalize_utf8()?,
//         Err(_) => bail!("Failed to ascertain pwd"),
//     };

//     Ok(pwd.join(file))
// }

#[cfg(test)]
mod test {
    use super::*;
    use camino_tempfile::tempdir;
    use snltest::fixture;
    use std::fs;

    #[cfg(target_os = "illumos")]
    #[test]
    fn test_all_snapshot_dirs() {
        let result = all_snapshot_dirs(&fixture!("restore")).unwrap();
        assert!(!result.is_empty());
        assert_eq!(None, all_snapshot_dirs(&Utf8PathBuf::from("/tmp")));
    }

    #[test]
    fn test_path_relative_to_fs_root() {
        assert_eq!(
            Utf8PathBuf::from("d/e/f"),
            path_relative_to_fs_root(
                &Utf8PathBuf::from("/a/b/c/d/e/f"),
                &Utf8PathBuf::from("/a/b/c")
            )
            .unwrap()
        );

        assert_eq!(
            None,
            path_relative_to_fs_root(
                &Utf8PathBuf::from("/a/b/c/d/e/f"),
                &Utf8PathBuf::from("/g/h/i")
            )
        );
    }

    #[test]
    fn test_candidates() {
        let mut expected = vec![
            fixture!("restore/.zfs/snapshot/monday/file_in_both"),
            fixture!("restore/.zfs/snapshot/tuesday/file_in_both"),
        ];

        let mut actual: Vec<Utf8PathBuf> =
            candidates(&fixture!("restore"), &fixture!("restore/file_in_both"))
                .unwrap()
                .into_iter()
                .map(|c| c.path)
                .collect();

        expected.sort();
        actual.sort();

        assert_eq!(expected, actual);

        assert_eq!(
            vec![fixture!("restore/.zfs/snapshot/monday/file_in_monday"),],
            candidates(&fixture!("restore"), &fixture!("restore/file_in_monday"),)
                .unwrap()
                .into_iter()
                .map(|c| c.path)
                .collect::<Vec<Utf8PathBuf>>()
        );

        assert!(
            candidates(&fixture!("restore"), &fixture!("restore/file_in_neither"),)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn test_restore_action_auto_mode() {
        // let temp_dir = tempdir().unwrap();
        // let file_path = temp_dir.path().join("test_file.txt");
        // fs::write(&file_path, "test content").unwrap();

        let result = restore_action(&file_path, true, &ZpZrOpts::default());

        dbg!(result);
        //     .unwrap()
        //     .unwrap();

        // assert_eq!(result.src, file_path);
        // assert_eq!(result.dest, file_path);
    }

    // #[test]
    // fn test_restore_action_no_candidates() {
    //     let temp_dir = tempdir().unwrap();
    //     let file_path = temp_dir.path().join("nonexistent_file.txt");
    //     let result = restore_action(&file_path, false, &ZpZrOpts::default()).unwrap();

    //     assert!(result.is_none());
    // }
}
