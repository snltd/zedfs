mod util;

mod commands;

use crate::commands::remove_snaps::RemoveSnapOpts;
use crate::commands::snap::{SnapOpts, SnapType};
use crate::commands::touch_from_snap::TouchFromSnapOpts;
use crate::util::types::ZpZrOpts;
use camino::Utf8PathBuf;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::generate;
use clap_complete::shells::{Bash, Fish, Zsh};
use commands::passphrased::PassphraseActions;
use std::process::ExitCode;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

const MY_NAME: &str = "zedfs";

#[derive(Parser)]
#[clap(version, about = "Help with some ZFS tasks")]
struct Cli {
    /// Be verbose
    #[clap(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Generate shell completions
    Completions {
        /// Generate completion code for the given shell: bash, fish, or zsh
        #[arg(required = true)]
        shell: String,
    },
    /// Mount and share any filesystems which require a passphrase
    Passphrased {
        /// Print what would happen, without doing it
        #[clap(short, long)]
        noop: bool,
        #[arg(value_enum)]
        action: PassphraseActions,
    },
    /// Promote files from a ZFS snapshot
    Promote {
        /// By default, existing live files are overwritten. With this option, they are not
        #[clap(short = 'N', long)]
        no_clobber: bool,
        /// Print what would happen, without doing it
        #[clap(short, long)]
        noop: bool,
        /// File(s) to promote
        #[clap(required = true)]
        file_list: Vec<Utf8PathBuf>,
    },
    /// Show the actual disk space used by filesystems and snapshots
    #[command(alias = "df")]
    RealUsage {
        /// Show zero-size datasets
        #[clap(short = '0', long)]
        show_zeroes: bool,
    },
    /// Bulk-remove snapshots
    #[command(alias = "rm")]
    RemoveSnaps {
        /// Specifies that args are files: the snapshots containing these files will be destroyed
        #[clap(short, long)]
        files: bool,
        /// Specifies that all args are snapshot names
        #[clap(short = 's', long = "snaps")]
        snaps: bool,
        /// Purge ALL datasets with this name ANYWHERE in the hierarchy
        #[clap(short = 'A', long = "all-datasets")]
        all: bool,
        /// Filesystem(s) from which snapshots should NOT be removed. Accepts * as a wildcard.
        #[clap(short = 'o', long, conflicts_with = "files", conflicts_with = "snaps")]
        omit_fs: Option<Vec<String>>,
        /// Snapshot name(s) which should NOT be removed. Accepts * as a wildcard.
        #[clap(short = 'O', long, conflicts_with = "files", conflicts_with = "snaps")]
        omit_snap: Option<Vec<String>>,
        /// Recurse down dataset hierarchies
        #[clap(short, long, conflicts_with = "snaps", conflicts_with = "all")]
        recurse: bool,
        /// Print what would happen, without doing it
        #[arg(short, long)]
        noop: bool,
        /// One or more datasets, snapshots, or directory names
        #[arg(required = true)]
        targets: Vec<String>,
    },
    /// Restore files from snapshots
    Restore {
        /// Automatically recover the newest backup
        #[clap(short, long)]
        auto: bool,
        /// By default, existing live files are overwritten. With this option, they are not
        #[clap(short = 'N', long)]
        no_clobber: bool,
        /// Print what would happen, without doing it
        #[clap(short, long)]
        noop: bool,
        /// File(s) to restore
        #[clap(required = true)]
        file_list: Vec<Utf8PathBuf>,
    },
    /// Find snapshots which don't match expected names
    #[command(alias = "rogues")]
    RogueSnaps {},
    /// Take snapshost
    Snap {
        #[clap(
            short = 't',
            long = "type",
            required = true,
            value_enum,
            long_help = "Specify the type of snapshot to take: this  determines the \
        snapshot names\n  e.g  day    @wednesday\n       month  @january\n       \
        date   @2008-30-01\n       time   @08:45\n       now    @2008-30-01_08:45:00"
        )]
        snap_type: SnapType,
        /// Specifies that args are files: the filesystems containing these files will be snapshotted
        #[clap(short, long, conflicts_with = "recurse")]
        files: bool,
        /// Print what would happen, without doing it
        #[clap(short, long)]
        noop: bool,
        /// Recurse down dataset hierarchies
        #[clap(short, long)]
        recurse: bool,
        /// Filesystem(s) to NOT snapshot. Accepts * as a wildcard.
        #[clap(short, long)]
        omit: Option<Vec<String>>,
        /// Dataset or directory name. If not args are given, every dataset will be snapshotted.
        #[clap(required_if_eq_any([("files", "true"), ("recurse", "true")]))]
        targets: Option<Vec<String>>,
    },
    /// Align timestamps with those in a given snapshot
    TouchFromSnap {
        /// Use specified snapshot name, rather than yesterday's
        #[clap(short, long)]
        snapname: Option<String>,
        /// Print what would happen, without doing it        
        #[clap(short, long)]
        noop: bool,
        /// One or more directories
        #[arg(required = true)]
        dirs: Vec<Utf8PathBuf>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let level = match cli.verbose {
        0 => Level::WARN,
        1 => Level::INFO,
        2 => Level::DEBUG,
        _ => Level::TRACE,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr) // Keep stdout clean for your ZFS data!
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("error setting default subscriber");

    let result = match cli.command {
        Commands::Completions { shell } => {
            match shell.as_str() {
                "bash" => {
                    generate(Bash, &mut Cli::command(), MY_NAME, &mut std::io::stdout());
                }
                "fish" => {
                    generate(Fish, &mut Cli::command(), MY_NAME, &mut std::io::stdout());
                }
                "zsh" => {
                    generate(Zsh, &mut Cli::command(), MY_NAME, &mut std::io::stdout());
                }
                _ => {
                    eprintln!("unsupported shell");
                    return ExitCode::FAILURE;
                }
            }
            return ExitCode::SUCCESS;
        }
        Commands::Passphrased { action, noop } => commands::passphrased::run(action, noop.into()),
        Commands::Promote {
            no_clobber,
            noop,
            file_list,
        } => commands::promote::run(
            file_list,
            &ZpZrOpts {
                no_clobber,
                noop: noop.into(),
            },
        ),
        Commands::RealUsage { show_zeroes } => commands::real_usage::run(show_zeroes),
        Commands::RemoveSnaps {
            files,
            snaps,
            all,
            omit_fs,
            omit_snap,
            noop,
            recurse,
            targets,
        } => commands::remove_snaps::run(
            &targets,
            &RemoveSnapOpts {
                files,
                snaps,
                all,
                noop: noop.into(),
                omit_fs,
                omit_snap,
                recurse,
            },
        ),
        Commands::Restore {
            auto,
            no_clobber,
            noop,
            file_list,
        } => commands::restore::command::run(
            file_list,
            auto,
            &ZpZrOpts {
                no_clobber,
                noop: noop.into(),
            },
        ),
        Commands::RogueSnaps {} => commands::rogue_snaps::run(),
        Commands::Snap {
            snap_type,
            files,
            noop,
            recurse,
            omit,
            targets,
        } => commands::snap::run(
            targets,
            &SnapOpts {
                snap_type,
                files,
                noop: noop.into(),
                recurse,
                omit,
            },
        ),
        Commands::TouchFromSnap {
            snapname,
            noop,
            dirs,
        } => commands::touch_from_snap::run(
            dirs,
            &TouchFromSnapOpts {
                snapname,
                noop: noop.into(),
            },
        ),
    };

    match result {
        Ok(code) => {
            if code {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            ExitCode::FAILURE
        }
    }
}
