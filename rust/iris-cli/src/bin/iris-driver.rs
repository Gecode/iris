use iris_cli::{CheckFailure, check_spec, protocol};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};
struct Request {
    spec: PathBuf,
}
fn parse_request() -> Result<Request, String> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("parse")) {
        return Err("only the parse operation is supported".into());
    }
    let spec = args
        .next()
        .filter(|p| !p.to_string_lossy().starts_with("--"))
        .ok_or("parse requires a reader-spec path")?;
    let mut version_seen = false;
    let mut params_seen = false;
    while let Some(option) = args.next() {
        let option = option.to_str().ok_or("options must be Unicode")?;
        let seen = match option {
            "--version" => &mut version_seen,
            "--params" => &mut params_seen,
            _ => return Err(format!("unexpected argument {option}")),
        };
        if *seen {
            return Err(format!("duplicate option {option}"));
        }
        *seen = true;
        let value: OsString = args
            .next()
            .ok_or_else(|| format!("missing value for {option}"))?;
        let value = value.to_str().ok_or("option values must be Unicode")?;
        match option {
            "--version" if value != "0.1" => {
                return Err(format!(
                    "unsupported language version {value}; supported: 0.1"
                ));
            }
            "--params" => protocol::validate_parse_params(value)
                .map_err(|e| format!("invalid parse params: {e}"))?,
            _ => {}
        }
    }
    Ok(Request { spec: spec.into() })
}
fn run() -> Result<u8, (u8, String)> {
    let request = parse_request().map_err(|e| (2, e))?;
    let bytes = std::fs::read(&request.spec)
        .map_err(|e| (3, format!("{}: {e}", request.spec.display())))?;
    let (status, document) = match check_spec(&bytes, &request.spec.to_string_lossy()) {
        Ok(_) => (0, protocol::PARSE_SUCCESS.to_vec()),
        Err(CheckFailure::Language(error)) => (
            1,
            protocol::error_json(&error).map_err(|e| (3, e.to_string()))?,
        ),
        Err(error) => return Err((3, error.to_string())),
    };
    io::stdout()
        .lock()
        .write_all(&document)
        .map_err(|e| (3, e.to_string()))?;
    Ok(status)
}
fn main() -> ExitCode {
    match run() {
        Ok(status) => ExitCode::from(status),
        Err((status, message)) => {
            eprintln!("iris-driver: {message}");
            ExitCode::from(status)
        }
    }
}
