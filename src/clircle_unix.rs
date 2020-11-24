use crate::Stdio;
use nix::libc;
use nix::sys::stat::{fstat, stat, FileStat};
use std::convert::TryFrom;
use std::path::Path;

/// Re-export of nix
pub use nix;

cfg_if::cfg_if! {
    if #[cfg(not(target_os = "android"))] {
        pub type DeviceType = libc::dev_t;
        pub type InodeType = libc::ino_t;
    } else {
        // This is just deduced from the libc crate source code, which is generated using bindgen.
        cfg_if::cfg_if! {
            if #[cfg(target_pointer_width = "32")] {
                pub type DeviceType = libc::c_ulonglong;
                pub type InodeType = libc::c_ulonglong;
            } else if #[cfg(target_pointer_width = "64")] {
                pub type DeviceType = libc::dev_t;
                pub type InodeType = libc::ino_t;
            } else {
                compile_error!("Unknown pointer width on android target.");
            }
        }
    }
}

/// Implementation of `Clircle` for Unix.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct UnixIdentifier {
    /// The `st_dev` of a `FileStat` (returned by the `stat` family of functions).
    pub device: DeviceType,
    /// The `st_ino` of a `FileStat` (returned by the `stat` family of functions).
    pub inode: InodeType,
}

/// Error for `TryFrom<Stdio>`
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClircleStdioError {
    /// The given variant points to a tty. Clircle doesn't hand out `Identifier`s to a tty, because
    /// if both stdin and stdout point to the same tty, no clash occurs, but a cycle will still be
    /// detected.
    IsTTY,
    /// The error returned from fstat.
    Nix(nix::Error),
}

impl TryFrom<Stdio> for UnixIdentifier {
    type Error = ClircleStdioError;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let fd = match stdio {
            Stdio::Stdin => libc::STDIN_FILENO,
            Stdio::Stdout => libc::STDOUT_FILENO,
            Stdio::Stderr => libc::STDERR_FILENO,
        };

        if nix::unistd::isatty(fd) == Ok(true) {
            Err(ClircleStdioError::IsTTY)
        } else {
            fstat(fd)
                .map(UnixIdentifier::from)
                .map_err(ClircleStdioError::Nix)
        }
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
