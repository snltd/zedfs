use crate::util::types::Noop;
use crate::{zfs_output, zfs_success};
use anyhow::Context;
use clap::ValueEnum;

#[derive(ValueEnum, Clone, Debug, PartialEq, Eq)]
pub enum PassphraseActions {
    Mount,
    Unmount,
}

pub fn run(action: PassphraseActions, noop: Noop) -> anyhow::Result<bool> {
    match action {
        PassphraseActions::Mount => mount(noop),
        PassphraseActions::Unmount => unmount(noop),
    }
}

fn mount(noop: Noop) -> anyhow::Result<bool> {
    let lines = passphrased_fses("no")?;
    let mut ret = true;
    let actions = ["load-key", "share"];

    if lines.is_empty() {
        println!("Nothing to mount");
    } else {
        for fs in lines {
            let key_status = zfs_output!("get", "-Ho", "value", "keystatus", &fs)
                .with_context(|| format!("cannot get keystatus for {fs}"))?;

            if &key_status != "available" {
                for action in actions {
                    ret = do_action(action, &fs, noop)?;
                }
            }
        }
    }

    Ok(ret)
}

fn unmount(noop: Noop) -> anyhow::Result<bool> {
    let lines = passphrased_fses("yes")?;
    let mut ret = true;
    let actions = ["unshare", "unmount", "unload-key"];

    if lines.is_empty() {
        println!("Nothing to unmount");
    } else {
        for fs in lines {
            for action in actions {
                ret = do_action(action, &fs, noop)?;
            }
        }
    }

    Ok(ret)
}

fn do_action(action: &str, fs: &str, noop: Noop) -> anyhow::Result<bool> {
    tracing::info!("{action} {fs}");

    if zfs_success!(noop, action, &fs)
        .with_context(|| format!("failed to run {action} for {fs}"))?
    {
        Ok(true)
    } else {
        tracing::error!("failed to {action} {fs}");
        Ok(false)
    }
}

fn mount_state(fs: &str) -> Option<String> {
    zfs_output!("get", "-Ho", "value", "mounted", fs).ok()
}

fn passphrased_fses(mounted: &str) -> anyhow::Result<Vec<String>> {
    Ok(zfs_output!("get", "-Ho", "name,value", "keylocation")
        .context("failed to get filesystem information")?
        .lines()
        .filter_map(|l| {
            let mut chunks = l.split_whitespace();
            let fs = chunks.next();
            let value = chunks.next();

            if let Some(fs) = fs
                && let Some(v) = value
                && v == "prompt"
                && let Some(mount_state) = mount_state(fs)
                && mount_state == mounted
            {
                Some(fs.to_owned())
            } else {
                None
            }
        })
        .collect())
}
