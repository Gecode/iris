use clap::Args;
use iris_core::{Params, Spec};
use std::{
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

#[derive(Args)]
pub struct Arguments {
    spec: PathBuf,
    input: PathBuf,
    /// Print canonical JSON.
    #[arg(long, required = true)]
    json: bool,
}
pub fn run(arguments: Arguments) -> ExitCode {
    match read(&arguments) {
        Ok(document) => match io::stdout().lock().write_all(&document) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("iris read: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
fn read(arguments: &Arguments) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source =
        std::fs::read(&arguments.spec).map_err(|e| format!("{}: {e}", arguments.spec.display()))?;
    let spec = Spec::parse(&source, &arguments.spec.to_string_lossy())?;
    let data = spec.read_path(&arguments.input, &Params::new())?;
    Ok(data.to_json()?)
}
