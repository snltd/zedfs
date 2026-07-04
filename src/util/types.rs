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
