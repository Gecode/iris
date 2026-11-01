use iris_conformance::{
    canonical::{Outcome, OutcomeKind},
    case::{Case, Operation, OutputFormat},
    driver::DriverCommand,
};
use std::{path::Path, time::Duration};

const SEED: u64 = 73003;
const COUNT: usize = 256;
const ARRAY_SEED: u64 = 73004;
const ARRAY_COUNT: usize = 384;
struct Probe {
    family: &'static str,
    spec: String,
    input: Vec<u8>,
}
impl Probe {
    fn new(spec: impl Into<String>, input: impl AsRef<[u8]>) -> Self {
        Self {
            family: "scalar",
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
    let scalar = probes();
    let arrays = array_probes();
    println!("Scalar differential: seed={SEED} count={}", scalar.len());
    let mut families = std::collections::BTreeMap::new();
    for probe in &arrays {
        *families.entry(probe.family).or_insert(0usize) += 1;
    }
    println!(
        "Array differential: seed={ARRAY_SEED} count={} families={families:?}",
        arrays.len()
    );
    for (index, probe) in scalar.iter().chain(&arrays).enumerate() {
        iris_cli::check_spec(probe.spec.as_bytes(), "generated differential probe").unwrap_or_else(
            |error| panic!("invalid generated probe {index}: {}: {error}", probe.spec),
        );
    }
    for (index, probe) in scalar.iter().chain(&arrays).enumerate() {
        let (seed, group_index) = if index < scalar.len() {
            (SEED, index)
        } else {
            (ARRAY_SEED, index - scalar.len())
        };
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
                "differential mismatch: seed={seed} index={group_index} family={}\nRust: {rust} read {}/spec.iris {}/input.bin --params '{{}}' --version 0.1\nGecode: {adapter} read {}/spec.iris {}/input.bin --params '{{}}' --version 0.1\nspec: {}\ninput bytes: {:?}\noutcomes: {outcomes:#?}",
                probe.family,
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
        scalar.len()
    );
    println!(
        "Array differential: seed={ARRAY_SEED} count={} mismatches=0; total={}",
        arrays.len(),
        scalar.len() + arrays.len()
    );
}
fn write_probe(directory: &Path, probe: &Probe) -> Case {
    let spec = directory.join("spec.iris");
    let input = directory.join("input.bin");
    std::fs::write(&spec, &probe.spec).unwrap();
    std::fs::write(&input, &probe.input).unwrap();
    Case {
        path: directory.into(),
        display_path: "generated differential probe".into(),
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

fn array_probes() -> Vec<Probe> {
    let mut probes = vec![];
    for (family, cases) in [
        (
            "arrays",
            &[
                ("a[2]:int offset 1", "9223372036854775807 bad"),
                ("a[2]:int offset 1", "9223372036854775807 0"),
                ("a[2]:num offset 1", "9223372036854775807 0.5"),
                ("a[2][2]:num offset -1", "9007199254740993 2 3 0.5"),
                (
                    "a[3]:num offset 1",
                    "-9223372036854775808 9223372036854775807 -0.0",
                ),
                ("a[2]:int offset -1", "-9223372036854775808 bad"),
                ("a[2]:int offset -1", "-9223372036854775808 0"),
                ("a[2]:0..3 offset 1", "9223372036854775807 4"),
                ("a[2]:int offset 1", "9223372036854775807"),
                ("a[0]:float offset 1.0", ""),
                ("z[0]:int a[0]:int offset z", ""),
                ("a[0][missing]:int", ""),
                ("a[46341][46341][missing]:int", ""),
                ("a[46341][46341][0]:int", ""),
                ("a[0][2147483647]:int", ""),
                ("a[0][-1]:int", ""),
                ("a[1.0][missing]:int", ""),
                ("a[0]:0..1.0", ""),
                ("type t=0..3 type u=t a[2]:u offset 7", "1 3"),
                ("type t=0..3 a[2]:t offset 1", "2 4"),
                ("a[2]:3..1", "1 2"),
                (
                    "x:word a[2][0][3]:float let y=a[1] let a=a let x=a let a=y",
                    "before",
                ),
                (
                    "a[2][2]:word let b=a[1] let c=b[1] let a=a[0]",
                    "one two three four",
                ),
                ("a[2][2]:num let b=a[1] let c=a[0][1] let a=b", "-0.0 2 3 4"),
                (
                    "a[1][1][1][1]:int let b=a[0][0][0][0]",
                    "-9223372036854775808",
                ),
                ("a[1]:float let b=a let c=-0.0 let d=c", "-0.0"),
                ("_[2]:word", "é a\0b"),
            ][..],
        ),
        (
            "expressions",
            &[
                ("a[0]:word let v=a==1", ""),
                ("a[0]:int let v=a==missing", ""),
                ("a[1]:int let v=-a", "3"),
                ("a[1]:float let v=-a", "3.0"),
                ("a[0]:int let v=a+missing", ""),
                ("a[0]:int let v=a+1", ""),
                ("a[2][0][3]:float let b=a[1] let c=!!b let d=!!a", ""),
                (
                    "a[2][0]:word let s=sum(a) let d=distinct(a) let n=size(a) let c=count(a)",
                    "",
                ),
                ("a[2][0]:float let s=min(a,3) let m=max(a,-2)", ""),
                ("a[0]:word let s=min(a)", ""),
                ("a[2][0]:float let s=max(a)", ""),
                ("a[0]:float let s=min(a,missing)", ""),
                ("a[0]:word let s=max(a,1.0)", ""),
                (
                    "a[2][2]:int let s=sum(a) let m=min(a) let n=max(a) let d=distinct(a)",
                    "1 2 3 4",
                ),
                ("a[2]:int let s=distinct(a,missing)", "1 1"),
                ("a[2]:int let s=distinct(a,1.0)", "1 1"),
                ("a[3]:int let s=sum(a)", "9223372036854775807 1 -1"),
                ("let a=1[missing]", ""),
                ("a[0]:int let b=a[missing]", ""),
                ("a[1]:int let b=a[1.0]", "1"),
                ("a[1]:int let b=a[\"x\"]", "1"),
                ("a[0]:int let b=a[-1]", ""),
                ("a[2][0]:int let b=a[1][0]", ""),
                ("a[1]:int let b=a[1]", "7"),
                ("a[0]:int let b=a[a]", ""),
                ("a[1]:int let b=any(x in a:1) let c=defined(x)", "0"),
                ("a[1][0]:int let s=sum(x in a:x)", ""),
                (
                    "a[2][2]:int let s=sum(row in a:sum(x in row:x)) let b=all(row in a:all(x in row:defined((x)))) let c=defined(row)",
                    "1 2 3 4",
                ),
                (
                    "a[2][0]:word let s=count(row in a:row) let b=all(row in a:!row)",
                    "",
                ),
                ("a[0]:int let a=all(x in a:missing) let b=defined(x)", ""),
                (
                    "a[0]:float let b=any(x in a:missing) let c=sum(x in a:missing) let d=count(x in a:missing)",
                    "",
                ),
                ("a[2]:int let b=all(x in a:x&&(1/0))", "0 1"),
                ("a[2]:int let b=any(x in a:x||(1/0))", "1 0"),
                ("a[1]:float let b=sum(x in a:x)", "1.0"),
                ("a[2]:int let b=sum(x in a:x)", "9223372036854775807 1"),
                ("let b=all(x in 1:missing)", ""),
                ("let b=any(x in missing:1)", ""),
                (
                    "a[1]:int let b=count(x in a:defined(x)) let c=defined(x)",
                    "0",
                ),
                ("a[1]:int let b=sum(x in a:sum(y in a:x+y))", "3"),
            ][..],
        ),
        (
            "layouts",
            &[
                ("a[46341][46341]:int layout 1", ""),
                ("a[2][3]:int layout \"bogus\"", ""),
                ("a[46341][46341]:int layout \"bogus\"", ""),
                ("a[46341][46341]:int layout \"UPPER_ROW\"", ""),
                ("a[46341][46341]:int layout \"FULL_MATRIX\"", ""),
                ("a[1.0][missing]:int layout missing", ""),
                (
                    "a[2][3]:int layout \"FULL_MATRIX\" offset -3",
                    "1 2 3 4 5 6",
                ),
                ("a[1][0]:float layout \"FULL_MATRIX\"", ""),
                (
                    "a[2][2]:num layout \"UPPER_DIAG_ROW\" offset 1",
                    "9223372036854775807 2 0.5",
                ),
                (
                    "a[2][2]:int layout \"LOWER_DIAG_COL\" offset 1",
                    "9223372036854775807 2 bad",
                ),
                (
                    "a[2][2]:int layout \"LOWER_DIAG_COL\" offset 1",
                    "9223372036854775807 2 0",
                ),
                ("a[2][2]:1..3 layout \"UPPER_ROW\" offset 10", "1"),
                ("a[2][2]:1..3 layout \"UPPER_ROW\" offset 10", "4"),
                ("z[0]:word a[0][0]:int layout z", ""),
            ][..],
        ),
        (
            "alphabets",
            &[
                ("a[1][3]:alphabet \"aba\"", "aba"),
                (
                    "type t=alphabet \"ab\" base -2 blank \".\" strip \"-\" type u=t a[2][2]:u",
                    "a- .\nb-a",
                ),
                (
                    "a[1][4]:alphabet \"abc\" base 3 base -2 blank \"a\" blank \"b\" strip \"c\" strip \"-\"",
                    "ab-cb",
                ),
                ("a[1][2]:alphabet \"é\"", "é"),
                ("a[2][2]:alphabet \"ab\"", "ab\nba"),
                (
                    "comment \"#\" stop \"END\" a[1][2]:alphabet \"ab\"",
                    "# skip\na\nb\nEND\nignored",
                ),
                ("a[1][2]:alphabet \"ab\" blank \"a\" strip \"a\"", "aabb"),
                ("a[1][2]:alphabet \"\" blank \".\"", ".."),
                ("a[0][3]:alphabet \"\"", ""),
                ("_[2][0]:alphabet \"ab\"", ""),
                ("a[1][1]:alphabet \"ab\"", "x"),
                ("a[1][1]:alphabet \"ab\" strip \"-\"", "--"),
                ("a[1][1]:alphabet \"ab\" base 9223372036854775807", "b"),
                (
                    "a[1][1]:alphabet \"ab\" base 9223372036854775807 blank \"b\"",
                    "b",
                ),
                ("a[1][1]:alphabet \"ab\" base -9223372036854775807", "b"),
                ("a[1][1]:alphabet \"a\0\"", "\0"),
                ("a[1][1]:alphabet \"a\" base:int", "a 7"),
                ("a[46341][46341]:alphabet \"a\"", ""),
                (
                    "separators \",\" tokens \";\" a[1][2]:alphabet \"a;\"",
                    ",a;",
                ),
                (
                    "a[1][1]:alphabet \"é\" base 9223372036854775807 ignore trailing",
                    "é",
                ),
            ][..],
        ),
        (
            "lists",
            &[
                ("list a:0..1.0 until missing offset \"bad\"", ""),
                ("list a:int until 0 offset 1.0", "bad 0"),
                ("list a:int until 0 offset 1.0", "1 0 tail"),
                ("list a:int until eol offset 1.0", "1 2  \n3"),
                ("list a:num until 2.0", "1 2.0"),
                ("list a:int until 2.0", "2.0"),
                ("list a:num until 9007199254740992.0", "1 9007199254740993"),
                ("list a:float until -0.0", "0.0"),
                ("list a:int until \"END\"", "1 2 END"),
                ("list a:word until 0", "+0 00 0"),
                ("list eol:int until 0", "1 0"),
                ("list a:word until 1.0", ""),
                ("z[0]:int list a:int until z", ""),
                ("list a:2..1 until 0", "0"),
                ("list a:1..2 until 0 offset 10", "1 2 0"),
                ("list a:1..2 until \"END\"", "1 END"),
                ("list a:int until 0", "1 2"),
                ("list a:int until \"END\"", "1 2"),
                ("list a:int until 0 offset 1", "9223372036854775807 bad 0"),
                ("list a:num until 0 offset 1", "9223372036854775807 0.5 0"),
                ("list a:int until 0 offset 1", "9223372036854775807 0"),
                ("list a:int until 0 offset 1.0", "0"),
                ("list a:float until 0 offset -1", "-0.0 1 0"),
                ("list a:int until eol b:int", "1 2  \n3"),
                ("list a:int until eol b:int", "1 2\r\n3"),
                ("list a:int until eol", "1\r2\u{b}3\u{c}4"),
                ("list a:int until eol", "\n  \n1 2 \t"),
                ("list a:int until eol", "\n \r\n"),
                ("comment \"#\" list a:int until eol b:int", "#skip\n1 2\n3"),
                ("stop \"END\" list a:int until eol", "END\n1 2"),
                ("tokens \",\" list a:word until eol", "a,b\n"),
                ("separators \",\" list a:int until eol b:int", "1,2,\n3"),
                (
                    "separators \",\" list a:int until eol offset 1.0",
                    "1,2,\n3",
                ),
                ("list _:word until \"END\"", "é a\0b END"),
                ("comment \"#\" list a:word until eol", "one # two\n"),
            ][..],
        ),
    ] {
        for &(spec, input) in cases {
            probes.push(Probe {
                family,
                ..Probe::new(spec, input)
            });
        }
    }
    // All nine names, including their empty and one-cell boundaries.
    for name in [
        "FULL_MATRIX",
        "UPPER_ROW",
        "LOWER_ROW",
        "UPPER_DIAG_ROW",
        "LOWER_DIAG_ROW",
        "UPPER_COL",
        "LOWER_COL",
        "UPPER_DIAG_COL",
        "LOWER_DIAG_COL",
    ] {
        for n in 0..=4 {
            let stored = if name == "FULL_MATRIX" {
                n * n
            } else if name.contains("DIAG") {
                n * (n + 1) / 2
            } else {
                n * (n + 1) / 2 - n
            };
            let input = (1..=stored)
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            probes.push(Probe {
                family: "layouts",
                ..Probe::new(format!("a[{n}][{n}]:int layout \"{name}\" offset 7"), input)
            });
        }
    }
    let mut state = ARRAY_SEED;
    while probes.len() < ARRAY_COUNT {
        let sample = probes.len();
        let mut dimensions = vec![];
        for _ in 0..1 + sample % 4 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            dimensions.push(((state >> 32) % 5) as usize);
        }
        let shape = dimensions
            .iter()
            .map(|n| format!("[{n}]"))
            .collect::<String>();
        let count: usize = dimensions.iter().product();
        let kind = ["int", "float", "num", "word", "-20..20"][sample % 5];
        let input = (0..count)
            .map(|i| match kind {
                "word" => format!("w{i}"),
                "float" => {
                    if i % 2 == 0 {
                        "-0.0".into()
                    } else {
                        format!("{i}.5")
                    }
                }
                "num" if i + 1 == count => "0.5".into(),
                _ => ((i % 31) as i64 - 15).to_string(),
            })
            .collect::<Vec<_>>()
            .join(if sample % 2 == 0 { "\n" } else { " " });
        let offset = if kind != "word" { " offset -2" } else { "" };
        let mut spec = format!("a{shape}:{kind}{offset} let b=a let n=size(a) let t=!!a");
        // Copy each reachable row/scalar without ever selecting an empty row's child.
        let mut path = String::from("a");
        for (level, &width) in dimensions.iter().enumerate() {
            if width == 0 {
                break;
            }
            path.push_str(&format!("[{}]", width - 1));
            spec.push_str(&format!(" let selected{level}={path}"));
        }
        if sample % 7 == 0 {
            spec.push_str(" let a=b let b=1 let a=a");
        }
        if sample % 11 == 0 {
            spec.push_str(" _[0]:float");
        }
        probes.push(Probe {
            family: "generated arrays",
            ..Probe::new(spec, input)
        });
    }
    assert_eq!(
        probes.len(),
        ARRAY_COUNT,
        "curated probes must never be truncated"
    );
    probes
}
