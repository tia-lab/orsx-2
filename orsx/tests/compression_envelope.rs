use orsx::{Compressed, CompressedWorkspace};

#[test]
fn compression_roundtrip_f64() {
    let values: Vec<f64> = (0..10_000).map(|i| 100.0 + (i as f64) * 0.01).collect();
    let c = Compressed::new(values.clone());

    let mut out = Vec::new();
    let mut ws = CompressedWorkspace::default();
    c.encode_envelope_into(&mut out, &mut ws).unwrap();

    let decoded = Compressed::<f64>::decode_envelope(&out).unwrap();
    assert_eq!(decoded.as_slice(), &values[..]);
}

#[test]
fn compression_rejects_corrupt_checksum() {
    let values: Vec<i64> = (0..1000).map(|i| i as i64).collect();
    let c = Compressed::new(values);

    let mut out = Vec::new();
    let mut ws = CompressedWorkspace::default();
    c.encode_envelope_into(&mut out, &mut ws).unwrap();

    // Flip a byte in payload.
    let last = out.len() - 1;
    out[last] ^= 0xAA;

    let err = Compressed::<i64>::decode_envelope(&out).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("checksum"), "unexpected error: {msg}");
}

#[test]
fn compression_rejects_type_mismatch() {
    let values: Vec<i32> = (0..1000).map(|i| i as i32).collect();
    let c = Compressed::new(values);

    let mut out = Vec::new();
    let mut ws = CompressedWorkspace::default();
    c.encode_envelope_into(&mut out, &mut ws).unwrap();

    let err = Compressed::<i64>::decode_envelope(&out).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("mismatch"), "unexpected error: {msg}");
}

