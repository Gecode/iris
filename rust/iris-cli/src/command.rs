//! The `iris` command line, run from arguments to an exit status.
use crate::{CheckFailure, check_spec, format_name, render_error};
use clap::{Parser, Subcommand};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};
mod conformance;
mod fmt;
mod list;
mod read;
#[derive(Parser)]
#[command(
    name = "iris",
    version = concat!(env!("CARGO_PKG_VERSION"), " (languages: 0.1)"),
    about = "Read inputs, check and format Iris reader specs, and run conformance cases"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Read input and print a summary, canonical JSON, or MiniZinc data.
    Read(read::Arguments),
    /// List instances, names, and rejection summaries.
    List(list::Arguments),
    /// Check a reader spec without reading input.
    Check { spec: PathBuf },
    /// Format reader specs in place, or preview changes.
    Fmt(fmt::Arguments),
    /// Serve the Iris language server over stdio.
    Lsp,
    /// Run conformance cases against an implementation driver.
    Conformance(conformance::Arguments),
}

/// Runs the `iris` command line on `args` and returns its exit status.
///
/// The first argument is the program name, which help and usage messages
/// show, as in a process's own arguments. Output goes to the process's
/// standard output and error, and standard output is flushed before the
/// function returns. Usage errors return 2, failures 1, and success 0.
pub fn run<I, T>(args: I) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let status = match Arguments::try_parse_from(args) {
        Ok(arguments) => dispatch(arguments.command),
        Err(error) => {
            // Help and version print to standard output, other errors to standard error.
            let _ = error.print();
            u8::try_from(error.exit_code()).unwrap_or(2)
        }
    };
    // A process flushes standard output at exit; a library caller may not exit.
    let _ = io::stdout().flush();
    status
}

fn dispatch(command: Command) -> u8 {
    match command {
        Command::Conformance(arguments) => conformance::run(arguments),
        Command::Read(arguments) => read::run(arguments),
        Command::List(arguments) => list::run(arguments),
        Command::Check { spec } => check(spec),
        Command::Fmt(arguments) => fmt::run(arguments),
        Command::Lsp => match iris_lsp::run_stdio() {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("iris lsp: {error}");
                1
            }
        },
    }
}

fn check(spec: PathBuf) -> u8 {
    let origin = spec.to_string_lossy();
    let bytes = match std::fs::read(&spec) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{origin}: {error}");
            return 1;
        }
    };
    match check_spec(&bytes, &origin) {
        Ok(checked) => {
            let suffix = format_name(&checked.parsed)
                .filter(|n| !n.is_empty())
                .map_or_else(String::new, |n| format!(" (format {n})"));
            println!("{origin}: ok{suffix}");
            0
        }
        Err(CheckFailure::Language(error)) => {
            eprintln!("{}", render_error(&error, &bytes));
            1
        }
        Err(error) => {
            eprintln!("{origin}: {error}");
            1
        }
    }
}
