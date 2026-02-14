use orsx::columnar::{
    decode_orsxcol_v1, decode_orsxcol_v2, decode_orsxcol_v2_into_with_workspace,
    decode_orsxcol_v2_with_workspace, encode_orsxcol_v1_into, encode_orsxcol_v2_into,
    encode_orsxcol_v2_into_with_workspace, ColumnarBatch, ColumnarField, ColumnarSchema,
    ColumnarType, OrsxcolV2DecodeWorkspace, OrsxcolV2EncodeWorkspace,
};

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn checksum_batch(batch: &ColumnarBatch) -> u64 {
    let mut acc: u64 = 0;
    for (col_idx, f) in batch.schema().fields().iter().enumerate() {
        if let Some(validity) = batch.column_validity_bytes(col_idx) {
            for &b in validity {
                acc = acc.wrapping_add(b as u64);
                acc = acc.rotate_left(1);
            }
        }
        match f.ty {
            ColumnarType::I64 => {
                if let Some(values) = batch.fixed_i64(col_idx) {
                    for &v in values {
                        acc = acc.wrapping_add(v as u64);
                        acc = acc.rotate_left(1);
                    }
                }
            }
            ColumnarType::F64 => {
                if let Some(values) = batch.fixed_f64_bits(col_idx) {
                    for &bits in values {
                        acc ^= bits;
                        acc = acc.rotate_left(1);
                    }
                }
            }
            ColumnarType::Utf8 | ColumnarType::Bytes | ColumnarType::JsonbText => {
                if let Some((offsets, chunks, total_len)) = batch.var_chunks(col_idx) {
                    for &o in offsets {
                        acc = acc.wrapping_add(o as u64);
                        acc = acc.rotate_left(1);
                    }
                    if let Some(inline) = batch.var_inline_bytes(col_idx) {
                        for &b in inline {
                            acc = acc.wrapping_add(b as u64);
                            acc = acc.rotate_left(1);
                        }
                    }
                    for c in chunks {
                        for &b in c.as_ref() {
                            acc = acc.wrapping_add(b as u64);
                            acc = acc.rotate_left(1);
                        }
                    }
                    acc = acc.wrapping_add(total_len as u64);
                    acc = acc.rotate_left(1);
                }
            }
            _ => {}
        }
    }
    acc
}

fn build_synthetic_batch(rows: usize, cols: usize) -> ColumnarBatch {
    let cols = cols.max(3);
    let fcols = cols - 3;

    let mut fields = Vec::with_capacity(cols);
    fields.push(ColumnarField {
        name: Some("id".to_string()),
        ty: ColumnarType::I64,
    });
    for i in 1..=fcols {
        fields.push(ColumnarField {
            name: Some(format!("c{i:03}")),
            ty: ColumnarType::F64,
        });
    }
    fields.push(ColumnarField {
        name: Some("t".to_string()),
        ty: ColumnarType::Utf8,
    });
    fields.push(ColumnarField {
        name: Some("by".to_string()),
        ty: ColumnarType::Bytes,
    });

    let schema = ColumnarSchema::new(fields).unwrap();
    let mut batch = ColumnarBatch::new(schema, rows.max(1)).unwrap();

    if rows == 0 {
        return batch;
    }

    for row in 1..=rows {
        batch.push_i64(0, row as i64).unwrap();

        for col_idx in 1..=fcols {
            if row % 10 == 0 {
                batch.push_null(col_idx).unwrap();
            } else {
                let v = (row as f64) * 0.001 + (col_idx as f64);
                batch.push_f64_bits(col_idx, v.to_bits()).unwrap();
            }
        }

        if row % 10 == 0 {
            batch.push_null(fcols + 1).unwrap();
            batch.push_null(fcols + 2).unwrap();
        } else {
            batch.push_utf8(fcols + 1, "hello").unwrap();
            batch.push_var_bytes(fcols + 2, &[1u8, 2, 3]).unwrap();
        }

        batch.end_row().unwrap();
    }

    batch
}

#[test]
#[ignore]
fn orscol_transport_perf_trial_v1_vs_v2() {
    let rows = env_usize("ORSX_COL_ROWS", 100_000);
    let cols = env_usize("ORSX_COL_COLS", 50);
    let iters = env_usize("ORSX_TRANSPORT_ITERS", 5).max(1);

    let batch = build_synthetic_batch(rows, cols);
    let checksum_in = checksum_batch(&batch);

    let mut out_v1 = Vec::<u8>::new();
    let mut out_v2 = Vec::<u8>::new();
    let mut out_v2_ws = Vec::<u8>::new();
    let mut enc_ws = OrsxcolV2EncodeWorkspace::default();

    // Encode timings
    let t0 = std::time::Instant::now();
    for _ in 0..iters {
        encode_orsxcol_v1_into(&batch, &mut out_v1).unwrap();
        std::hint::black_box(out_v1.len());
    }
    let dt_v1_encode = t0.elapsed();

    let t1 = std::time::Instant::now();
    for _ in 0..iters {
        encode_orsxcol_v2_into(&batch, &mut out_v2).unwrap();
        std::hint::black_box(out_v2.len());
    }
    let dt_v2_encode = t1.elapsed();

    let t2 = std::time::Instant::now();
    for _ in 0..iters {
        encode_orsxcol_v2_into_with_workspace(&batch, &mut out_v2_ws, &mut enc_ws).unwrap();
        std::hint::black_box(out_v2_ws.len());
    }
    let dt_v2_encode_ws = t2.elapsed();

    // Decode timings (using the last produced bytes).
    let mut dec_ws = OrsxcolV2DecodeWorkspace::default();
    let t3 = std::time::Instant::now();
    let mut checksum_v1: u64 = 0;
    for _ in 0..iters {
        let decoded = decode_orsxcol_v1(&out_v1).unwrap();
        checksum_v1 ^= checksum_batch(&decoded);
        checksum_v1 = checksum_v1.rotate_left(1);
    }
    let dt_v1_decode = t3.elapsed();

    let t4 = std::time::Instant::now();
    let mut checksum_v2: u64 = 0;
    for _ in 0..iters {
        let decoded = decode_orsxcol_v2(&out_v2).unwrap();
        checksum_v2 ^= checksum_batch(&decoded);
        checksum_v2 = checksum_v2.rotate_left(1);
    }
    let dt_v2_decode = t4.elapsed();

    let t5 = std::time::Instant::now();
    let mut checksum_v2_ws: u64 = 0;
    for _ in 0..iters {
        let decoded = decode_orsxcol_v2_with_workspace(&out_v2_ws, &mut dec_ws).unwrap();
        checksum_v2_ws ^= checksum_batch(&decoded);
        checksum_v2_ws = checksum_v2_ws.rotate_left(1);
    }
    let dt_v2_decode_ws = t5.elapsed();

    // In-place decode (allocation-reuse hot path).
    let mut batch_v2_inplace = ColumnarBatch::new(batch.schema().clone(), rows.max(1)).unwrap();
    let t6 = std::time::Instant::now();
    let mut checksum_v2_inplace: u64 = 0;
    for _ in 0..iters {
        decode_orsxcol_v2_into_with_workspace(&out_v2_ws, &mut batch_v2_inplace, &mut dec_ws)
            .unwrap();
        checksum_v2_inplace ^= checksum_batch(&batch_v2_inplace);
        checksum_v2_inplace = checksum_v2_inplace.rotate_left(1);
    }
    let dt_v2_decode_inplace = t6.elapsed();

    // Correctness: at least one decode must match the input checksum.
    let decoded_once_v1 = decode_orsxcol_v1(&out_v1).unwrap();
    let decoded_once_v2 = decode_orsxcol_v2(&out_v2).unwrap();
    assert_eq!(checksum_batch(&decoded_once_v1), checksum_in);
    assert_eq!(checksum_batch(&decoded_once_v2), checksum_in);
    assert_eq!(checksum_batch(&batch_v2_inplace), checksum_in);

    eprintln!(
        "orscol_transport: rows={rows} cols={cols} iters={iters} bytes_v1={} bytes_v2={} bytes_v2_ws={} | v1_encode={:?} v2_encode={:?} v2_encode_ws={:?} | v1_decode={:?} v2_decode={:?} v2_decode_ws={:?} v2_decode_inplace={:?} | checksum_in={checksum_in} checksum_v1={checksum_v1} checksum_v2={checksum_v2} checksum_v2_ws={checksum_v2_ws} checksum_v2_inplace={checksum_v2_inplace}",
        out_v1.len(),
        out_v2.len(),
        out_v2_ws.len(),
        dt_v1_encode,
        dt_v2_encode,
        dt_v2_encode_ws,
        dt_v1_decode,
        dt_v2_decode,
        dt_v2_decode_ws,
        dt_v2_decode_inplace,
    );
}
