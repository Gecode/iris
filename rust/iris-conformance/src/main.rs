use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use clap::{ArgGroup, Parser, Subcommand};
use iris_conformance::RunOptions;
use iris_conformance::compare;
use iris_conformance::differential::{self, Limit};

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
    /// Compare two reader specs as syntax trees, or each line of a manifest.
    /// Exits 0 when they agree, 1 on a difference and 2 when a spec cannot be
    /// read or recognized.
    #[command(group(ArgGroup::new("input").required(true).args(["left", "manifest"])))]
    Compare {
        #[arg(requires = "right")]
        left: Option<PathBuf>,
        right: Option<PathBuf>,
        /// A file of `ORIGINAL TAB PRINTED` lines, or `ORIGINAL` alone for a skip.
        #[arg(long, conflicts_with = "right")]
        manifest: Option<PathBuf>,
    },
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
        /// Report cases whose paths contain this text as skipped; repeatable.
        #[arg(long)]
        skip: Vec<String>,
        #[arg(long, default_value = "0.1")]
        version: String,
        #[arg(long, default_value = "30", value_parser = parse_timeout)]
        timeout: Duration,
        #[arg(required = true)]
        cases_dir: Vec<PathBuf>,
    },
    /// Compare drivers on generated and mutated cases until the first
    /// difference in canonical outcome.
    #[command(group(ArgGroup::new("limit").required(true).args(["cases", "duration"])))]
    Differential {
        /// A driver command; give two or more.
        #[arg(long = "driver", required = true)]
        drivers: Vec<String>,
        /// The run's seed; by default one from the clock, which the summary prints.
        #[arg(long)]
        seed: Option<u64>,
        /// The index of the first case.
        #[arg(long, default_value_t = 0)]
        start: u64,
        /// Stop after this many cases.
        #[arg(long)]
        cases: Option<u64>,
        /// Stop starting cases after this many seconds.
        #[arg(long, value_parser = parse_seconds)]
        duration: Option<Duration>,
        /// Cases compared at the same time.
        #[arg(long, default_value_t = 1)]
        jobs: usize,
        /// Seconds each driver may take for one request.
        #[arg(long, default_value = "30", value_parser = parse_timeout)]
        timeout: Duration,
        /// Write the original and reduced failing case to this empty directory.
        #[arg(long)]
        keep: Option<PathBuf>,
        #[arg(long, default_value = "0.1")]
        version: String,
        /// Conformance case roots whose cases the mutate strategy changes.
        cases_dir: Vec<PathBuf>,
    },
}

fn compare_files(left: &Path, right: &Path) -> ExitCode {
    let read =
        |path: &Path| std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()));
    let result = read(left).and_then(|left_bytes| {
        let right_bytes = read(right)?;
        compare::compare(&left_bytes, &right_bytes).map_err(|failure| failure.to_string())
    });
    match result {
        Ok(None) => ExitCode::SUCCESS,
        Ok(Some(difference)) => {
            println!("{difference}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn compare_manifest(manifest: &Path) -> ExitCode {
    match compare::compare_manifest(manifest) {
        Ok(batch) => {
            for difference in &batch.differences {
                println!("DIFF {difference}");
            }
            println!("{}", batch.summary());
            ExitCode::from(u8::from(!batch.differences.is_empty()))
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
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
        Commands::Compare {
            left,
            right,
            manifest,
        } => match manifest {
            Some(manifest) => compare_manifest(&manifest),
            None => compare_files(
                &left.expect("clap requires left without a manifest"),
                &right.expect("clap requires right with left"),
            ),
        },
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
            skip,
            version,
            timeout,
            cases_dir,
        } => {
            let options = RunOptions {
                driver,
                driver_id,
                roots: cases_dir,
                filter,
                skip,
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
        Commands::Differential {
            drivers,
            seed,
            start,
            cases,
            duration,
            jobs,
            timeout,
            keep,
            version,
            cases_dir,
        } => {
            let limit = match (cases, duration) {
                (Some(cases), _) => Limit::Cases(cases),
                (None, Some(duration)) => Limit::Duration(duration),
                (None, None) => unreachable!("clap requires --cases or --duration"),
            };
            let options = differential::Options {
                drivers,
                seed: seed.unwrap_or_else(clock_seed),
                start,
                limit,
                jobs,
                timeout,
                keep,
                version,
                roots: cases_dir,
            };
            match differential::run(&options) {
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

fn clock_seed() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
}

fn parse_seconds(text: &str) -> Result<Duration, String> {
    let seconds: f64 = text
        .parse()
        .map_err(|_| "duration must be positive seconds".to_string())?;
    match Duration::try_from_secs_f64(seconds) {
        Ok(duration) if !duration.is_zero() => Ok(duration),
        _ => Err("duration must be finite, positive seconds".into()),
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
