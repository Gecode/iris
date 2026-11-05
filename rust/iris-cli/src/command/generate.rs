//! `iris gen`: generate code from a reader spec.
use crate::{CheckFailure, check_spec, render_error};
use clap::{Args, Subcommand, builder::PossibleValuesParser, builder::TypedValueParser};
use gecode_iris_gen::{Language, builder_code};
use std::{
    io::{self, Write},
    path::PathBuf,
};

#[derive(Args)]
pub(super) struct Arguments {
    #[command(subcommand)]
    command: Command,
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
    match arguments.command {
        Command::Builder { lang, spec } => builder(lang, &spec),
    }
}

fn builder(language: Language, spec: &std::path::Path) -> u8 {
    let origin = spec.to_string_lossy();
    let bytes = match std::fs::read(spec) {
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
    let code = builder_code(&checked.parsed, &checked.analysis, language)
        .expect("a spec that passed the check has no first error");
    match io::stdout().write_all(code.as_bytes()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("iris gen: {error}");
            1
        }
    }
}
