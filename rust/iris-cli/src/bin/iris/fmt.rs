use iris_cli::render_error;
use iris_syntax::{FormatError, FormatOptions, LanguageVersion, ParseError};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(clap::Args)]
pub struct Arguments {
    /// Files or directories to format; use - alone for standard input.
    #[arg(required = true, num_args = 1..)]
    paths: Vec<PathBuf>,
    /// Leave files unchanged and exit 1 if formatting is needed.
    #[arg(long)]
    check: bool,
    /// Print a unified diff instead of writing files.
    #[arg(long)]
    diff: bool,
    /// Use this path for standard input's .editorconfig lookup.
    #[arg(long, value_name = "PATH")]
    stdin_filepath: Option<PathBuf>,
}

pub fn run(arguments: Arguments) -> ExitCode {
    let has_stdin = arguments.paths.iter().any(|path| path == Path::new("-"));
    if (has_stdin && arguments.paths.len() != 1)
        || (!has_stdin && arguments.stdin_filepath.is_some())
    {
        eprintln!("iris fmt: - must be the only operand; --stdin-filepath requires -");
        return ExitCode::from(2);
    }
    let mut stdout = io::stdout().lock();
    let mut success = true;
    if has_stdin {
        let origin = arguments
            .stdin_filepath
            .as_deref()
            .unwrap_or(Path::new("<stdin>"));
        let result = (|| {
            let mut bytes = Vec::new();
            io::stdin()
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())?;
            format_source(
                &bytes,
                origin,
                arguments.stdin_filepath.as_deref(),
                true,
                &arguments,
                &mut stdout,
            )
        })();
        success = report(origin, result);
    } else {
        for path in &arguments.paths {
            success &= format_path(path, &arguments, &mut stdout);
        }
    }
    if let Err(error) = stdout.flush() {
        eprintln!("iris fmt: {error}");
        success = false;
    }
    ExitCode::from(u8::from(!success))
}

fn format_path(path: &Path, arguments: &Arguments, stdout: &mut impl Write) -> bool {
    let result = (|| {
        let kind = fs::symlink_metadata(path)
            .map_err(|error| error.to_string())?
            .file_type();
        if kind.is_dir() {
            let mut success = true;
            let mut entries = Vec::new();
            for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
                match entry {
                    Ok(entry) => entries.push(entry.path()),
                    Err(error) => success &= report(path, Err(error.to_string())),
                }
            }
            entries.sort();
            for entry in entries {
                // symlink_metadata in the recursive call skips symbolic links.
                // Explicit file operands need not have the .iris suffix.
                if entry.is_dir()
                    || entry
                        .file_name()
                        .is_some_and(|name| name.as_encoded_bytes().ends_with(b".iris"))
                {
                    success &= format_path(&entry, arguments, stdout);
                }
            }
            return Ok(success);
        }
        if kind.is_symlink() {
            return Ok(true);
        }
        if !kind.is_file() {
            return Err("not a regular file or directory".into());
        }
        let bytes = fs::read(path).map_err(|error| error.to_string())?;
        format_source(&bytes, path, Some(path), false, arguments, stdout)
    })();
    report(path, result)
}

fn report(path: &Path, result: Result<bool, String>) -> bool {
    match result {
        Ok(success) => success,
        Err(error) => {
            eprintln!("{}: {error}", path.display());
            false
        }
    }
}

fn format_source(
    bytes: &[u8],
    origin: &Path,
    config_path: Option<&Path>,
    is_stdin: bool,
    arguments: &Arguments,
    stdout: &mut impl Write,
) -> Result<bool, String> {
    let options = match config_path {
        Some(path) => FormatOptions::for_path(path)?,
        None => FormatOptions::default(),
    };
    let origin_text = origin.to_string_lossy();
    let formatted = match iris_syntax::format(bytes, &origin_text, LanguageVersion::V0_1, options) {
        Ok(formatted) => formatted,
        Err(error) => {
            eprintln!("{}", render_format_error(error, bytes, &origin_text));
            return Ok(false);
        }
    };
    let changed = formatted.as_bytes() != bytes;
    if arguments.diff {
        if changed {
            let source = std::str::from_utf8(bytes).expect("formatter validated UTF-8");
            let diff = similar::TextDiff::from_lines(source, &formatted);
            write!(
                stdout,
                "{}",
                // A tab ends each header file name, so patch reads names with spaces.
                diff.unified_diff().header(
                    &format!("{origin_text}\toriginal"),
                    &format!("{origin_text}\tformatted")
                )
            )
            .map_err(|error| error.to_string())?;
        }
    } else if !arguments.check {
        if is_stdin {
            stdout
                .write_all(formatted.as_bytes())
                .map_err(|error| error.to_string())?;
        } else if changed {
            replace_file(origin, formatted.as_bytes()).map_err(|error| error.to_string())?;
        }
    }
    if arguments.check && changed {
        eprintln!("{}: would reformat", origin.display());
    }
    Ok(!(arguments.check && changed))
}

/// Writes `contents` to a sibling temporary file and renames it over `path`,
/// so a failed write leaves the original file intact.
fn replace_file(path: &Path, contents: &[u8]) -> io::Result<()> {
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    name.push(format!(".iris-fmt-{}.tmp", std::process::id()));
    let temporary = path.with_file_name(name);
    let permissions = fs::metadata(path)?.permissions();
    // Created outside the cleanup scope: an existing file of the same name is
    // someone else's and must survive the failure.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(contents)?;
        file.sync_all()?;
        fs::set_permissions(&temporary, permissions)?;
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn render_format_error(error: FormatError, bytes: &[u8], origin: &str) -> String {
    match error {
        FormatError::Source(ParseError::InvalidUtf8(diagnostic)) => {
            render_error(&iris_sema::Error::from(&diagnostic), bytes)
        }
        FormatError::Syntax(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| render_error(&iris_sema::Error::from(diagnostic), bytes))
            .collect::<Vec<_>>()
            .join("\n"),
        other => format!("{origin}: {other}"),
    }
}
