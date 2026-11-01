use clap::Args;
use iris_conformance::RunOptions;
use std::{path::PathBuf, process::ExitCode, time::Duration};

#[derive(Args)]
pub struct Arguments {
    /// Driver executable and optional prefix arguments, using shell quoting (no shell runs).
    driver: String,
    /// Case roots, relative to the current directory.
    #[arg(default_value = "conformance/cases/0.1")]
    roots: Vec<PathBuf>,
    /// Select this driver's known-failure annotations; omitted means no exemptions.
    #[arg(long)]
    driver_id: Option<String>,
    /// Select cases whose paths contain this text.
    #[arg(long)]
    filter: Option<String>,
    /// Language version used by the cases and driver.
    #[arg(long, default_value = "0.1")]
    version: String,
    /// Positive timeout in seconds per case.
    #[arg(long, default_value = "30", value_parser = parse_timeout)]
    timeout: Duration,
}

pub fn run(arguments: Arguments) -> ExitCode {
    let options = RunOptions {
        driver: arguments.driver,
        driver_id: arguments.driver_id,
        roots: arguments.roots,
        filter: arguments.filter,
        version: arguments.version,
        timeout: arguments.timeout,
    };
    match iris_conformance::run(&options) {
        Ok(report) => {
            print!("{}", report.render());
            ExitCode::from(report.exit_code())
        }
        Err(error) => {
            eprintln!("iris conformance: {error}");
            ExitCode::from(2)
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
