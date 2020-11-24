//! The `clircle` crate helps you detect IO circles in your CLI applications.
//!
//! Imagine you want to
//! read data from a couple of files and output something according to the contents of these files.
//! If the user redirects the output of your program to one of the input files, you might end up in
//! an infinite circle of reading and writing.
//!
//! The crate provides the struct `Identifier` which is a platform dependent type alias, so that
//! you can use it on all platforms and do not need to introduce any conditional compilation
//! yourself.
//! On both Unix and Windows systems, `Identifier` holds information to identify a file on a disk.
//!
//! The `Clircle` trait is implemented on both of these structs and requires `TryFrom` for the
//! `clircle::Stdio` enum and for `&Path`, so that all possible inputs can be represented as an
//! `Identifier`.
//! Finally, `Clircle` is a subtrait of `Eq`, so that the identifiers can be conveniently compared
//! and circles can be detected.
//! The `clircle` crate also provides some convenience functions around the comparison of `Clircle`
//! implementors.
#![deny(clippy::all)]
#![deny(missing_docs)]
#![warn(clippy::pedantic)]

cfg_if::cfg_if! {
    if #[cfg(unix)] {
        mod clircle_unix;
        pub use clircle_unix::{nix, UnixIdentifier};
        /// Identifies a file. The type is aliased according to the target platform.
        pub type Identifier = UnixIdentifier;
    } else if #[cfg(windows)] {
        mod clircle_windows;
        pub use clircle_windows::{winapi, WindowsIdentifier};
        /// Identifies a file. The type is aliased according to the target platform.
        pub type Identifier = WindowsIdentifier;
    } else {
        compile_error!("Neither cfg(unix) nor cfg(windows) was true, aborting.");
    }
}

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::path::Path;

/// The `Clircle` trait describes the public interface of the crate.
/// It contains all the platform-independent functionality.
/// This trait is implemented for the structs `UnixIdentifier` and `WindowsIdentifier`.
pub trait Clircle: Eq + TryFrom<Stdio> + for<'a> TryFrom<&'a Path> {}

/// The three stdio streams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[allow(missing_docs)]
pub enum Stdio {
    Stdin,
    Stdout,
    Stderr,
}

impl<T> Clircle for T where T: Eq + TryFrom<Stdio> + for<'a> TryFrom<&'a Path> {}

/// Finds a common `Identifier` in the two given slices.
pub fn output_among_inputs<'o, T>(outputs: &'o [T], inputs: &[T]) -> Option<&'o T>
where
    T: Clircle,
{
    outputs.iter().find(|output| inputs.contains(output))
}

/// Finds `Stdio::Stdout` in the given slice.
pub fn stdout_among_inputs<T>(inputs: &[T]) -> bool
where
    T: Clircle,
{
    T::try_from(Stdio::Stdout)
        .map(|stdout| inputs.contains(&stdout))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs::{self, File};
    use std::hash::Hash;

    fn contains_duplicates<T>(items: &[T]) -> bool
    where
        T: Eq + Hash + Copy,
    {
        let mut set = HashSet::new();
        items.iter().copied().any(|item| !set.insert(item))
    }

    #[test]
    fn test_from_path() -> Result<(), &'static str> {
        // To ensure that this directory is deleted again, assert! is not invoked.
        // Instead, this method returns Result and .? is used.
        let dir = tempfile::tempdir().expect("Couldn't create tempdir.");
        let dir_path = dir.path().to_path_buf();

        let non_existing_file = dir_path.join("oop_in_c.txt");
        if Identifier::try_from(non_existing_file.as_path()).is_ok() {
            return Err(
                "Identifier::try_from returned Ok when given a path to a file that does not exist.",
            );
        }

        let exising_file = dir_path.join("useful_rust_resources.txt");
        fs::write(&exising_file, b"github.com/rust-lang/rustlings")
            .map_err(|_| "Failed to write file.")?;
        Identifier::try_from(exising_file.as_path())
            .map_err(|_| "Identifier::try_from returned Err when given a path to a valid file.")?;

        Ok(())
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

        for path in &paths {
            File::create(&path).map_err(|_| "Couldn't create temporary file.")?;
        }

        let identifiers = paths
            .iter()
            .map(AsRef::as_ref)
            .map(Identifier::try_from)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "Some Identifier conversions failed.")?;

        if contains_duplicates(&identifiers) {
            return Err("Duplicate identifier found for set of unique paths.");
        }

        Ok(())
    }
}
