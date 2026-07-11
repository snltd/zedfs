use std::process::{Command, Output};

pub fn cmd_to_string(cmd: &Command) -> String {
    let program = cmd.get_program().to_string_lossy();
    let args = cmd
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    format!("{program} {args}")
}

pub fn log_error(cmd: &Command, output: Output) -> String {
    let cmd = cmd_to_string(cmd);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let exit_code = &output.status.code();
    tracing::error!(
        command = cmd,
        exit_code = exit_code,
        stdout = stdout,
        stderr = stderr
    );
    "error running external command".to_owned()
}

/// Builds a command from its args, returning a Command. Logs the constructed command
#[macro_export]
macro_rules! zfs_cmd {
    ( $( $arg:expr ),+ $(,)? ) => {{
        use std::process::{Command, Stdio};
        let mut cmd = Command::new($crate::util::constants::ZFS);
        $(
            cmd.arg($arg);
        )*
        cmd.stderr(Stdio::piped());
        tracing::debug!(command = $crate::util::macros::cmd_to_string(&cmd));
        cmd
    }};
}

/// Executes zfs(8) with the given args, returning a result of stdout
///
#[macro_export]
macro_rules! zfs_output {
    ( $( $arg:expr ),+ $(,)? ) => {{
        (|| -> anyhow::Result<String> {
            let mut cmd = $crate::zfs_cmd!( $($arg), +);
            let output = cmd.output()?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                anyhow::bail!($crate::util::macros::log_error(&cmd, output))
            }
        })()
    }};
}

/// Executes zfs(8) with the given args, returning a bool of success or failure
#[macro_export]
macro_rules! zfs_success {
    ( $docmd:expr, $( $arg:expr ),+ $(,)? ) => {{
        let mut cmd = $crate::zfs_cmd!( $($arg), +);
        match $docmd {
            $crate::util::types::Noop::True => Ok(true),
            $crate::util::types::Noop::False=> {
                let status = cmd.status()?;
                Ok::<bool, anyhow::Error>(status.success())
            }
        }
    }};
}
