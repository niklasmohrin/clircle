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

#[cfg(unix)]
mod clircle_unix;
#[cfg(unix)]
pub use clircle_unix::UnixIdentifier;

#[cfg(windows)]
mod clircle_windows;
#[cfg(windows)]
pub use clircle_windows::WindowsIdentifier;

#[cfg(unix)]
/// Identifies a file. The type is aliased according to the target platform.
pub type Identifier = UnixIdentifier;
#[cfg(windows)]
/// Identifies a file. The type is aliased according to the target platform.
pub type Identifier = WindowsIdentifier;

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
    T::try_from(Stdio::Stdout).map_or(false, |stdout| inputs.contains(&stdout))
}
