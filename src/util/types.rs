use camino::Utf8PathBuf;

// pub type ArgList = Vec<String>;
// pub type SnapshotList = Vec<String>;

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Mount {
    pub mountpoint: Utf8PathBuf,
    pub device: String,
}

// pub type Filesystem = String;
// pub type Snapshot = String;
// pub type MountList = Vec<(Utf8PathBuf, String)>;
// pub type Filesystems = Vec<String>;
// pub type ZfsMounts = Vec<(PathBuf, String)>;

pub struct RemoveSnapOpts {
    pub files: bool,
    pub snaps: bool,
    pub omit_fs: Option<String>,
    pub omit_snaps: Option<String>,
    pub recurse: bool,
    pub all: bool,
    pub noop: bool,
}
