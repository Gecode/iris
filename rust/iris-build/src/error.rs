//! What `build()` reports.
use gecode_iris_core::{CallSite, CallSites, Error, Failure};
use std::fmt;

/// Which check of the builder failed. The language's own errors are
/// [`BuildErrorKind::Spec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IssueKind {
    Scope,
    Provenance,
    Redefinition,
    Unprintable,
    Binding,
}

/// A builder check that failed, recorded until `build()`.
#[derive(Debug, Clone)]
pub(crate) struct Issue {
    pub kind: IssueKind,
    pub message: String,
    pub site: CallSite,
}

impl Issue {
    pub fn new(kind: IssueKind, message: impl Into<String>, site: CallSite) -> Self {
        Self {
            kind,
            message: message.into(),
            site,
        }
    }
}

/// Why a reader spec could not be built.
#[derive(Debug)]
#[non_exhaustive]
pub enum BuildErrorKind {
    /// A statement went to a scope that was not the innermost open block.
    Scope,
    /// A handle came from another builder.
    Provenance,
    /// A name was defined again without its handle.
    Redefinition,
    /// A value has no spelling in Iris text.
    Unprintable,
    /// A capture or forward handle was not bound as it was declared.
    Binding,
    /// The emitted text failed in `Spec::parse`.
    Spec(Failure),
}

/// The first failure of a builder, or of parsing the text it emitted.
#[derive(Debug)]
pub struct BuildError {
    kind: BuildErrorKind,
    message: String,
    text: Option<String>,
    call_site: Option<CallSite>,
}

impl BuildError {
    pub fn kind(&self) -> &BuildErrorKind {
        &self.kind
    }

    /// The language failure, when the emitted text was rejected.
    pub fn failure(&self) -> Option<&Failure> {
        match &self.kind {
            BuildErrorKind::Spec(failure) => Some(failure),
            _ => None,
        }
    }

    /// The emitted text, when it was rejected.
    pub fn text(&self) -> Option<&str> {
        self.text.as_deref()
    }

    /// The host code of the statement that failed. A statement records the
    /// line that called it, so a helper needs `#[track_caller]` to pass on its
    /// own caller. A failure that names no statement has none.
    pub fn call_site(&self) -> Option<CallSite> {
        self.call_site
    }

    /// The line of emitted text that the language failure points at.
    pub fn emitted_line(&self) -> Option<&str> {
        let text = self.text.as_deref()?;
        let offset = usize::from(self.language_error()?.spec_span()?.start()).min(text.len());
        let start = text[..offset].rfind('\n').map_or(0, |newline| newline + 1);
        text[start..].lines().next()
    }

    fn language_error(&self) -> Option<&Error> {
        match self.failure()? {
            Failure::Language(error) => Some(error),
            _ => None,
        }
    }

    pub(crate) fn from_failure(failure: Failure, text: String, sites: &CallSites) -> Self {
        // The message comes first: the display adds the call site after the
        // emitted line.
        let message = failure.to_string();
        let failure = sites.locate(failure);
        let call_site = match &failure {
            Failure::Language(error) => error.call_site(),
            _ => None,
        };
        Self {
            message,
            kind: BuildErrorKind::Spec(failure),
            text: Some(text),
            call_site,
        }
    }
}

impl From<Issue> for BuildError {
    fn from(issue: Issue) -> Self {
        let kind = match issue.kind {
            IssueKind::Scope => BuildErrorKind::Scope,
            IssueKind::Provenance => BuildErrorKind::Provenance,
            IssueKind::Redefinition => BuildErrorKind::Redefinition,
            IssueKind::Unprintable => BuildErrorKind::Unprintable,
            IssueKind::Binding => BuildErrorKind::Binding,
        };
        Self {
            kind,
            message: issue.message,
            text: None,
            call_site: Some(issue.site),
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error = self.language_error();
        if let Some(error) = error {
            write!(f, "{}: ", error.code.as_str())?;
        }
        f.write_str(&self.message)?;
        let number = error.and_then(|error| error.spec_location()?.line);
        if let (Some(number), Some(line)) = (number, self.emitted_line()) {
            write!(f, "\n{number:>6} | {}", escape_controls(line))?;
        }
        if let Some(site) = self.call_site {
            write!(f, "\ncall site: {}", escape_controls(&site.to_string()))?;
        }
        Ok(())
    }
}

/// Emitted strings keep some control characters raw, and a file name can hold
/// them too. A terminal must not act on them.
fn escape_controls(line: &str) -> String {
    let mut escaped = String::with_capacity(line.len());
    for character in line.chars() {
        if character.is_control() {
            escaped.extend(character.escape_default());
        } else {
            escaped.push(character);
        }
    }
    escaped
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.failure().map(|failure| failure as _)
    }
}
