//! Input reading, expression evaluation, and owned column data for Iris 0.1.
mod cancellation;
mod dzn;
mod error;
mod eval;
mod input;
mod json;
mod numeric;
mod reader;
pub mod summary;
mod value;

pub use cancellation::{CancellationToken, ReadOptions};
pub use error::{Error, ErrorCode, InputSpan, Location};
use iris_sema::Analysis;
pub use iris_syntax::LanguageVersion;
use iris_syntax::{Parse, ParseError};
use std::{collections::HashMap, fmt, path::Path};
pub use value::{AccessError, Data, Entry, Value, ValueRef};

#[derive(Debug)]
pub struct Spec {
    parsed: Parse,
    analysis: Analysis,
    source: Vec<u8>,
    index: reader::ModelIndex,
}
impl Spec {
    pub fn parse(source: &[u8], origin: &str) -> Result<Self, Failure> {
        Self::parse_version(source, origin, LanguageVersion::V0_1)
    }

    pub fn parse_version(
        source: &[u8],
        origin: &str,
        version: LanguageVersion,
    ) -> Result<Self, Failure> {
        let parsed = iris_syntax::parse(source, origin, version).map_err(|error| match error {
            ParseError::InvalidUtf8(diagnostic) => {
                Failure::Language(Error::from_spec(&iris_sema::Error::from(&diagnostic)))
            }
            ParseError::SourceTooLarge { len } => Failure::Host(HostError::SourceTooLarge { len }),
        })?;
        let analysis = iris_sema::analyze(&parsed);
        if let Some(error) = analysis.first_error() {
            return Err(Error::from_spec(error).into());
        }
        let index = reader::ModelIndex::new(&analysis.model, parsed.syntax());
        Ok(Self {
            parsed,
            analysis,
            source: source.to_vec(),
            index,
        })
    }

    /// The reader spec's format name, if declared.
    pub fn format_name(&self) -> Option<String> {
        self.parsed.format_name()
    }

    pub fn read(&self, input: &[u8], origin: &str, params: &Params) -> Result<Data, Failure> {
        self.read_with_options(input, origin, params, &ReadOptions::default())
    }

    pub fn read_with_options(
        &self,
        input: &[u8],
        origin: &str,
        params: &Params,
        options: &ReadOptions,
    ) -> Result<Data, Failure> {
        let result = (|| {
            let control = cancellation::Control::new(options);
            control.check()?;
            self.admit_params(params, &control)?;
            let input = self.controlled_input(input, origin, control)?;
            reader::Interpreter::new(self, input)?
                .with_params(params)?
                .read()
        })();
        Self::finish_read(result, options)
    }

    fn finish_read(result: Result<Data, Failure>, options: &ReadOptions) -> Result<Data, Failure> {
        if matches!(result, Err(Failure::Language(_)))
            && options
                .cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
        {
            return Err(HostError::Cancelled.into());
        }
        result
    }

    /// Execute every instance and return its count, usable names, and rejections.
    pub fn catalog(&self, input: &[u8], origin: &str, params: &Params) -> Result<Data, Failure> {
        reader::Interpreter::new(self, self.input(input, origin, params)?)?
            .with_params(params)?
            .catalog()
    }

    pub fn catalog_path(&self, path: impl AsRef<Path>, params: &Params) -> Result<Data, Failure> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(HostError::Io)?;
        self.catalog(&bytes, &path.to_string_lossy(), params)
    }

    fn input<'a>(
        &self,
        bytes: &'a [u8],
        origin: &'a str,
        params: &Params,
    ) -> Result<input::Input<'a>, Failure> {
        let control = cancellation::Control::default();
        self.admit_params(params, &control)?;
        self.controlled_input(bytes, origin, control)
    }

    fn controlled_input<'a>(
        &self,
        bytes: &'a [u8],
        origin: &'a str,
        control: cancellation::Control,
    ) -> Result<input::Input<'a>, Failure> {
        let mut input = input::Input::new(bytes, origin);
        input.control = control;
        if let Err(offset) = input.control.utf8(bytes)? {
            // Advance once rather than rescanning prefixes for a location.
            input.advance_to(offset)?;
            return Err(Error::input(
                ErrorCode::Utf8,
                "input is not valid UTF-8",
                &input,
                input.position(),
                None,
                None,
            )
            .into());
        }
        Ok(input)
    }

    pub fn read_path(&self, path: impl AsRef<Path>, params: &Params) -> Result<Data, Failure> {
        let path = path.as_ref();
        let bytes = std::fs::read(path).map_err(HostError::Io)?;
        self.read(&bytes, &path.to_string_lossy(), params)
    }

    fn admit_params(
        &self,
        params: &Params,
        control: &cancellation::Control,
    ) -> Result<(), Failure> {
        for (name, _) in &params.pairs {
            control.check()?;
            if !self.index.params.contains(name) {
                return Err(Error::unknown_param(name, self.parsed.origin()).into());
            }
        }
        Ok(())
    }
}

/// Caller parameters retain supplied order and exact string values.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Params {
    pairs: Vec<(String, String)>,
    positions: HashMap<String, usize>,
}
impl Params {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let name = name.into();
        let value = value.into();
        if let Some(&index) = self.positions.get(&name) {
            self.pairs[index].1 = value;
        } else {
            self.positions.insert(name.clone(), self.pairs.len());
            self.pairs.push((name, value));
        }
    }
    pub fn get(&self, name: &str) -> Option<&str> {
        self.positions
            .get(name)
            .map(|&index| self.pairs[index].1.as_str())
    }
    pub fn pairs(&self) -> &[(String, String)] {
        &self.pairs
    }
}

#[derive(Debug)]
pub enum Failure {
    Language(Error),
    Host(HostError),
    Unsupported(UnsupportedFeature),
}
#[derive(Debug)]
pub enum HostError {
    Cancelled,
    Io(std::io::Error),
    SourceTooLarge { len: usize },
}
#[derive(Debug)]
pub struct UnsupportedFeature {
    pub feature: &'static str,
    pub task: &'static str,
    pub range: iris_syntax::TextRange,
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Language(error)
    }
}
impl From<HostError> for Failure {
    fn from(error: HostError) -> Self {
        Self::Host(error)
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Language(e) => e.fmt(f),
            Self::Host(e) => e.fmt(f),
            Self::Unsupported(e) => e.fmt(f),
        }
    }
}
impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("reading cancelled by host"),
            Self::Io(e) => e.fmt(f),
            Self::SourceTooLarge { len } => {
                write!(f, "source has {len} bytes, exceeding rowan's u32 capacity")
            }
        }
    }
}
impl fmt::Display for UnsupportedFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not implemented yet ({})", self.feature, self.task)
    }
}
impl std::error::Error for Failure {}
impl std::error::Error for HostError {}
impl std::error::Error for UnsupportedFeature {}
