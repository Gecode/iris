//! The running example of `docs/builders.md`. For every case of
//! `formats/jobshop`, the built spec reads what the text spec reads.
use gecode_iris::build::{
    D2, DynField, Field, Int, Maybe, SpecBuilder, Str, distinct, gecode, lit, pattern, sum,
    sum_over, word,
};
use gecode_iris::{Failure, Params, Spec};
use gecode_iris_conformance::case::{self, Case};
use std::{fs, path::Path};

/// What `iris gen --lang rust` prints for `formats/jobshop/jobshop.iris`.
#[path = "../../iris-cli/tests/fixtures/jobshop-view.rs"]
mod view;

const MSG_DISTINCT: &str = "a job uses a machine twice; each job must use every machine once";
const MSG_TOTAL: &str = "the total duration is too large";

struct Jobshop {
    spec: Spec,
    name: Field<Str, gecode_iris::build::D0, Maybe>,
    n_jobs: Field<Int>,
    n_machines: Field<Int>,
    machine: Field<Int, D2>,
    duration: Field<Int, D2>,
}

fn jobshop() -> Jobshop {
    let mut b = SpecBuilder::new("jobshop");
    b.comment(["#", "+++"]);
    let (name, n_jobs, n_machines, machine, duration) = b.instances(|i| {
        let name = i.optional_capture("name", word());
        let n_jobs = i.capture("n_jobs", lit(1)..);
        let n_machines = i.capture("n_machines", lit(1)..);
        let (job, machine, duration) = i.sections().skip_other().body(|s| {
            s.arm(pattern!("instance {name}")).optional();
            s.arm(pattern!("{n_jobs} {n_machines}")).last().body(|a| {
                a.check(n_jobs.le((gecode::INT_MAX - 2) / 2 / n_machines))
                    .message("there are too many jobs and machines");
                a.repeat(n_jobs, |job| {
                    let (machine, duration) = job.repeat(n_machines, |step| {
                        (
                            step.read("machine", lit(0)..n_machines.ex()), // prints 0..<n_machines
                            step.read("duration", lit(0)..),
                        )
                    });
                    (job.record(), machine, duration)
                })
            })
        });
        i.check(distinct(job.get(machine))).message(MSG_DISTINCT);
        i.check(sum_over(job, |j| sum(j.get(duration))).le(gecode::INT_MAX))
            .message(MSG_TOTAL);
        (name, n_jobs, n_machines, machine, duration)
    });
    let spec = b.build().unwrap_or_else(|error| {
        panic!("{error}\n{}", error.text().unwrap_or_default());
    });
    Jobshop {
        spec,
        name,
        n_jobs,
        n_machines,
        machine,
        duration,
    }
}

fn params(case: &Case) -> Params {
    let mut params = Params::new();
    if let Some(text) = &case.params {
        let values: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(text).unwrap();
        for (name, value) in values {
            params.set(name, value.as_str().unwrap());
        }
    }
    params
}

fn cases() -> Vec<Case> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../formats/jobshop");
    case::discover(&[root], None)
        .unwrap()
        .iter()
        .map(|path| Case::load(path, "0.1").unwrap())
        .collect()
}

fn code(failure: &Failure) -> String {
    match failure {
        Failure::Language(error) => error.code.as_str().to_owned(),
        other => panic!("unexpected failure: {other}"),
    }
}

/// A compile error unless both handles have one type.
fn same_handle<H>(_: H, _: H) {}

#[test]
fn a_view_of_the_text_spec_has_the_handles_of_the_builder() {
    let built = jobshop();
    same_handle(built.name, view::name);
    same_handle(built.n_jobs, view::n_jobs);
    same_handle(built.n_machines, view::n_machines);
    same_handle(built.machine, view::machine);
    same_handle(built.duration, view::duration);
    let mut read = 0;
    for case in cases() {
        let input = fs::read(case.input.as_ref().unwrap()).unwrap();
        let Ok(data) = built.spec.read(&input, "input", &params(&case)) else {
            continue;
        };
        read += 1;
        assert_eq!(data.get(view::name), data.get(built.name));
        assert_eq!(data.get(view::n_jobs), data.get(built.n_jobs));
        assert_eq!(data.get(view::n_machines), data.get(built.n_machines));
        let rows = |handle: Field<Int, D2>| data.get(handle).rows().collect::<Vec<_>>();
        assert_eq!(rows(view::machine), rows(built.machine));
        assert_eq!(rows(view::duration), rows(built.duration));
    }
    assert!(read >= 2, "too few reads: {read}");
}

#[test]
fn the_text_differs_only_in_internal_names() {
    let expected = r##"format jobshop
comment "#" "+++"
instances {
  sections skip other {
    "instance {name:word}" optional
    "{n_jobs:1..} {n_machines:1..}" last {
      check n_jobs <= (gecode.int_max - 2) / 2 / n_machines "there are too many jobs and machines"
      repeat n_jobs as records_1 {
        each record_1
        repeat n_machines {
          machine: 0..<n_machines
          duration: 0..
        }
      }
    }
  }
  check distinct(record_1.machine) "a job uses a machine twice; each job must use every machine once"
  check sum(element_1 in records_1: sum(element_1.duration)) <= gecode.int_max "the total duration is too large"
}
"##;
    assert_eq!(jobshop().spec.source(), expected);
}

#[test]
fn every_case_reads_like_the_text_spec() {
    let built = jobshop();
    let cases = cases();
    assert_eq!(cases.len(), 3);
    for case in cases {
        let text = Spec::parse(&fs::read(&case.spec).unwrap(), "jobshop.iris").unwrap();
        let input = fs::read(case.input.as_ref().unwrap()).unwrap();
        let params = params(&case);
        let expected = text.read(&input, "input.txt", &params);
        let actual = built.spec.read(&input, "input.txt", &params);
        match (expected, actual) {
            (Ok(expected), Ok(actual)) => {
                assert_eq!(
                    expected.to_json().unwrap(),
                    actual.to_json().unwrap(),
                    "{}",
                    case.display_path
                );
            }
            (Err(expected), Err(actual)) => {
                assert_eq!(code(&expected), code(&actual), "{}", case.display_path);
            }
            (expected, actual) => panic!(
                "{}: the text spec gave {expected:?}, the builder {actual:?}",
                case.display_path
            ),
        }
    }
}

#[test]
fn handles_read_the_documented_types() {
    let built = jobshop();
    let input = "instance tiny\n2 2\n0 3 1 4\n1 5 0 6\n";
    let spec = &built.spec;
    let data = spec
        .read(input.as_bytes(), "input.txt", &Params::new())
        .unwrap();
    let jobs: i64 = data.get(built.n_jobs);
    let machines: i64 = data.get(built.n_machines);
    let first_job: &[i64] = data.get(built.machine).row(0);
    let last_duration: i64 = data.get(built.duration).row(1)[1];
    let label: Option<&str> = data.get(built.name);
    assert_eq!((jobs, machines), (2, 2));
    assert_eq!(first_job, &[0, 1]);
    assert_eq!(last_duration, 6);
    assert_eq!(label, Some("tiny"));
}

#[test]
fn a_dynamic_handle_reads_the_same_entry() {
    let built = jobshop();
    let dynamic: DynField = Field::declared("machine");
    let data = built
        .spec
        .read(b"1 1\n0 7\n", "input.txt", &Params::new())
        .unwrap();
    assert_eq!(data.get(dynamic).unwrap().row(0).as_ints().unwrap(), &[0]);
}
