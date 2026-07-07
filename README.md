[![Rust Tests](https://github.com/snltd/zedfs/actions/workflows/test-rust.yml/badge.svg)](https://github.com/snltd/zedfs/actions/workflows/test-rust.yml) [![Lint Tests](https://github.com/snltd/zedfs/actions/workflows/test-lint.yml/badge.svg)](https://github.com/snltd/zedfs/actions/workflows/test-lint.yml)

# zedfs

Some tools that may be useful when working with ZFS.

## Install

Assuming you have [Rust installed](https://rustup.rs/), clone the repo and run:

```sh
$ cargo install --path .
```

## Usage

`zedfs` has a number of sub-commands, each detailed below.

By default most commands are mostly silent: use `-v` to get `INFO` level output,
and `-vv` to get `DEBUG` info.

Any commands which manipulate the filesystem take a `--noop` options.

## Sub-Commands

### completions

Generates, to stdout, shell completions.

```sh
$ zedfs completions <SHELL>
```

`SHELL` may be bash, fish, or zsh.

### promote

Promote files from a ZFS snapshot

```sh
$ zedfs promote [OPTIONS] <FILE_LIST>...
```

Promotes files from a ZFS snapshot.

`FILE_LIST` is one or more files or directories inside a `.zfs/snapshot`
directory. Directories are promoted recursively.

- `-N, --noclobber` by default, existing files are overwritten. Use this option
  to preserve them.

### real-usage

Show the actual disk space used by filesystems and snapshots.

```sh
$ zedfs real-usage [OPTIONS]
```

The way ZFS reports space can be a little confusing: `zfs-real-usage` clearly
tells you how much real disk space is occupied by your filesystems and
snapshots, sorting from the least to the most.

- `-0, --zeroes` shows empty datasets. Otherwise, these are omitted.

### remove-snaps

```sh
$ zedfs remove-snaps [OPTIONS] <TARGETS>...
```

Batch-removes ZFS snapshots.

- `-f, --files` specifies that `TARGETS` are files. The program will work out
  which filesystems contain them.
- `-s, --snaps` means that all `TARGETS` are snapshot names. So `-s monday`
  would remove all `@monday` snapshots anywhere in your hierarchy.

  If you don't supply `-f` or `-s`, then all `TARGETS` are assumed to be ZFS
  filesystem names.
- `-a, --all-datasets` tells the program to remove snapshots under all
  filesystems whose name matches any of the arguments. So `-a logs` would remove
  snaps for `rpool/logs` `rpool/application/logs` and `tank/logs`.
- `-o, --omit-fs <FILESYSTEM>` tells the program NOT to delete snapshots
  belonging to `FILESYSTEM`. `*` can be used as a wildcard at the beginning and
  end of `FILESYSTEM`. This option can be supplied multiple times.
- `-O, --omit-snap <SNAPSHOT>` tells the program NOT to delete any snapshots
  whose names match `SNAPSHOT`. It may be specified multiple times, and `*` is a
  wildcard.

  You can use `-o` and `-O` together, but you can't use them when your arguments
  are snapshots or dataset names. i.e. with `-s` or `-a`.
- `-r, --recurse` removes snapshots from any children of the datasets selected
  by normal rules. Works in conjunction with `-o` and `-O`. a

### restore

Restore files from snapshots

```sh
$ zedfs restore [OPTIONS] <FILE_LIST>...
```

For each file in `FILE_LIST`, finds versions of it in snapshots, and offers the
user a list of them with their size and time of last modification. When one is
selected, it is restored to the live filesystem. Directories are restored
recursively, and existing files are overwritten by default. The specified files
do not have to exist in the live filesystem.

- `-a, --auto` recovers the most recently modified version of the file rather
  than asking for the user's choice.

- `-N, --noclobber` does not overwrite existing live files.

### rogue-snaps

Find snapshots which don't match expected names

### snap

Bulk-creates ZFS snapshots according to a naming scheme.

```sh
$ zedfs snap --type <SNAP_TYPE> [TARGETS]...
```

- `-t, --type` specifies the format of the snapshot name. Choose from:
  - `day` lowercase week day: `wednesday`.
  - `month` lowercase month name: `august`.
  - `date` today's date: `2026-07-06`.
  - `time` current time to the minute: `13:49`.
  - `now` current time to the second: `2026-07-06_13:49:26`.
- `-f, --files` specifies that `TARGETS` are files, and snapshots their
  containing filesystems.
- `-r, --recurse` recurses down ZFS hierarchies, snapshotting everything under
  the given `TARGETS`.
- `-o, --omit <FILESYSTEM>` specifies that `FILESYSTEM` will NOT be snapshotted.
  This is applied after any recursion is calculated. You can use `*` as a
  wildcard at the start and end of the filesystem name.

Existing snapshots with the same names are removed.

### touch-from-snap

Align timestamps with those in a given snapshot

```sh
$ zedfs touch-from-snap [OPTIONS] <DIRS>...
```

Compares a live filesystem with one of its snapshots, and modifies the mtimes of
the live files, using the snapshot contents as a reference.

- `-s , --snapname <SNAPSHOT>` tells the program which snapshot to use. If you
  do not supply one, it will assume you have snapshots `monday` through
  `sunday`, and use yesterday's.

## zfs-rogue-snaps

Finds any snapshots which do not match the naming scheme used by the `snap`
command.
