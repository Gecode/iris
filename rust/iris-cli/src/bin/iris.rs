use clap::{Parser, Subcommand};
use iris_cli::{CheckFailure, check_spec, format_name, render_error};
use std::{path::PathBuf, process::ExitCode};
#[derive(Parser)]
#[command(
    name = "iris",
    version = "0.1.0 (language 0.1)",
    about = "Check an Iris reader spec"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Check { spec: PathBuf },
}
fn main() -> ExitCode {
    let Arguments {
        command: Command::Check { spec },
    } = Arguments::parse();
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
