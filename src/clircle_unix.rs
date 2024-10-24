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
pub(crate) struct Identifier {
    device: u64,
    inode: u64,
    size: u64,
    is_regular_file: bool,
    file: Option<File>,
    owns_fd: bool,
}

impl Identifier {
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
    unsafe fn try_from_raw_fd(fd: RawFd, owns_fd: bool) -> io::Result<Self> {
        Self::try_from(File::from_raw_fd(fd)).map(|mut ident| {
            ident.owns_fd = owns_fd;
            ident
        })
    }
}

impl Clircle for Identifier {
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

impl TryFrom<Stdio> for Identifier {
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

impl ops::Drop for Identifier {
    fn drop(&mut self) {
        if !self.owns_fd {
            let _ = self.file.take().map(IntoRawFd::into_raw_fd);
        }
    }
}

impl TryFrom<File> for Identifier {
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

impl cmp::PartialEq for Identifier {
    #[must_use]
    fn eq(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

impl Eq for Identifier {}

impl hash::Hash for Identifier {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.device.hash(state);
        self.inode.hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;
    use std::os::fd::OwnedFd;

    use nix::pty::{openpty, OpenptyResult};
    use nix::unistd::close;

    #[test]
    fn test_into_inner() {
        let file = tempfile::tempfile().expect("failed to create tempfile");
        file.metadata().expect("can stat file");
        let ident = Identifier::try_from(file).expect("failed to create identifier");
        let mut file = ident
            .into_inner()
            .expect("failed to convert identifier to file");
        file.write_all(b"some test content")
            .expect("failed to write test content to file");
    }

    #[test]
    fn test_borrowed_fd() {
        let file = tempfile::tempfile().expect("failed to create tempfile");
        let fd: OwnedFd = file.into();
        let ident = unsafe { Identifier::try_from_raw_fd(fd.as_raw_fd(), false) }
            .expect("failed to create identifier");
        drop(ident);
        let fd = fd.into_raw_fd();
        close(fd).expect("error closing fd");
        #[cfg(feature = "test-close-again")]
        close(fd).expect_err("closing again should fail");
    }

    #[test]
    fn test_owned_fd() {
        let file = tempfile::tempfile().expect("failed to create tempfile");
        let fd: OwnedFd = file.into();
        let ident = unsafe { Identifier::try_from_raw_fd(fd.as_raw_fd(), true) }
            .expect("failed to create identifier");
        drop(ident);
        #[cfg(feature = "test-close-again")]
        close(fd.into_raw_fd())
            .expect_err("the fd should have already been closed by dropping the identifier");
    }

    #[test]
    fn test_pty_equal_but_not_conflicting() {
        let OpenptyResult {
            master: parent,
            slave: child,
        } = openpty(None, None).expect("failed to open pty");

        let parent_ident = unsafe { Identifier::try_from_raw_fd(parent.as_raw_fd(), false) }
            .expect("failed to create parent identifier");

        assert_eq!(parent_ident, parent_ident);

        assert!(!parent_ident.surely_conflicts_with(&parent_ident));

        let child_ident = unsafe { Identifier::try_from_raw_fd(child.as_raw_fd(), false) }
            .expect("failed to create child identifier");

        assert_ne!(parent_ident, child_ident);
        assert!(!parent_ident.surely_conflicts_with(&child_ident));

        drop(child_ident);
        drop(parent_ident);
        let child = child.into_raw_fd();
        close(child).expect("failed to close child");
        #[cfg(feature = "test-close-again")]
        close(child).expect_err("closing child again should fail");
        let parent = parent.into_raw_fd();
        close(parent).expect("failed to close parent");
        #[cfg(feature = "test-close-again")]
        close(parent).expect_err("closing parent again should fail");
    }
}
