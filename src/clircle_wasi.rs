use crate::{Clircle, Stdio};

use std::convert::TryFrom;
use std::fs::File;
use std::io::{self, Seek, SeekFrom};
use std::os::wasi::fs::MetadataExt;
use std::os::wasi::io::{FromRawFd, IntoRawFd, RawFd};
use std::{cmp, hash, ops};

/// Re-export of libc
pub use libc;

/// Implementation of `Clircle` for WASI.
#[derive(Debug)]
pub struct WasiIdentifier {
    device: u64,
    inode: u64,
    size: u64,
    is_regular_file: bool,
    file: Option<File>,
    owns_fd: bool,
}

impl WasiIdentifier {
    fn file(&self) -> &File {
        self.file.as_ref().expect("Called file() on an identifier that has already been destroyed, this should never happen! Please file a bug!")
    }

    fn current_file_offset(&self) -> io::Result<u64> {
        self.file().seek(SeekFrom::Current(0))
    }

    fn has_content_left_to_read(&self) -> io::Result<bool> {
        Ok(self.current_file_offset()? < self.size)
    }

    /// Creates a `WasiIdentifier` from a raw file descriptor. The preferred way to create a
    /// `WasiIdentifier` is through one of the `TryFrom` implementations.
    ///
    /// # Safety
    ///
    /// The `owns_fd` argument should only be true, if the given file descriptor owns the resource
    /// it points to (for example a file).
    /// If it is true, a `File` can be obtained back with `Clircle::into_inner`, or it will be
    /// closed when the `WasiIdentifier` is dropped.
    ///
    /// # Errors
    ///
    /// The underlying call to `File::metadata` fails.
    pub unsafe fn try_from_raw_fd(fd: RawFd, owns_fd: bool) -> io::Result<Self> {
        Self::try_from(File::from_raw_fd(fd)).map(|mut ident| {
            ident.owns_fd = owns_fd;
            ident
        })
    }
}

impl Clircle for WasiIdentifier {
    #[must_use]
    fn into_inner(mut self) -> Option<File> {
        if self.owns_fd {
            self.owns_fd = false;
            self.file.take()
        } else {
            None
        }
    }

    /// This method implements the conflict check that is used in the GNU coreutils program `cat`.
    #[must_use]
    fn surely_conflicts_with(&self, other: &Self) -> bool {
        PartialEq::eq(self, other)
            && self.is_regular_file
            && other.has_content_left_to_read().unwrap_or(true)
    }
}

impl TryFrom<Stdio> for WasiIdentifier {
    type Error = <Self as TryFrom<File>>::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let fd = match stdio {
            Stdio::Stdin => libc::STDIN_FILENO,
            Stdio::Stdout => libc::STDOUT_FILENO,
            Stdio::Stderr => libc::STDERR_FILENO,
        };
        // Safety: It is okay to create the file, because it won't be dropped later since the
        // `owns_fd` field is not set.
        unsafe { Self::try_from_raw_fd(fd as RawFd, false) }
    }
}

impl ops::Drop for WasiIdentifier {
    fn drop(&mut self) {
        if !self.owns_fd {
            let _ = self.file.take().map(IntoRawFd::into_raw_fd);
        }
    }
}

impl TryFrom<File> for WasiIdentifier {
    type Error = io::Error;

    fn try_from(file: File) -> Result<Self, Self::Error> {
        file.metadata().map(|metadata| Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.size(),
            is_regular_file: metadata.file_type().is_file(),
            file: Some(file),
            owns_fd: true,
        })
    }
}

impl cmp::PartialEq for WasiIdentifier {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

impl Eq for WasiIdentifier {}

impl hash::Hash for WasiIdentifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.device.hash(state);
        self.inode.hash(state);
    }
}
