# Clircle

[![CI](https://github.com/niklasmohrin/clircle/actions/workflows/ci.yml/badge.svg?branch=main&event=push)](https://github.com/niklasmohrin/clircle/actions/workflows/ci.yml)
[![crates.io version](https://img.shields.io/crates/v/clircle)](https://crates.io/crates/clircle)
[![MSRV](https://img.shields.io/badge/MSRV-1.69.0-blue)](https://blog.rust-lang.org/2023/04/20/Rust-1.69.0.html)

Clircle provides a cross-platform API to detect read / write cycles from your
user-supplied arguments. You can get the important identifiers of a file (from
a path) and for all three stdio streams, if they are piped from or to a file as
well.

## Why?

Imagine you want to read data from a couple of files and output something according to the
contents of these files. If the user redirects the output of your program to one of the
input files, you might end up in an infinite circle of reading and writing.

The crate provides the struct `Identifier` which is a platform dependent type alias, so that
you can use it on all platforms and do not need to introduce any conditional compilation
yourself.
On both Unix and Windows systems, `Identifier` holds information to identify a file on a disk.

The `Clircle` trait is implemented on both of these structs and requires `TryFrom` for the
`clircle::Stdio` enum and for `&Path`, so that all possible inputs can be represented as an
`Identifier`.
Finally, `Clircle` is a subtrait of `Eq`, so that the identifiers can be conveniently compared
and circles can be detected.
The `clircle` crate also provides some convenience functions around the comparison of `Clircle`
implementors.

## Why should I use this and not just `fs::Metadata`?

The `clircle` crate seamlessly works on Linux **and** Windows through
a single API, so no conditional compilation is needed at all.
Furthermore, `MetadataExt` is not stable on Windows yet, meaning you
would have to dig into the Windows APIs yourself to get the information
needed to identify a file.

## Where did this crate come from?

This crate originated in a pull request to the [`bat`](https://github.com/sharkdp/bat) project.
The `bat` tool strives to be a drop-in replacement for the unix tool `cat`.
Since `cat` detects these cycles, `bat` has to do so too, which is where most
of this code came into play. However, it was decided, that the new logic was

- useful for other projects and
- too platform specific for `bat`s scope.

So now, you can use `clircle` too!
