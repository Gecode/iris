//! `iris gen`: generate code from a reader spec.
use crate::{CheckFailure, CheckedSpec, check_spec, render_error};
use clap::{
    Arg, Args, Command as ClapCommand, Subcommand,
    builder::{PossibleValue, PossibleValuesParser, TypedValueParser},
    error::{ContextKind, ContextValue, Error, ErrorKind},
};
use gecode_iris_gen::{Language, ViewError, builder_code, view_code};
use std::ffi::OsStr;
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true, subcommand_negates_reqs = true)]
pub(super) struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
    /// The language of the handles.
    #[arg(long, required = true, value_parser = ViewLanguageParser)]
    lang: Option<Language>,
    /// Print a module of typed handles for this reader spec.
    #[arg(required = true)]
    spec: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Print a program that builds the reader spec with the spec builder.
    Builder {
        /// The language of the program.
        #[arg(long, value_parser = language_parser())]
        lang: Language,
        spec: PathBuf,
    },
}

/// A parser whose values are the languages that have a builder backend.
fn language_parser() -> impl TypedValueParser<Value = Language> {
    PossibleValuesParser::new(Language::ALL.iter().map(|language| language.name())).map(|name| {
        name.parse()
            .expect("the possible values are the languages' names")
    })
}

/// A parser whose values are the languages that have a view printer. Another
/// language with a builder backend gets an error that says so.
#[derive(Clone)]
struct ViewLanguageParser;

impl TypedValueParser for ViewLanguageParser {
    type Value = Language;

    fn parse_ref(
        &self,
        command: &ClapCommand,
        argument: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Language, Error> {
        let name = value.to_string_lossy();
        match name.parse::<Language>() {
            Ok(language) if language.has_view() => Ok(language),
            Ok(language) => {
                let message = format!(
                    "{} has no view generator yet; `iris gen builder --lang {} SPEC` \
                     prints a builder program\n",
                    language.name(),
                    language.name()
                );
                Err(Error::raw(ErrorKind::InvalidValue, message).with_cmd(command))
            }
            Err(_) => {
                let mut error = Error::new(ErrorKind::InvalidValue).with_cmd(command);
                if let Some(argument) = argument {
                    let argument = ContextValue::String(argument.to_string());
                    error.insert(ContextKind::InvalidArg, argument);
                }
                error.insert(ContextKind::InvalidValue, ContextValue::String(name.into()));
                let valid = view_languages().map(|language| language.name().to_owned());
                error.insert(
                    ContextKind::ValidValue,
                    ContextValue::Strings(valid.collect()),
                );
                Err(error)
            }
        }
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(
            view_languages().map(|language| PossibleValue::new(language.name())),
        ))
    }
}

fn view_languages() -> impl Iterator<Item = Language> {
    Language::ALL
        .iter()
        .copied()
        .filter(|language| language.has_view())
}

pub(super) fn run(arguments: Arguments) -> u8 {
    match (arguments.command, arguments.lang, arguments.spec) {
        (Some(Command::Builder { lang, spec }), _, _) => print_code(&spec, |checked| {
            builder_code(&checked.parsed, &checked.analysis, lang).map_err(ViewError::Spec)
        }),
        (None, Some(lang), Some(spec)) => print_code(&spec, |checked| {
            view_code(&checked.parsed, &checked.analysis, lang)
        }),
        (None, _, _) => unreachable!("clap requires --lang and SPEC without a subcommand"),
    }
}

/// Prints what `generate` makes of the spec at `path`, once the spec is checked.
fn print_code(
    path: &std::path::Path,
    generate: impl FnOnce(&CheckedSpec) -> Result<String, ViewError>,
) -> u8 {
    let origin = path.to_string_lossy();
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("{origin}: {error}");
            return 1;
        }
    };
    let checked = match check_spec(&bytes, &origin) {
        Ok(checked) => checked,
        Err(CheckFailure::Language(error)) => {
            eprintln!("{}", render_error(&error, &bytes));
            return 1;
        }
        Err(error) => {
            eprintln!("{origin}: {error}");
            return 1;
        }
    };
    let code = match generate(&checked) {
        Ok(code) => code,
        Err(ViewError::Spec(error)) => {
            eprintln!("{}", render_error(&error, &bytes));
            return 1;
        }
        Err(error) => {
            eprintln!("iris gen: {error}");
            return 1;
        }
    };
    match io::stdout().write_all(code.as_bytes()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("iris gen: {error}");
            1
        }
    }
}
