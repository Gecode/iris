use iris_cli::{CheckFailure, check_spec, protocol};
use iris_core::{Failure, Params, Spec};
use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

enum Operation {
    Parse,
    Read { input: PathBuf },
    Catalog { input: PathBuf },
    Dzn { input: PathBuf },
}
struct Request {
    operation: Operation,
    spec: PathBuf,
    params: Params,
    dzn: bool,
}
fn parse_request() -> Result<Request, String> {
    let mut args = std::env::args_os().skip(1);
    let operation = args.next().ok_or("expected parse, read, catalog, or dzn")?;
    if operation != "parse" && operation != "read" && operation != "catalog" && operation != "dzn" {
        return Err("only parse, read, catalog, and dzn operations are supported".into());
    }
    let spec = required_path(&mut args, "a reader-spec path")?;
    let operation = if operation == "read" {
        Operation::Read {
            input: required_path(&mut args, "an input path")?,
        }
    } else if operation == "catalog" {
        Operation::Catalog {
            input: required_path(&mut args, "an input path")?,
        }
    } else if operation == "dzn" {
        Operation::Dzn {
            input: required_path(&mut args, "an input path")?,
        }
    } else {
        Operation::Parse
    };
    let mut version_seen = false;
    let mut params_seen = false;
    let mut format_seen = false;
    let mut params = Params::new();
    let mut dzn = matches!(operation, Operation::Dzn { .. });
    while let Some(option) = args.next() {
        let option = option.to_str().ok_or("options must be Unicode")?;
        let seen = match option {
            "--version" => &mut version_seen,
            "--params" => &mut params_seen,
            "--format" if matches!(operation, Operation::Catalog { .. }) => &mut format_seen,
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
            "--format" => {
                dzn = match value {
                    "json" => false,
                    "dzn" => true,
                    _ => return Err("expected catalog format json or dzn".into()),
                };
            }
            "--version" if value != "0.1" => {
                return Err(format!(
                    "unsupported language version {value}; supported: 0.1"
                ));
            }
            "--params" => {
                if matches!(operation, Operation::Parse) {
                    protocol::validate_parse_params(value)
                        .map_err(|e| format!("invalid parse params: {e}"))?;
                } else {
                    params = protocol::parse_params(value)
                        .map_err(|e| format!("invalid params: {e}"))?;
                }
            }
            _ => {}
        }
    }
    Ok(Request {
        operation,
        spec,
        params,
        dzn,
    })
}
fn required_path(
    args: &mut impl Iterator<Item = OsString>,
    description: &str,
) -> Result<PathBuf, String> {
    args.next()
        .filter(|p| !p.to_string_lossy().starts_with("--"))
        .map(PathBuf::from)
        .ok_or_else(|| format!("operation requires {description}"))
}
fn read_document(request: &Request, bytes: &[u8]) -> Result<(u8, Vec<u8>), (u8, String)> {
    let input = match &request.operation {
        Operation::Read { input } | Operation::Catalog { input } | Operation::Dzn { input } => {
            input
        }
        Operation::Parse => unreachable!("read dispatch"),
    };
    match Spec::parse(bytes, &request.spec.to_string_lossy()).and_then(|spec| {
        match request.operation {
            Operation::Catalog { .. } => spec.catalog_path(input, &request.params),
            _ => spec.read_path(input, &request.params),
        }
    }) {
        Ok(data) => {
            let document = if request.dzn {
                data.to_dzn().map_err(|e| (3, e.to_string()))?
            } else {
                data.to_json().map_err(|e| (3, e.to_string()))?
            };
            Ok((0, document))
        }
        Err(Failure::Language(error)) => Ok((1, error.to_json().map_err(|e| (3, e.to_string()))?)),
        Err(Failure::Host(error)) => Err((3, error.to_string())),
        Err(Failure::Unsupported(error)) => Err((2, error.to_string())),
    }
}
fn run() -> Result<u8, (u8, String)> {
    let request = parse_request().map_err(|e| (2, e))?;
    let bytes = std::fs::read(&request.spec)
        .map_err(|e| (3, format!("{}: {e}", request.spec.display())))?;
    let (status, document) = match request.operation {
        Operation::Parse => match check_spec(&bytes, &request.spec.to_string_lossy()) {
            Ok(_) => (0, protocol::PARSE_SUCCESS.to_vec()),
            Err(CheckFailure::Language(error)) => (
                1,
                protocol::error_json(&error).map_err(|e| (3, e.to_string()))?,
            ),
            Err(error) => return Err((3, error.to_string())),
        },
        Operation::Read { .. } | Operation::Catalog { .. } | Operation::Dzn { .. } => {
            read_document(&request, &bytes)?
        }
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
