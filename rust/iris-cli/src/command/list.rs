use clap::Args;
use gecode_iris_core::{Data, Spec};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Args)]
pub struct Arguments {
    spec: PathBuf,
    #[arg(required = true, num_args = 1..)]
    input: Vec<PathBuf>,
    #[command(flatten)]
    params: super::read::Parameters,
    /// Render catalog data as dzn instead of the instance listing.
    #[arg(long)]
    dzn: bool,
}
pub fn run(arguments: Arguments) -> u8 {
    match list(&arguments) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("{error}");
            1
        }
    }
}
fn list(arguments: &Arguments) -> Result<(), Box<dyn std::error::Error>> {
    let source =
        std::fs::read(&arguments.spec).map_err(|e| format!("{}: {e}", arguments.spec.display()))?;
    let spec = Spec::parse(&source, &arguments.spec.to_string_lossy())?;
    let params = arguments.params.params();
    let mut out = io::stdout().lock();
    for input in &arguments.input {
        let catalog = spec.catalog_path(input, &params)?;
        if arguments.dzn {
            catalog.write_dzn(&mut out)?;
        } else {
            write_catalog(&mut out, &input.to_string_lossy(), &catalog)?;
        }
    }
    Ok(())
}
fn write_catalog(out: &mut impl Write, path: &str, catalog: &Data) -> io::Result<()> {
    let count = catalog.int("instances").expect("catalog count");
    writeln!(
        out,
        "{path}: {count} instance{}",
        if count == 1 { "" } else { "s" }
    )?;
    let names = catalog.entries().iter().any(|e| e.name() == "name");
    let failed = catalog.entries().iter().any(|e| e.name() == "failed");
    if !names && !failed {
        return Ok(());
    }
    for index in 0..count as usize {
        let message = if failed {
            catalog
                .value("failed", &[index])
                .expect("catalog failure")
                .as_str()
                .expect("string")
        } else {
            ""
        };
        if !names && message.is_empty() {
            continue;
        }
        write!(out, "  {}", index + 1)?;
        if names {
            write!(
                out,
                " {}",
                catalog
                    .value("name", &[index])
                    .expect("catalog name")
                    .as_str()
                    .expect("string")
            )?;
        }
        if !message.is_empty() {
            write!(out, " (failed: {message})")?;
        }
        writeln!(out)?;
    }
    Ok(())
}
