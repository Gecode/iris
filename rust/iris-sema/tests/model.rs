use gecode_iris_sema::*;
use gecode_iris_syntax::{LanguageVersion, TextSize};

fn analyze(source: &str) -> Analysis {
    let parsed =
        gecode_iris_syntax::parse(source.as_bytes(), "model", LanguageVersion::V0_1).unwrap();
    let analysis = gecode_iris_sema::analyze(&parsed);
    assert!(analysis.is_valid(), "{:?}", analysis.diagnostics);
    analysis
}
fn definition<'a>(model: &'a SemanticModel, name: &str) -> &'a Definition {
    model.definitions().iter().find(|d| d.name == name).unwrap()
}
fn reference<'a>(model: &'a SemanticModel, name: &str) -> &'a Reference {
    model
        .references()
        .iter()
        .rev()
        .find(|r| r.name == name)
        .unwrap()
}
fn depth(facts: &ValueFacts, expected: usize) {
    assert_eq!(facts.depths, Possibilities::one(expected));
}

#[test]
fn redefined_name_is_valid() {
    let source = "x:int let before=x x:float let after=x if condition {x:word} let joined=x";
    let analysis = analyze(source);
    let model = &analysis.model;
    let xs: Vec<_> = model
        .definitions()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.name == "x")
        .map(|(i, _)| DefinitionId(i))
        .collect();
    assert_eq!(xs.len(), 3);
    let refs: Vec<_> = model
        .references()
        .iter()
        .filter(|r| r.name == "x")
        .collect();
    assert_eq!(refs[0].resolution.definitions(), &xs[..1]);
    assert_eq!(refs[1].resolution.definitions(), &xs[1..2]);
    assert_eq!(refs[2].resolution.definitions().len(), 2);
    for (index, r) in model.references().iter().enumerate() {
        for id in r.resolution.definitions() {
            assert!(
                model
                    .definition(*id)
                    .references
                    .contains(&ReferenceId(index))
            );
        }
    }
    for (index, d) in model.definitions().iter().enumerate() {
        for id in &d.references {
            assert!(
                model
                    .reference(*id)
                    .resolution
                    .definitions()
                    .contains(&DefinitionId(index))
            );
        }
    }
    let offset = TextSize::new(source.rfind('x').unwrap() as u32);
    assert!(model.reference_at(offset).is_some());
    assert!(model.definition_at(TextSize::new(0)).is_some());
    assert!(model.scope_at(offset).is_some());
}
#[test]
fn branch_only_name_is_valid() {
    let analysis = analyze("if condition {x:int} let y=x");
    let r = reference(&analysis.model, "x");
    assert!(matches!(
        r.resolution,
        Resolution::Value {
            possibly_unavailable: true,
            ..
        }
    ));
    assert_eq!(r.resolution.definitions().len(), 1);
}
#[test]
fn num_type_remains_uncertain() {
    let analysis = analyze("repeat 0 {n:num} let column=n");
    let model = &analysis.model;
    let facts = &definition(model, "n").facts;
    assert_eq!(
        facts.value_types,
        Possibilities::Known(vec![ValueType::Int, ValueType::Float])
    );
    depth(facts, 0);
    assert_eq!(reference(model, "n").facts.value_types, facts.value_types);
    depth(&reference(model, "n").facts, 1);
    assert_eq!(
        model.repeats()[0].fields[0].fallback,
        ValueFacts::known(ValueType::Int, 0)
    );
}
#[test]
fn plain_declaration_shapes() {
    let analysis = analyze(
        "i:int f:float a[3]:int repeat 2 { row[4]:float matrix[2][3]:int } let rows=row let matrices=matrix",
    );
    let model = &analysis.model;
    assert_eq!(
        definition(model, "i").facts,
        ValueFacts::known(ValueType::Int, 0)
    );
    assert_eq!(
        definition(model, "f").facts,
        ValueFacts::known(ValueType::Float, 0)
    );
    depth(&definition(model, "a").facts, 1);
    depth(&definition(model, "row").facts, 1);
    depth(&definition(model, "matrix").facts, 2);
    depth(&reference(model, "row").facts, 2);
    depth(&reference(model, "matrix").facts, 3);
}
#[test]
fn type_record_and_binding_navigation() {
    let analysis = analyze(
        "type t=float repeat 1 as rows {each row x:t} let total=sum(r in rows:r.x) check row.x>0",
    );
    let model = &analysis.model;
    assert!(matches!(
        reference(model, "t").resolution,
        Resolution::NamedType(_)
    ));
    assert!(matches!(
        reference(model, "r").resolution,
        Resolution::Bound(_)
    ));
    assert!(matches!(
        reference(model, "x").resolution,
        Resolution::Field { .. }
    ));
    assert_eq!(model.repeats()[0].fields[0].name, "x");
    assert_eq!(model.comprehensions()[0].record_elements, Some(RepeatId(0)));
    assert_eq!(model.checks()[0].deepest, vec![RepeatId(0)]);
    assert!(model.repeats()[0].body_scope.is_some());
}
#[test]
fn runtime_uncertainty_and_whole_array_positions() {
    let analysis = analyze(
        "a[2]:float param p=a let missing=undefined let bad=1/0 index 0 repeat 2 {r[3]:int} check size((r))==3 check defined(r) check count(r)>0 let total=sum(x in a:x) let outside=x",
    );
    let model = &analysis.model;
    assert_eq!(
        definition(model, "p").facts.depths,
        Possibilities::Known(vec![1, 0])
    );
    assert_eq!(
        definition(model, "p").facts.value_types,
        Possibilities::one(ValueType::Float)
    );
    assert_eq!(model.checks()[0].deepest, vec![RepeatId(0)]);
    assert!(model.checks()[1].dependencies.is_empty());
    assert_eq!(model.checks()[2].deepest, vec![RepeatId(0)]);
    assert!(reference(model, "x").resolution.definitions().is_empty());
    assert_eq!(definition(model, "missing").facts, ValueFacts::unknown());
}
#[test]
fn captures_use_precommit_references_and_postcommit_bounds() {
    let analysis = analyze("let n=4 line \"{$n} {x:0..n} {n:int}\" let result=x");
    let model = &analysis.model;
    let ns: Vec<_> = model
        .definitions()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.name == "n")
        .map(|(i, _)| DefinitionId(i))
        .collect();
    let refs: Vec<_> = model
        .references()
        .iter()
        .filter(|r| r.name == "n")
        .collect();
    let pattern_ref = refs
        .iter()
        .find(|r| r.name_range.start() > TextSize::new(20))
        .unwrap();
    assert_eq!(pattern_ref.resolution.definitions(), &ns[1..]);
    assert!(refs.iter().any(|r| r.resolution.definitions() == &ns[..1]));
}

#[test]
fn repeated_let_facts_include_empty_fallback() {
    let analysis =
        analyze("a[2]:word repeat 0 { let s=\"text\" let row=a } let strings=s let rows=row");
    let model = &analysis.model;
    assert_eq!(
        definition(model, "s").facts,
        ValueFacts::known(ValueType::String, 0)
    );
    assert_eq!(
        reference(model, "s").facts.value_types,
        Possibilities::Known(vec![ValueType::String, ValueType::Int])
    );
    assert_eq!(
        reference(model, "row").facts.depths,
        Possibilities::Known(vec![2, 1])
    );
    assert_eq!(
        model.repeats()[0].fields[0].fallback,
        ValueFacts::known(ValueType::Int, 0)
    );
}

#[test]
fn instances_see_definitions_published_by_a_retained_instance() {
    for source in [
        "let x=1 instances {let y=x x:word let z=y+0}",
        "instances {let y=x x:word let z=y+0}",
        "let x=1 instances {repeat 1 {let y=x} x:word}",
    ] {
        let analysis = analyze(source);
        let model = &analysis.model;
        let r = reference(model, "x");
        for (index, d) in model
            .definitions()
            .iter()
            .enumerate()
            .filter(|(_, d)| d.name == "x")
        {
            assert!(
                r.resolution.definitions().contains(&DefinitionId(index)),
                "{source}: {d:?}"
            );
        }
        for facts in [&r.facts, &definition(model, "y").facts] {
            assert!(match &facts.value_types {
                Possibilities::Unknown => true,
                Possibilities::Known(types) => types.contains(&ValueType::String),
            });
        }
    }
}

#[test]
fn repeat_iterations_use_fresh_frames() {
    let analysis = analyze("let x=1 repeat 2 {let y=x x:word} let column=x");
    let model = &analysis.model;
    let refs: Vec<_> = model
        .references()
        .iter()
        .filter(|r| r.name == "x")
        .collect();
    assert_eq!(refs[0].resolution.definitions(), &[DefinitionId(0)]);
    assert_eq!(refs[0].facts, ValueFacts::known(ValueType::Int, 0));
    assert_eq!(refs[1].resolution.definitions().len(), 1);
    assert_eq!(
        model.definition(refs[1].resolution.definitions()[0]).kind,
        DefinitionKind::Read
    );
    assert_eq!(refs[1].facts, ValueFacts::known(ValueType::String, 1));
}

#[test]
fn sections_arms_see_definitions_from_other_arms() {
    let analysis = analyze("let x=1 sections {\"A\" {let y=x} \"B\" {x:word}} let z=x");
    let model = &analysis.model;
    for r in model.references().iter().filter(|r| r.name == "x") {
        assert_eq!(r.resolution.definitions().len(), 2);
        assert_eq!(r.facts.value_types, Possibilities::Unknown);
    }
    assert_eq!(
        definition(model, "y").facts.value_types,
        Possibilities::Unknown
    );
}

#[test]
fn branches_after_repeats_retain_column_and_scalar_candidates() {
    let analysis =
        analyze("repeat 2 {x:word} if condition {let x=2.0} else {let previous=x} let y=x");
    let r = reference(&analysis.model, "x");
    assert_eq!(r.resolution.definitions().len(), 2);
    assert_eq!(
        r.facts.value_types,
        Possibilities::Known(vec![ValueType::Float, ValueType::String])
    );
    assert_eq!(r.facts.depths, Possibilities::Known(vec![0, 1]));
}

#[test]
fn sections_store_preserves_values_when_no_terminator_matches() {
    let analysis = analyze("let x=1 sections until \"END\" store x {\"A\" optional} let y=x");
    let r = reference(&analysis.model, "x");
    assert_eq!(r.resolution.definitions().len(), 2);
    assert_eq!(
        r.facts.value_types,
        Possibilities::Known(vec![ValueType::Int, ValueType::String])
    );
}

fn entry<'a>(model: &'a SemanticModel, name: &str) -> &'a OutputEntry {
    model
        .output_entries()
        .iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("no output entry '{name}'"))
}
fn names(model: &SemanticModel) -> Vec<&str> {
    model
        .output_entries()
        .iter()
        .map(|entry| entry.name.as_str())
        .collect()
}

#[test]
fn output_entries_inside_repeats_collect_every_enclosing_level() {
    let analysis = analyze("repeat 2 {n:int repeat n {a[n]:float list b:word until eol}}");
    let model = &analysis.model;
    assert_eq!(names(model), ["n", "a", "b"]);
    assert_eq!(
        entry(model, "n").facts,
        ValueFacts::known(ValueType::Int, 1)
    );
    assert_eq!(
        entry(model, "a").facts,
        ValueFacts::known(ValueType::Float, 3)
    );
    assert_eq!(
        entry(model, "b").facts,
        ValueFacts::known(ValueType::String, 3)
    );
    assert!(!entry(model, "a").possibly_absent);
}

#[test]
fn an_empty_repeat_keeps_the_fallback_depth_but_not_its_value_type() {
    let analysis = analyze("a[2]:int repeat 0 {let s=\"text\" let row=a}");
    let model = &analysis.model;
    // A `let` falls back to an empty int column, which matches any value type.
    assert_eq!(
        entry(model, "s").facts,
        ValueFacts::known(ValueType::String, 1)
    );
    let row = &entry(model, "row").facts;
    assert_eq!(row.value_types, Possibilities::one(ValueType::Int));
    assert_eq!(row.depths, Possibilities::Known(vec![2, 1]));
}

#[test]
fn output_entries_of_conditional_definitions_may_be_absent() {
    let analysis = analyze(
        "c:int
        if c {x:int} else {y:int}
        if c {z:int}
        z:int
        sections {\"A {a:int}\" optional \"B {b:int}\" {let d=1}}
        if c {repeat 2 {r:int}}",
    );
    let model = &analysis.model;
    for name in ["x", "y", "a", "r"] {
        assert!(entry(model, name).possibly_absent, "{name}");
    }
    for name in ["c", "z", "b", "d"] {
        assert!(!entry(model, name).possibly_absent, "{name}");
    }
}

#[test]
fn a_repeat_publishes_its_conditional_fields() {
    let analysis = analyze(
        "repeat 2 {k:int if k {list a:int until eol}
        sections {\"A {b:int}\" optional}}",
    );
    let model = &analysis.model;
    assert!(!entry(model, "a").possibly_absent);
    assert!(!entry(model, "b").possibly_absent);
    assert_eq!(
        entry(model, "a").facts,
        ValueFacts::known(ValueType::Int, 2)
    );
}

#[test]
fn instances_and_sections_keep_their_definitions_facts() {
    let analysis = analyze(
        "instances {sections {\"A {n:int}\" {repeat n {v:float}} \"B {t:word}\" optional}
        let m=1}",
    );
    let model = &analysis.model;
    assert_eq!(
        entry(model, "n").facts,
        ValueFacts::known(ValueType::Int, 0)
    );
    assert_eq!(
        entry(model, "v").facts,
        ValueFacts::known(ValueType::Float, 1)
    );
    assert_eq!(
        entry(model, "t").facts,
        ValueFacts::known(ValueType::String, 0)
    );
    assert!(entry(model, "t").possibly_absent);
    assert!(!entry(model, "n").possibly_absent);
    assert!(!entry(model, "m").possibly_absent);
}

#[test]
fn output_entries_of_a_redefined_name_join_all_its_definitions() {
    let analysis = analyze("x:int let x=\"s\" y:int repeat 2 {y:int} z:int z:int");
    let model = &analysis.model;
    let x = entry(model, "x");
    assert_eq!(x.definitions.len(), 2);
    assert_eq!(
        x.facts.value_types,
        Possibilities::Known(vec![ValueType::Int, ValueType::String])
    );
    let y = entry(model, "y");
    assert_eq!(y.facts.depths, Possibilities::Known(vec![0, 1]));
    assert_eq!(y.definitions.len(), 2);
    let z = entry(model, "z");
    assert_eq!(z.definitions.len(), 2);
    assert_eq!(z.facts, ValueFacts::known(ValueType::Int, 0));
}

#[test]
fn unreferenced_entries_are_output_and_structural_names_are_not() {
    let analysis = analyze(
        "type t = int
        v:t
        param p=2
        repeat 2 as rows {each row w:int let unused=1}
        let total=sum(i in w: i)",
    );
    let model = &analysis.model;
    assert_eq!(names(model), ["v", "p", "w", "unused", "total"]);
    assert!(entry(model, "unused").definitions.len() == 1);
    assert!(
        model
            .definition(entry(model, "v").definitions[0])
            .references
            .is_empty()
    );
}

#[test]
fn a_store_named_underscore_stays_only_outside_repeats() {
    let top = analyze("repeat until \"END\" store _ {x:int}");
    assert_eq!(names(&top.model), ["_", "x"]);
    assert_eq!(
        entry(&top.model, "_").facts,
        ValueFacts::known(ValueType::String, 0)
    );
    assert!(!entry(&top.model, "_").possibly_absent);

    let nested = analyze("repeat 2 {repeat until \"END\" store _ {x:int}}");
    assert_eq!(names(&nested.model), ["x"]);
}

#[test]
fn a_store_of_sections_may_be_absent_outside_a_repeat() {
    let top = analyze("sections until \"END\" store ending {\"A\" optional}");
    assert!(entry(&top.model, "ending").possibly_absent);
    let nested = analyze("repeat 2 {sections until \"END\" store ending {\"A\" optional}}");
    assert!(!entry(&nested.model, "ending").possibly_absent);
    assert_eq!(
        entry(&nested.model, "ending").facts,
        ValueFacts::known(ValueType::String, 1)
    );
}

#[test]
fn an_unknown_fact_stays_unknown_and_an_invalid_spec_has_no_entries() {
    let analysis = analyze("sections {\"A\" {let y=x} \"B {x:word}\" optional}");
    assert_eq!(entry(&analysis.model, "y").facts, ValueFacts::unknown());

    let parsed =
        gecode_iris_syntax::parse(b"let x=missing(1)", "model", LanguageVersion::V0_1).unwrap();
    let analysis = gecode_iris_sema::analyze(&parsed);
    assert!(!analysis.is_valid());
    assert!(analysis.model.output_entries().is_empty());
}

#[test]
fn output_entries_handle_any_nesting_depth() {
    let depth = 256;
    let source = format!("{}v:int{}", "repeat 1 {".repeat(depth), "}".repeat(depth));
    let analysis = analyze(&source);
    assert_eq!(
        entry(&analysis.model, "v").facts,
        ValueFacts::known(ValueType::Int, depth)
    );
}
