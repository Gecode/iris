use iris_conformance::{
    canonical::{Outcome, OutcomeKind},
    case::{Case, Operation, OutputFormat},
    driver::DriverCommand,
};
use std::{path::Path, time::Duration};

const SEED: u64 = 73003;
const COUNT: usize = 256;
struct Probe {
    spec: String,
    input: Vec<u8>,
}
impl Probe {
    fn new(spec: impl Into<String>, input: impl AsRef<[u8]>) -> Self {
        Self {
            spec: spec.into(),
            input: input.as_ref().to_vec(),
        }
    }
}
#[test]
#[ignore = "requires pinned Gecode adapter in GECODE_ADAPTER"]
fn scalar_differential() {
    let adapter = std::env::var("GECODE_ADAPTER")
        .expect("set GECODE_ADAPTER to the pinned adapter executable");
    let rust = env!("CARGO_BIN_EXE_iris-driver");
    let drivers = [
        DriverCommand::parse(&format!("\"{rust}\"")).unwrap(),
        DriverCommand::parse(&format!("\"{adapter}\"")).unwrap(),
    ];
    let directory = tempfile::tempdir().unwrap();
    let probes = probes();
    println!("Scalar differential: seed={SEED} count={}", probes.len());
    for (index, probe) in probes.iter().enumerate() {
        iris_cli::check_spec(probe.spec.as_bytes(), "generated scalar probe").unwrap();
        let case = write_probe(directory.path(), probe);
        let outcomes = drivers.each_ref().map(|driver| {
            let output = driver
                .invoke(&case, "0.1", Duration::from_secs(5))
                .map_err(|e| format!("{e:?}"))?;
            let kind = match output.status.code() {
                Some(0) => OutcomeKind::Data,
                Some(1) => OutcomeKind::Error,
                _ => return Err(format!("unexpected driver output: {output:?}")),
            };
            Outcome::decode(kind, &output.stdout, "0.1")
        });
        if !matches!(&outcomes,[Ok(a),Ok(b)] if a==b) {
            let repro = directory.keep();
            panic!(
                "scalar mismatch: seed={SEED} index={index}\nRust: {rust} read {}/spec.iris {}/input.bin --params '{{}}' --version 0.1\nGecode: {adapter} read {}/spec.iris {}/input.bin --params '{{}}' --version 0.1\nspec: {}\ninput bytes: {:?}\noutcomes: {outcomes:#?}",
                repro.display(),
                repro.display(),
                repro.display(),
                repro.display(),
                probe.spec,
                probe.input
            );
        }
    }
    println!(
        "Scalar differential: seed={SEED} count={} mismatches=0",
        probes.len()
    );
}
fn write_probe(directory: &Path, probe: &Probe) -> Case {
    let spec = directory.join("spec.iris");
    let input = directory.join("input.bin");
    std::fs::write(&spec, &probe.spec).unwrap();
    std::fs::write(&input, &probe.input).unwrap();
    Case {
        path: directory.into(),
        display_path: "generated scalar probe".into(),
        operation: Operation::Read,
        output: OutputFormat::Json,
        spec,
        input: Some(input),
        params: Some("{}".into()),
        expected: Outcome::Parse,
        known_failures: vec![],
    }
}
fn probes() -> Vec<Probe> {
    let mut probes = vec![];
    for kind in ["int", "float", "num"] {
        for token in [
            "+007",
            "-07",
            "-0",
            "-9223372036854775808",
            "9223372036854775807",
            "9223372036854775808",
            "+.5",
            "1.e2",
            "0e-9999",
            "2.2250738585072013e-308",
            "5e-324",
            "1e-9999",
            "1e309",
            "nan",
            "0x10",
            "1\0",
            "oops",
            "",
        ] {
            probes.push(Probe::new(format!("x:{kind}"), token));
        }
    }
    for (spec, input) in [
        ("a:word b:rest", "é\t the rest \r\n"),
        ("_:word x:word", "discard a\0b"),
        ("x:rest", " \talpha\rbeta\u{b}\u{c} \t\r\n"),
        ("tokens \"::\" a:word b:word c:word d:word", "a::b"),
        (
            "tokens \",\" separators \",\" tokens \",\" a:int b:int",
            "1,2",
        ),
        ("a:word tokens \";\" b:word c:word d:word", "a;b x;y"),
        ("comment \"END\" stop \"END\" x:word", "END\nkeep"),
        ("comment \"END\" x:word", "ENDé\nkeep"),
        ("comment \"é\" x:word", "éx\nkeep"),
        ("comment \"END_\" x:word", "END_x"),
        ("comment \"#\" x:word", " \t\r#skip\nkeep"),
        ("comment \"#\" x:word", "\u{b}#keep"),
        ("separators \",\" comment \"#\" x:word", ",#keep"),
        ("stop \"END\" x:word", "ENDx"),
        ("stop \"END\" x:word", "ENDé\nlater\n"),
        ("stop \"\" x:word", "keep"),
        ("comment \"#\" x:int", "7\n# tail"),
        ("x:int ignore trailing", "7 junk"),
        (
            "n:int type t=0..n type alias=t let n=3 a:alias b:0..n",
            "2 2 3",
        ),
        ("n:int type t=0..n let n=3 a:t", "2 3"),
        ("if 1 {type t=int a:t} if 1 {type t=word b:t}", "7 hi"),
        ("if 0 {} else if:int", "7"),
        (
            "let x=1 if 0 {let x=9} else if 1 {let y=\"yes\"} else {let y=\"no\"} let x=\"s\"",
            "",
        ),
        (
            "let a=defined((missing)) let missing=2 let b=defined(missing)",
            "",
        ),
        ("let v=1||missing let w=0&&(1/0)", ""),
        ("let v=gecode.int_min let w=gecode.int_max", ""),
        ("x:float let z=-0.0 let y=-x", "-0.0"),
        (
            "let a=min(3,-2,9) let b=max(-3,7) let c=sum(1,-4,8) let d=distinct(1,2,3) let e=distinct(1,1)",
            "",
        ),
        ("let v=distinct(1,1,missing)", ""),
        ("let v=min(\"x\",missing)", ""),
        ("let v=sum(9223372036854775807,1,-1)", ""),
        ("let v=1.0+missing", ""),
        ("let v=1.0/0.0", ""),
        ("let v=size(1)", ""),
        ("let v=count(1)", ""),
        ("x:..3", "-7"),
        ("x:0..", "9"),
        ("x:1..<3", "3"),
        ("x:1.0..missing", ""),
        ("x:0..<(-9223372036854775807-1)", ""),
    ] {
        probes.push(Probe::new(spec, input));
    }
    for expression in [
        "0^0",
        "(-2)^63",
        "(-9223372036854775807-1)^1",
        "(-9223372036854775807-1)^0",
        "0^9223372036854775807",
        "1^9223372036854775807",
        "(-1)^9223372036854775807",
        "2^63",
        "2^64",
        "0^-1",
        "1^-1",
        "(-9223372036854775807-1)/-1",
        "(-9223372036854775807-1)%-1",
        "-(-9223372036854775807-1)",
        "-2^63",
        "!\"\"",
        "!!\"0\"",
        "!-0.0",
        "9007199254740993==9007199254740992.0",
        "9007199254740993==9007199254740992",
        "\"é\"==\"é\"",
        "\"1\"==1",
        "\"1\"!=1",
        "\"a\"<\"b\"",
        "1.0^-1",
        "1/0",
        "1%0",
    ] {
        probes.push(Probe::new(format!("let value={expression}"), ""));
    }
    let mut state = SEED;
    while probes.len() < COUNT {
        // Fixed LCG: stable samples without a new random-number dependency.
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let a = ((state >> 32) % 31) as i64 - 15;
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let b = ((state >> 32) % 17) as i64 - 8;
        let operators = [
            "+", "-", "*", "/", "%", "^", "==", "!=", "<", ">", "<=", ">=", "&&", "||",
        ];
        let operator = operators[(state as usize) % operators.len()];
        probes.push(Probe::new(
            format!("a:int b:int let result=a{operator}b"),
            format!("{a}\t{b}\n"),
        ));
    }
    assert_eq!(probes.len(), COUNT);
    probes
}
