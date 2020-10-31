#[cfg(unix)]
fn main() {
    use std::convert::TryFrom;

    let cli_input_args = ["/some/file", "~/myFile"]
        .iter()
        .map(AsRef::as_ref)
        .flat_map(clircle::Identifier::try_from)
        .collect::<Vec<_>>();
    let cli_output_args = ["/another/file"]
        .iter()
        .map(AsRef::as_ref)
        .flat_map(clircle::Identifier::try_from)
        .collect::<Vec<_>>();

    let common = clircle::output_among_inputs(&cli_input_args, &cli_output_args);
    assert_eq!(common, None);
}

#[cfg(windows)]
fn main() {}
