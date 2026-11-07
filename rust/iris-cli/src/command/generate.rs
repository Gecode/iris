//! `iris gen`: generate code from a reader spec.
use crate::{CheckFailure, CheckedSpec, check_spec, render_error};
use clap::{Args, Subcommand, builder::PossibleValuesParser, builder::TypedValueParser};
use gecode_iris_gen::{Language, builder_code, view_code};
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
    #[arg(long, required = true, value_parser = language_parser())]
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

/// A parser whose values are the languages that have a backend.
fn language_parser() -> impl TypedValueParser<Value = Language> {
    PossibleValuesParser::new(Language::ALL.iter().map(|language| language.name())).map(|name| {
        name.parse()
            .expect("the possible values are the languages' names")
    })
}

pub(super) fn run(arguments: Arguments) -> u8 {
    match (arguments.command, arguments.lang, arguments.spec) {
        (Some(Command::Builder { lang, spec }), _, _) => print_code(&spec, |checked| {
            builder_code(&checked.parsed, &checked.analysis, lang)
        }),
        (None, Some(lang), Some(spec)) => {
            print_code(&spec, |checked| view_code(&checked.analysis, lang))
        }
        (None, _, _) => unreachable!("clap requires --lang and SPEC without a subcommand"),
    }
}

/// Prints what `generate` makes of the spec at `path`, once the spec is checked.
fn print_code(
    path: &std::path::Path,
    generate: impl FnOnce(&CheckedSpec) -> Result<String, gecode_iris_sema::Error>,
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
    let code = generate(&checked).expect("a spec that passed the check has no first error");
    match io::stdout().write_all(code.as_bytes()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("iris gen: {error}");
            1
        }
    }
}
