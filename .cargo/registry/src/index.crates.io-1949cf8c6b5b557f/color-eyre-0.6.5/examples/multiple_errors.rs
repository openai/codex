use color_eyre::{eyre::eyre, eyre::Report, Section};
use thiserror::Error;

fn main() -> Result<(), Report> {
    color_eyre::install()?;
    let errors = get_errors();
    join_errors(errors)
}

fn join_errors(results: Vec<Result<(), SourceError>>) -> Result<(), Report> {
    if results.iter().all(|r| r.is_ok()) {
        return Ok(());
    }

    let err = results
        .into_iter()
        .filter(Result::is_err)
        .map(Result::unwrap_err)
        .fold(eyre!("encountered multiple errors"), |report, e| {
            report.error(e)
        });

    Err(err)
}

/// Helper function to generate errors
fn get_errors() -> Vec<Result<(), SourceError>> {
    vec![
        Err(SourceError {
            source: StrError("The task you ran encountered an error"),
            msg: "The task could not be completed",
        }),
        Err(SourceError {
            source: StrError("The machine you're connecting to is actively on fire"),
            msg: "The machine is unreachable",
        }),
        Err(SourceError {
            source: StrError("The file you're parsing is literally written in c++ instead of rust, what the hell"),
            msg: "The file could not be parsed",
        }),
    ]
}

/// Arbitrary error type for demonstration purposes
#[derive(Debug, Error)]
#[error("{0}")]
struct StrError(&'static str);

/// Arbitrary error type for demonstration purposes with a source error
#[derive(Debug, Error)]
#[error("{msg}")]
struct SourceError {
    msg: &'static str,
    source: StrError,
}
