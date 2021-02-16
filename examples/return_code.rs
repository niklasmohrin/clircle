//! This example collects all given arguments, interprets them as paths, gathers information
//! about them and checks if stdout is being redirected to one of the given files.
//! If that's the case, the return code will be set to 1. In both cases, there is an according
//! output to stderr.
use clircle::{stdout_among_inputs, Identifier};
use std::convert::TryFrom;
use std::fs::File;

fn main() {
    let inputs: Vec<_> = std::env::args().collect();
    let inputs: Result<Vec<Identifier>, _> = inputs
        .iter()
        .map(File::open)
        .map(Result::unwrap)
        .map(Identifier::try_from)
        .collect();
    let inputs = inputs.expect("There was an argument that could not be converted to an Idenfier!");

    if stdout_among_inputs(&inputs) {
        eprintln!("Cycle detected!");
        std::process::exit(1);
    } else {
        eprintln!("No cycle detected.")
    }
}
