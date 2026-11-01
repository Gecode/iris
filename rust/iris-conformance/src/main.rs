use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use iris_conformance::RunOptions;

#[derive(Parser)]
#[command(
    version,
    about = "Run Iris conformance cases against an implementation driver"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check reader specs against the formal grammar, independently of drivers.
    CheckGrammar { paths: Vec<PathBuf> },
    /// Generate conformance cases from executable Specification examples.
    ExtractExamples {
        source: PathBuf,
        destination: PathBuf,
        #[arg(long)]
        check: bool,
    },
    Run {
        #[arg(long)]
        driver: String,
        #[arg(long)]
        driver_id: Option<String>,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long, default_value = "0.1")]
        version: String,
        #[arg(long, default_value = "30", value_parser = parse_timeout)]
        timeout: Duration,
        #[arg(required = true)]
        cases_dir: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Commands::CheckGrammar { paths } => {
            match iris_conformance::grammar::check_grammar(&paths) {
                Ok(report) => {
                    print!("{}", report.render());
                    ExitCode::from(report.exit_code())
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(2)
                }
            }
        }
        Commands::ExtractExamples {
            source,
            destination,
            check,
        } => match iris_conformance::examples::extract(&source, &destination, check) {
            Ok(report) => {
                println!("{report}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: {error}");
                ExitCode::from(1)
            }
        },
        Commands::Run {
            driver,
            driver_id,
            filter,
            version,
            timeout,
            cases_dir,
        } => {
            let options = RunOptions {
                driver,
                driver_id,
                roots: cases_dir,
                filter,
                version,
                timeout,
            };
            match iris_conformance::run(&options) {
                Ok(report) => {
                    print!("{}", report.render());
                    ExitCode::from(report.exit_code())
                }
                Err(error) => {
                    eprintln!("error: {error}");
                    ExitCode::from(2)
                }
            }
        }
    }
}

fn parse_timeout(text: &str) -> Result<Duration, String> {
    let seconds: f64 = text
        .parse()
        .map_err(|_| "timeout must be positive seconds".to_string())?;
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| "timeout must be finite, positive seconds".to_string())?;
    if duration.is_zero() {
        return Err("timeout must be positive".into());
    }
    Ok(duration)
}
