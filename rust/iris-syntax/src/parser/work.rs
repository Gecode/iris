//! Count type-name lookup and scope maintenance in scaling tests.
#[cfg(test)]
std::thread_local! {
    static OPERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[inline]
pub(super) fn record() {
    #[cfg(test)]
    OPERATIONS.with(|operations| operations.set(operations.get() + 1));
}

#[cfg(test)]
mod tests {
    use super::OPERATIONS;
    use crate::LanguageVersion;
    use std::fmt::Write;

    fn work(family: &str, n: usize) -> usize {
        let mut source = String::new();
        if family == "block" {
            source.push_str("repeat 1 {\n");
        }
        if family == "shadowing" {
            source.push_str("type t=int\n");
        }
        for i in 0..n {
            match family {
                "siblings" => writeln!(source, "repeat 1 {{type t_{i}=int a_{i}:t_{i}}}"),
                "shadowing" => writeln!(source, "repeat 1 {{type t=word a_{i}:t}} b_{i}:t"),
                _ => writeln!(source, "type t_{i}=int a_{i}:t_{i}"),
            }
            .unwrap();
        }
        if family == "block" {
            source.push_str("}\n");
        }
        OPERATIONS.with(|operations| operations.set(0));
        let parsed = crate::parse(source.as_bytes(), "scaling", LanguageVersion::V0_1).unwrap();
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        OPERATIONS.with(|operations| operations.get())
    }

    #[test]
    fn type_names_scale_linearly() {
        for family in ["top", "block", "siblings", "shadowing"] {
            let small = work(family, 256);
            let large = work(family, 1024);
            println!("{family}: N=256 {small} operations; N=1024 {large} operations");
            assert!(small > 0);
            // Four times the input allows six times the lookup/scope work.
            // Scanning all earlier declarations exceeds this budget.
            assert!(large <= small * 6, "{family}: {small} -> {large}");
        }
    }
}
