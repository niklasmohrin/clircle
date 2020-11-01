use crate::Stdio;
use nix::libc;
use nix::sys::stat::{fstat, stat, FileStat};
use std::convert::TryFrom;
use std::path::Path;

/// Re-export of nix
pub use nix;

/// Implementation of `Clircle` for Unix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct UnixIdentifier {
    /// The `st_dev` of a `FileStat` (returned by the `stat` family of functions).
    pub device: libc::dev_t,
    /// The `st_ino` of a `FileStat` (returned by the `stat` family of functions).
    pub inode: libc::ino_t,
}

impl TryFrom<Stdio> for UnixIdentifier {
    type Error = nix::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let fd = match stdio {
            Stdio::Stdin => libc::STDIN_FILENO,
            Stdio::Stdout => libc::STDOUT_FILENO,
            Stdio::Stderr => libc::STDERR_FILENO,
        };
        fstat(fd).map(UnixIdentifier::from)
    }
}

impl<'a> TryFrom<&'a Path> for UnixIdentifier {
    type Error = nix::Error;

    fn try_from(path: &'a Path) -> Result<Self, Self::Error> {
        stat(path).map(UnixIdentifier::from)
    }
}

impl From<FileStat> for UnixIdentifier {
    fn from(stats: FileStat) -> Self {
        UnixIdentifier {
            device: stats.st_dev,
            inode: stats.st_ino,
        }
    }
}
