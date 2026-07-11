use camino::Utf8PathBuf;

#[derive(Clone, Default, Copy, Debug, PartialEq, Eq)]
pub enum Noop {
    #[default]
    False,
    True,
}

impl From<bool> for Noop {
    fn from(noop: bool) -> Self {
        if noop { Noop::True } else { Noop::False }
    }
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub struct Mount {
    pub mountpoint: Utf8PathBuf,
    pub device: String,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Default))]
pub struct ZpZrOpts {
    pub noop: Noop,
    pub no_clobber: bool,
}
