//! What `build()` reports.
use gecode_iris_core::Failure;
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
}

impl Issue {
    pub fn new(kind: IssueKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
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

    pub(crate) fn from_failure(failure: Failure, text: String) -> Self {
        Self {
            message: failure.to_string(),
            kind: BuildErrorKind::Spec(failure),
            text: Some(text),
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
        }
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.failure().map(|failure| failure as _)
    }
}
