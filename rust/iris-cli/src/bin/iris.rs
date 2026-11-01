use clap::{Parser, Subcommand};
use iris_cli::{CheckFailure, check_spec, format_name, render_error};
use std::{path::PathBuf, process::ExitCode};
#[path = "iris/fmt.rs"]
mod fmt;
#[path = "iris/read.rs"]
mod read;
#[derive(Parser)]
#[command(
    name = "iris",
    version = "0.1.0 (language 0.1)",
    about = "Check and format Iris reader specs"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Read scalar input and print canonical JSON.
    Read(read::Arguments),
    Check {
        spec: PathBuf,
    },
    /// Format reader specs in place, or preview changes.
    Fmt(fmt::Arguments),
    /// Serve the Iris language server over stdio.
    Lsp,
}
fn main() -> ExitCode {
    match Arguments::parse().command {
        Command::Read(arguments) => read::run(arguments),
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
