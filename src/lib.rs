//! The `clircle` crate helps you detect IO circles in your CLI applications.
//!
//! Imagine you want to read data from a couple of files and output something according to the
//! contents of these files. If the user redirects the output of your program to one of the input
//! files, you might end up in an infinite circle of reading and writing.
//!
//! The crate provides the struct `Identifier` which is a platform dependent type alias, so that
//! you can use it on all platforms and do not need to introduce any conditional compilation
//! yourself. `Identifier` implements the `Clircle` trait, which is where you should look for the
//! public functionality.
//!
//! The `Clircle` trait is implemented on both of these structs and requires `TryFrom` for the
//! `clircle::Stdio` enum and for `File`, so that all possible inputs can be represented as an
//! `Identifier`. Additionally, there are `unsafe` methods for each specific implementation, but
//! they are not recommended to use.
//! Finally, `Clircle` is a subtrait of `Eq`, which allows checking if two `Identifier`s point to
//! the same file, even if they don't conflict. If you only need this last feature, you should
//! use [`same-file`](https://crates.io/crates/same-file) instead of this crate.
//!
//! ## Examples
//!
//! To check if two `Identifier`s conflict, use
//! `Clircle::surely_conflicts_with`:
//!
//! ```rust,no_run
//! # fn example() -> Option<()> {
//! # use clircle::{Identifier, Clircle, Stdio::{Stdin, Stdout}};
//! # use std::convert::TryFrom;
//! let stdin = Identifier::stdin()?;
//! let stdout = Identifier::stdout()?;
//!
//! if stdin.surely_conflicts_with(&stdout) {
//!     eprintln!("stdin and stdout are conflicting!");
//! }
//! # Some(())
//! # }
//! ```
//!
//! On Linux, the above snippet could be used to detect `cat < x > x`, while allowing just
//! `cat`, although stdin and stdout are pointing to the same pty in both cases. On Windows, this
//! code will not print anything, because the same operation is safe there.

#![deny(clippy::all)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        mod clircle_unix;
        use clircle_unix as imp;
    } else if #[cfg(windows)] {
        mod clircle_windows;
        use clircle_windows as imp;
    } else {
        compile_error!("Neither cfg(unix) nor cfg(windows) was true, aborting.");
    }
}

#[cfg(feature = "serde")]
use serde_derive::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fs::File;
use std::io;

/// The `Clircle` trait describes the public interface of the crate.
/// It contains all the platform-independent functionality.
/// Additionally, an implementation of `Eq` is required, that gives a simple way to check for
/// conflicts, if using the more elaborate `surely_conflicts_with` method is not wanted.
/// This trait is implemented for the structs `UnixIdentifier` and `WindowsIdentifier`.
pub trait Clircle: Eq + TryFrom<Stdio> + TryFrom<File> {
    /// Returns the `File` that was used for `From<File>`. If the instance was created otherwise,
    /// this may also return `None`.
    fn into_inner(self) -> Option<File>;

    /// Checks whether the two values will without doubt conflict. By default, this always returns
    /// `false`, but implementors can override this method. Currently, only the Unix implementation
    /// overrides `surely_conflicts_with`.
    fn surely_conflicts_with(&self, _other: &Self) -> bool {
        false
    }

    /// Shorthand for `try_from(Stdio::Stdin)`.
    #[must_use]
    fn stdin() -> Option<Self> {
        Self::try_from(Stdio::Stdin).ok()
    }

    #[must_use]
    /// Shorthand for `try_from(Stdio::Stdout)`.
    fn stdout() -> Option<Self> {
        Self::try_from(Stdio::Stdout).ok()
    }

    #[must_use]
    /// Shorthand for `try_from(Stdio::Stderr)`.
    fn stderr() -> Option<Self> {
        Self::try_from(Stdio::Stderr).ok()
    }
}

/// The three stdio streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(missing_docs)]
pub enum Stdio {
    Stdin,
    Stdout,
    Stderr,
}

/// Finds a common `Identifier` in the two given slices.
pub fn output_among_inputs<'o, T>(outputs: &'o [T], inputs: &[T]) -> Option<&'o T>
where
    T: Clircle,
{
    outputs.iter().find(|output| inputs.contains(output))
}

/// Checks if `Stdio::Stdout` is in the given slice.
pub fn stdout_among_inputs<T>(inputs: &[T]) -> bool
where
    T: Clircle,
{
    T::stdout().map_or(false, |stdout| inputs.contains(&stdout))
}

/// Identifies a file. The type forwards all methods to the platform implementation.
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct Identifier(imp::Identifier);

impl Clircle for Identifier {
    #[must_use]
    fn into_inner(self) -> Option<File> {
        self.0.into_inner()
    }

    fn surely_conflicts_with(&self, other: &Self) -> bool {
        self.0.surely_conflicts_with(&other.0)
    }
}

impl TryFrom<Stdio> for Identifier {
    type Error = io::Error;

    fn try_from(stdio: Stdio) -> Result<Self, Self::Error> {
        imp::Identifier::try_from(stdio).map(Self)
    }
}

impl TryFrom<File> for Identifier {
    type Error = io::Error;

    fn try_from(file: File) -> Result<Self, Self::Error> {
        imp::Identifier::try_from(file).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::hash::Hash;

    fn contains_duplicates<T>(items: Vec<T>) -> bool
    where
        T: Eq + Hash,
    {
        let mut set = HashSet::new();
        items.into_iter().any(|item| !set.insert(item))
    }

    #[test]
    fn test_basic_comparisons() -> Result<(), &'static str> {
        let dir = tempfile::tempdir().expect("Couldn't create tempdir.");
        let dir_path = dir.path().to_path_buf();

        let filenames = ["a", "b", "c", "d"];
        let paths: Vec<_> = filenames
            .iter()
            .map(|filename| dir_path.join(filename))
            .collect();

        let identifiers = paths
            .iter()
            .map(File::create)
            .map(Result::unwrap)
            .map(Identifier::try_from)
            .map(Result::unwrap)
            .collect::<Vec<_>>();

        if contains_duplicates(identifiers) {
            return Err("Duplicate identifier found for set of unique paths.");
        }

        Ok(())
    }
}
