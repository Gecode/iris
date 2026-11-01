use clap::{Parser, Subcommand};
use iris_cli::{CheckFailure, check_spec, format_name, render_error};
use std::{path::PathBuf, process::ExitCode};
#[path = "iris/conformance.rs"]
mod conformance;
#[path = "iris/fmt.rs"]
mod fmt;
#[path = "iris/list.rs"]
mod list;
#[path = "iris/read.rs"]
mod read;
#[path = "iris/summary.rs"]
mod summary;
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
fn main() -> ExitCode {
    match Arguments::parse().command {
        Command::Conformance(arguments) => conformance::run(arguments),
        Command::Read(arguments) => read::run(arguments),
        Command::List(arguments) => list::run(arguments),
        Command::Check { spec } => check(spec),
        Command::Fmt(arguments) => fmt::run(arguments),
        Command::Lsp => match iris_lsp::run_stdio() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("iris lsp: {error}");
                ExitCode::FAILURE
            }
        },
    }
}

fn check(spec: PathBuf) -> ExitCode {
    let origin = spec.to_string_lossy();
    let bytes = match std::fs::read(&spec) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{origin}: {error}");
            return ExitCode::from(1);
        }
    };
    match check_spec(&bytes, &origin) {
        Ok(checked) => {
            let suffix = format_name(&checked.parsed)
                .filter(|n| !n.is_empty())
                .map_or_else(String::new, |n| format!(" (format {n})"));
            println!("{origin}: ok{suffix}");
            ExitCode::SUCCESS
        }
        Err(CheckFailure::Language(error)) => {
            eprintln!("{}", render_error(&error, &bytes));
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{origin}: {error}");
            ExitCode::from(1)
        }
    }
}
