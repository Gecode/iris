//! Count analysis operations in scaling tests, without timing thresholds.
#[cfg(test)]
std::thread_local! {
    static OPERATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[inline]
pub(crate) fn record() {
    #[cfg(test)]
    OPERATIONS.with(|operations| operations.set(operations.get() + 1));
}

#[cfg(test)]
mod tests {
    use super::OPERATIONS;
    use crate::{DefinitionId, Resolution, ValueFacts, ValueType};
    use iris_syntax::LanguageVersion;

    fn repetition_work(statement: &str, repetitions: usize) -> usize {
        let source = if statement == "x:int\n" {
            statement.repeat(repetitions)
        } else {
            format!("let x=0\n{}let result=x\n", statement.repeat(repetitions))
        };
        let parsed =
            iris_syntax::parse(source.as_bytes(), "scaling", LanguageVersion::V0_1).unwrap();
        OPERATIONS.with(|operations| operations.set(0));
        let analysis = crate::analyze(&parsed);
        let work = OPERATIONS.with(|operations| operations.get());
        assert!(analysis.is_valid(), "{:?}", analysis.diagnostics);
        if statement == "if condition {let x=1}\n" {
            let reference = analysis.model.references().last().unwrap();
            let expected: Vec<_> = (0..=repetitions).rev().map(DefinitionId).collect();
            assert_eq!(reference.resolution.definitions(), expected);
            assert!(matches!(
                reference.resolution,
                Resolution::Value {
                    possibly_unavailable: false,
                    ..
                }
            ));
            assert_eq!(reference.facts, ValueFacts::known(ValueType::Int, 0));
        }
        work
    }

    #[test]
    fn repeated_definitions_scale_linearly() {
        for statement in [
            "x:int\n",
            "if condition {let x=1}\n",
            "if condition {let y=1}\n",
            "if condition {if other {let x=1}} else {if other {let x=2}}\n",
        ] {
            let small = repetition_work(statement, 256);
            let large = repetition_work(statement, 1024);
            println!("{statement:?}: N=256 {small} operations; N=1024 {large} operations");
            // Four times the input allows six times the lookup/union work.
            // Quadratic scans and cubic target deduplication exceed this budget.
            assert!(large <= small * 6, "{statement:?}: {small} -> {large}");
        }
    }
}

#[cfg(test)]
mod container_tests {
    use super::OPERATIONS;
    use iris_syntax::LanguageVersion;

    fn source(family: &str, n: usize) -> String {
        match family {
            "distinct" => (0..n)
                .map(|i| format!("if condition {{let a_{i}=1}}\n"))
                .collect(),
            "nested" => (0..n)
                .step_by(16)
                .map(|start| {
                    let end = (start + 16).min(n);
                    let mut source: String = (start..end)
                        .map(|i| format!("if condition {{let a_{i}=1\n"))
                        .collect();
                    source.push_str(&"}\n".repeat(end - start));
                    source
                })
                .collect(),
            "repeats" => (0..n)
                .map(|i| format!("repeat 1 as rows_{i} {{let a_{i}=1}}\n"))
                .collect(),
            "sections" => (0..n)
                .map(|i| format!("sections {{\"A\" optional {{let a_{i}=1}}}}\n"))
                .collect(),
            "stores" => (0..n)
                .map(|i| format!("sections until \"END\" store a_{i} {{\"A\" optional}}\n"))
                .collect(),
            "types" => (0..n)
                .map(|i| format!("type t_{i}=int\na_{i}:t_{i}\n"))
                .collect(),
            "branch types" => (0..n)
                .map(|i| format!("if condition {{type t=int a_{i}:t}}\n"))
                .collect(),
            _ => unreachable!(),
        }
    }
    fn work(family: &str, n: usize) -> usize {
        let source = source(family, n);
        let parsed =
            iris_syntax::parse(source.as_bytes(), "scaling", LanguageVersion::V0_1).unwrap();
        OPERATIONS.with(|count| count.set(0));
        let analysis = crate::analyze(&parsed);
        assert!(analysis.is_valid(), "{:?}", analysis.diagnostics);
        OPERATIONS.with(|count| count.get())
    }
    #[test]
    fn containers_scale_with_changed_names() {
        for family in [
            "distinct",
            "nested",
            "repeats",
            "sections",
            "stores",
            "types",
            "branch types",
        ] {
            let small = work(family, 256);
            let large = work(family, 1024);
            println!("{family}: {small} -> {large}");
            assert!(large <= small * 6, "{family}: {small} -> {large}");
        }
    }
}
