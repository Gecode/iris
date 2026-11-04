use iris_core::{Params, Spec};
use iris_js::{SpecHandle, render_dzn, render_json, transfer};
use serde_json::Value as Json;

const PROBE_SPEC: &str = "\
bounds[2]: int  big[3]: int  f[4]: float  s[2]: word
repeat 2 { k: int  x[k]: int }
e[2][0]: int
repeat 2 { m: int  repeat m { list y: int until 0 } }
let z = -0.0
";
const PROBE_INPUT: &str = "\
-9223372036854775808 9223372036854775807
9007199254740991 9007199254740992 9007199254740993
0 1e300 0.5 -2
h\u{e9}llo \u{2713}
2 1 2
0
1 3 0
2 1 0 0
";

fn envelope(error: String) -> Json {
    serde_json::from_str(&error).expect("failures are JSON envelopes")
}

#[test]
fn reads_render_the_same_documents_as_iris_core() {
    let handle = SpecHandle::parse(PROBE_SPEC.as_bytes(), "probe.iris").unwrap();
    let encoded = handle
        .read(PROBE_INPUT.as_bytes(), "probe.txt", vec![])
        .unwrap();
    let data = Spec::parse(PROBE_SPEC.as_bytes(), "probe.iris")
        .unwrap()
        .read(PROBE_INPUT.as_bytes(), "probe.txt", &Params::new())
        .unwrap();
    let json = render_json(&encoded).unwrap();
    assert_eq!(json.as_bytes(), data.to_json().unwrap());
    assert_eq!(
        render_dzn(&encoded).unwrap().as_bytes(),
        data.to_dzn().unwrap()
    );
    for probe in [
        "-9223372036854775808",
        "9223372036854775807",
        "9007199254740993",
        "-0.0",
        "1e300",
        "h\u{e9}llo",
        "[[1,2],[]]",
        "[[[3]],[[1],[]]]",
        "[[],[]]",
    ] {
        assert!(json.contains(probe), "{probe} missing from {json}");
    }
}

#[test]
fn every_numeric_array_is_aligned_for_typed_array_views() {
    let handle = SpecHandle::parse(PROBE_SPEC.as_bytes(), "probe.iris").unwrap();
    let encoded = handle
        .read(PROBE_INPUT.as_bytes(), "probe.txt", vec![])
        .unwrap();
    assert_eq!(encoded.len() % 4, 0);
    assert_eq!(&encoded[..4], &transfer::FORMAT_VERSION.to_le_bytes());
}

#[test]
fn params_are_applied_in_order_and_catalogs_render() {
    let spec = "param n = 1\nparam n2 = n\ninstances { name: word  a[n]: int }";
    let handle = SpecHandle::parse(spec.as_bytes(), "spec.iris").unwrap();
    let input = "first 5\nsecond 7\n";
    let read = handle
        .read(
            input.as_bytes(),
            "input.txt",
            vec!["instance".into(), "second".into()],
        )
        .unwrap();
    assert!(render_json(&read).unwrap().contains("\"value\":[7]"));
    let catalog = handle
        .catalog(input.as_bytes(), "input.txt", vec![])
        .unwrap();
    let expected = Spec::parse(spec.as_bytes(), "spec.iris")
        .unwrap()
        .catalog(input.as_bytes(), "input.txt", &Params::new())
        .unwrap();
    assert_eq!(
        render_json(&catalog).unwrap().as_bytes(),
        expected.to_json().unwrap()
    );
    assert_eq!(
        render_dzn(&catalog).unwrap().as_bytes(),
        expected.to_dzn().unwrap()
    );
    let odd = handle.read(input.as_bytes(), "input.txt", vec!["instance".into()]);
    assert_eq!(envelope(odd.err().unwrap())["category"], "internal");
}

#[test]
fn language_failures_carry_the_canonical_document_and_diagnostic() {
    let Err(error) = SpecHandle::parse(b"x: nope", "bad.iris") else {
        panic!("spec error expected");
    };
    let error = envelope(error);
    assert_eq!(error["kind"], "language");
    let document: Json = serde_json::from_str(error["document"].as_str().unwrap()).unwrap();
    assert_eq!(document["spec_location"]["origin"], "bad.iris");
    assert!(document["input_location"].is_null());
    assert!(
        error["diagnostic"]
            .as_str()
            .unwrap()
            .starts_with("bad.iris:1:")
    );

    let handle = SpecHandle::parse(b"n: int  repeat n { x: int }  check x == 1", "c.iris").unwrap();
    let error = envelope(handle.read(b"2\n1\n2\n", "c.txt", vec![]).err().unwrap());
    let document: Json = serde_json::from_str(error["document"].as_str().unwrap()).unwrap();
    assert_eq!(document["code"], "E-CHECK-FAILED");
    assert_eq!(document["record_path"], serde_json::json!([1]));
    assert_eq!(document["input_location"]["origin"], "c.txt");
    assert_eq!(document["spec_location"]["origin"], "c.iris");
}

#[test]
fn decode_rejects_truncated_and_inconsistent_buffers() {
    let handle = SpecHandle::parse(PROBE_SPEC.as_bytes(), "probe.iris").unwrap();
    let encoded = handle
        .read(PROBE_INPUT.as_bytes(), "probe.txt", vec![])
        .unwrap();
    for length in 0..encoded.len() {
        assert!(
            transfer::decode(&encoded[..length]).is_err(),
            "prefix {length}"
        );
    }
    let mut trailing = encoded.clone();
    trailing.extend([0; 8]);
    assert_eq!(
        transfer::decode(&trailing).err(),
        Some(transfer::TransferError::TrailingBytes)
    );
    let mut version = encoded.clone();
    version[0] = 9;
    assert_eq!(
        transfer::decode(&version).err(),
        Some(transfer::TransferError::UnknownVersion(9))
    );
    // Corrupt each byte in turn; decoding may succeed or fail, but never panics.
    for index in 0..encoded.len() {
        let mut corrupt = encoded.clone();
        corrupt[index] ^= 0xff;
        let _ = transfer::decode(&corrupt);
    }
    assert_eq!(
        envelope(render_json(&encoded[..9]).unwrap_err())["kind"],
        "host"
    );
}
