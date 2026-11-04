//! Greedy reduction of a failing case: delete spec lines, then input lines,
//! while the case still fails.

use super::GeneratedCase;

/// Reduce `case` by deleting runs of spec lines, then of input lines,
/// keeping each deletion after which `fails` still holds. Runs halve from
/// half the lines down to single lines. `fails` is called at most `budget`
/// times.
pub(super) fn reduce(
    case: &GeneratedCase,
    budget: usize,
    mut fails: impl FnMut(&GeneratedCase) -> bool,
) -> GeneratedCase {
    let mut reduced = case.clone();
    let mut remaining = budget;
    for part in [Part::Spec, Part::Input] {
        let mut lines = split_lines(part.bytes(&reduced));
        let mut run = lines.len().div_ceil(2).max(1);
        loop {
            let mut start = 0;
            while start < lines.len() {
                if remaining == 0 {
                    return reduced;
                }
                remaining -= 1;
                let end = (start + run).min(lines.len());
                let mut candidate = reduced.clone();
                *part.bytes_mut(&mut candidate) =
                    [&lines[..start], &lines[end..]].concat().concat();
                if fails(&candidate) {
                    lines.drain(start..end);
                    reduced = candidate;
                } else {
                    start = end;
                }
            }
            if run == 1 {
                break;
            }
            run = run.div_ceil(2);
        }
    }
    reduced
}

#[derive(Clone, Copy)]
enum Part {
    Spec,
    Input,
}

impl Part {
    fn bytes(self, case: &GeneratedCase) -> &[u8] {
        match self {
            Self::Spec => &case.spec,
            Self::Input => &case.input,
        }
    }

    fn bytes_mut(self, case: &mut GeneratedCase) -> &mut Vec<u8> {
        match self {
            Self::Spec => &mut case.spec,
            Self::Input => &mut case.input,
        }
    }
}

/// The lines of `bytes`, each with its LF if it has one.
fn split_lines(bytes: &[u8]) -> Vec<Vec<u8>> {
    bytes
        .split_inclusive(|&byte| byte == b'\n')
        .map(<[u8]>::to_vec)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::differential::Strategy;

    #[test]
    fn keeps_only_the_lines_the_failure_needs() {
        let case = GeneratedCase {
            strategy: Strategy::Generate,
            spec: b"a\nb\nbad\nc\nd\n".to_vec(),
            input: b"1\n2\n3\n".to_vec(),
            params: None,
        };
        let reduced = reduce(&case, 500, |candidate| {
            candidate.spec.windows(3).any(|window| window == b"bad")
        });
        assert_eq!(reduced.spec, b"bad\n");
        assert!(reduced.input.is_empty());
    }

    #[test]
    fn stops_at_the_budget() {
        let case = GeneratedCase {
            strategy: Strategy::Generate,
            spec: b"a\nb\nc\nd\n".to_vec(),
            input: Vec::new(),
            params: None,
        };
        let mut calls = 0;
        reduce(&case, 3, |_| {
            calls += 1;
            false
        });
        assert_eq!(calls, 3);
    }
}
