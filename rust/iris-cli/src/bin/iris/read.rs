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
    #[arg(required = true, num_args = 1..)]
    input: Vec<PathBuf>,
    #[command(flatten)]
    params: Parameters,
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
#[derive(Args)]
pub(super) struct Parameters {
    /// Supply an exact string value; repeated keys replace earlier values.
    #[arg(long = "param", value_name = "NAME=VALUE", value_parser = assignment)]
    values: Vec<(String, String)>,
}
fn assignment(text: &str) -> Result<(String, String), String> {
    let (name, value) = text.split_once('=').ok_or("expected NAME=VALUE")?;
    if name.is_empty() {
        return Err("parameter name must not be empty".into());
    }
    Ok((name.into(), value.into()))
}
impl Parameters {
    pub(super) fn params(&self) -> Params {
        let mut params = Params::new();
        for (name, value) in &self.values {
            params.set(name, value);
        }
        params
    }
}
fn read(arguments: &Arguments) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let source =
        std::fs::read(&arguments.spec).map_err(|e| format!("{}: {e}", arguments.spec.display()))?;
    let spec = Spec::parse(&source, &arguments.spec.to_string_lossy())?;
    let params = arguments.params.params();
    let mut document = vec![];
    for input in &arguments.input {
        document.extend(spec.read_path(input, &params)?.to_json()?);
    }
    Ok(document)
}
