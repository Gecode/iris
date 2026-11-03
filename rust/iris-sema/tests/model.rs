use iris_sema::*;
use iris_syntax::{LanguageVersion, TextSize};

fn analyze(source: &str) -> Analysis {
    let parsed = iris_syntax::parse(source.as_bytes(), "model", LanguageVersion::V0_1).unwrap();
    let analysis = iris_sema::analyze(&parsed);
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
