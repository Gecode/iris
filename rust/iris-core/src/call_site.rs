//! Where the statements of a built reader spec came from in the host program.
use crate::Failure;

/// A place in the host program, as `#[track_caller]` reports it.
pub type CallSite = &'static std::panic::Location<'static>;

#[derive(Debug, Clone, Copy)]
struct StatementSite {
    start: usize,
    site: CallSite,
}

/// The call site of each statement in a spec's text, sorted by offset.
///
/// A spec builder fills this in as it prints. A statement runs up to the next
/// one, so every offset after the first statement's start has a call site. A
/// spec parsed from text has none.
#[derive(Debug, Clone, Default)]
pub struct CallSites {
    statements: Vec<StatementSite>,
}

impl CallSites {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the statement starting at byte `start`. Statements arrive in text
    /// order.
    pub fn push(&mut self, start: usize, site: CallSite) {
        debug_assert!(
            self.statements
                .last()
                .is_none_or(|last| last.start <= start)
        );
        self.statements.push(StatementSite { start, site });
    }

    /// The call site of the statement that holds byte `offset`.
    pub fn at(&self, offset: usize) -> Option<CallSite> {
        let after = self
            .statements
            .partition_point(|statement| statement.start <= offset);
        Some(self.statements.get(after.checked_sub(1)?)?.site)
    }

    /// Add the call site of the statement a language failure belongs to, or of
    /// the statement it points at when that is not known. The failure keeps its
    /// code and its location in the text.
    pub fn locate(&self, mut failure: Failure) -> Failure {
        if let Failure::Language(error) = &mut failure {
            let statement = error
                .details
                .owner
                .or_else(|| Some(error.spec_span()?.start()));
            error.details.call_site = statement.and_then(|start| self.at(start.into()));
        }
        failure
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::Location;

    #[track_caller]
    fn here() -> CallSite {
        Location::caller()
    }

    #[test]
    fn an_offset_belongs_to_the_last_statement_at_or_before_it() {
        let (first, second) = (here(), here());
        let mut sites = CallSites::new();
        sites.push(2, first);
        sites.push(10, second);
        assert_eq!(sites.at(0), None);
        assert_eq!(sites.at(1), None);
        assert_eq!(sites.at(2), Some(first));
        assert_eq!(sites.at(9), Some(first));
        assert_eq!(sites.at(10), Some(second));
        assert_eq!(sites.at(usize::MAX), Some(second));
    }

    #[test]
    fn no_statements_have_no_call_site() {
        assert_eq!(CallSites::new().at(0), None);
    }
}
