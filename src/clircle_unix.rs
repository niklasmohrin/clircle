use crate::{Clircle, Stdio};

use std::convert::TryFrom;
use std::fs::File;
use std::io::{self, Seek};
use std::os::fd::AsRawFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::{FromRawFd, IntoRawFd, RawFd};
use std::{cmp, hash, ops};

/// Implementation of `Clircle` for Unix.
#[derive(Debug)]
pub struct UnixIdentifier {
    device: u64,
    inode: u64,
    size: u64,
    is_regular_file: bool,
    file: Option<File>,
    owns_fd: bool,
}

impl UnixIdentifier {
    fn file(&self) -> &File {
        self.file.as_ref().expect("Called file() on an identifier that has already been destroyed, this should never happen! Please file a bug!")
    }

    fn current_file_offset(&self) -> io::Result<u64> {
        self.file().stream_position()
    }

    fn has_content_left_to_read(&self) -> io::Result<bool> {
        Ok(self.current_file_offset()? < self.size)
    }

    /// Creates a `UnixIdentifier` from a raw file descriptor. The preferred way to create a
    /// `UnixIdentifier` is through one of the `TryFrom` implementations.
    ///
    /// # Safety
    ///
    /// The `owns_fd` argument should only be true, if the given file descriptor owns the resource
    /// it points to (for example a file).
    /// If it is true, a `File` can be obtained back with `Clircle::into_inner`, or it will be
    /// closed when the `UnixIdentifier` is dropped.
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

impl Clircle for UnixIdentifier {
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

impl TryFrom<Stdio> for UnixIdentifier {
    type Error = <Self as TryFrom<File>>::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        let fd = match stdio {
            Stdio::Stdin => io::stdin().as_raw_fd(),
            Stdio::Stdout => io::stdout().as_raw_fd(),
            Stdio::Stderr => io::stderr().as_raw_fd(),
        };
        // Safety: It is okay to create the file, because it won't be dropped later since the
        // `owns_fd` field is not set.
        unsafe { Self::try_from_raw_fd(fd, false) }
    }
}

impl ops::Drop for UnixIdentifier {
    fn drop(&mut self) {
        if !self.owns_fd {
            let _ = self.file.take().map(IntoRawFd::into_raw_fd);
        }
    }
}

impl TryFrom<File> for UnixIdentifier {
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

impl cmp::PartialEq for UnixIdentifier {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

impl Eq for UnixIdentifier {}

impl hash::Hash for UnixIdentifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.device.hash(state);
        self.inode.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::error::Error;
    use std::io::Write;

    use nix::pty::{openpty, OpenptyResult};
    use nix::unistd::close;

    #[test]
    fn test_fd_closing() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir().expect("Couldn't create tempdir.");
        let dir_path = dir.path().to_path_buf();

        // 1) Check that the file returned by into_inner is still valid
        let file = File::create(dir_path.join("myfile"))?;
        let ident = UnixIdentifier::try_from(file)?;
        let mut file = ident
            .into_inner()
            .ok_or("Did not get file back from identifier")?;
        // Check if file can be written to without weird errors
        file.write_all(b"Some test content")?;

        // 2) Check that dropping the Identifier does not close the file, if owns_fd is false
        let fd = file.into_raw_fd();
        let ident = unsafe { UnixIdentifier::try_from_raw_fd(fd, false) };
        if let Err(e) = ident {
            let _ = dbg!(close(fd));
            return Err(Box::new(e));
        }
        let ident = ident.unwrap();
        drop(ident);
        close(fd).map_err(|e| {
            format!(
                "Error closing file, that I told UnixIdentifier not to close: {}",
                e
            )
        })?;

        // 3) Check that the file is closed on drop, if owns_fd is true
        let fd = File::open(dir_path.join("myfile"))?.into_raw_fd();
        let ident = unsafe { UnixIdentifier::try_from_raw_fd(fd, true) };
        if let Err(e) = ident {
            let _ = dbg!(close(fd));
            return Err(Box::new(e));
        }
        let ident = ident.unwrap();
        drop(ident);
        close(fd).expect_err("This file descriptor should have been closed already!");

        Ok(())
    }

    #[test]
    fn test_pty_equal_but_not_conflicting() -> Result<(), &'static str> {
        let OpenptyResult { master, slave } = openpty(None, None).expect("Could not open pty.");
        let res = unsafe { UnixIdentifier::try_from_raw_fd(slave, false) }
            .map_err(|_| "Error creating UnixIdentifier from pty fd")
            .and_then(|ident| {
                if !ident.eq(&ident) {
                    return Err("ident != ident");
                }
                if ident.surely_conflicts_with(&ident) {
                    return Err("pty fd does not conflict with itself, but conflict detected");
                }

                let second_ident = unsafe { UnixIdentifier::try_from_raw_fd(slave, false) }
                    .map_err(|_| "Error creating second Identifier to pty")?;
                if !ident.eq(&second_ident) {
                    return Err("ident != second_ident");
                }
                if ident.surely_conflicts_with(&second_ident) {
                    return Err(
                        "Two Identifiers to the same pty should not conflict, but they do.",
                    );
                }
                Ok(())
            });

        let r1 = close(master);
        let r2 = close(slave);

        r1.expect("Error closing master end of pty");
        r2.expect("Error closing slave end of pty");

        res
    }
}
