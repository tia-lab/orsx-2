use orsx::columnar::{
    decode_orsxcol_v2_into_with_workspace, encode_orsxcol_v2_into_with_workspace, ColumnarBatch,
    ColumnarField, ColumnarSchema, ColumnarType, CopyBinaryBatchReader, OrsxcolV2DecodeWorkspace,
    OrsxcolV2EncodeWorkspace,
};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{Connection, Executor, Row};
use std::io::{Read, Write};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressionCodec {
    None,
    Gzip,
    Zstd,
}

fn env_codec(name: &str, default: CompressionCodec) -> CompressionCodec {
    match std::env::var(name).ok().as_deref() {
        None => default,
        Some("none") => CompressionCodec::None,
        Some("gzip") => CompressionCodec::Gzip,
        Some("zstd") => CompressionCodec::Zstd,
        Some(_) => default,
    }
}

fn env_i32(name: &str, default: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(default)
}

fn compress_bytes(codec: CompressionCodec, level: i32, input: &[u8]) -> Vec<u8> {
    match codec {
        CompressionCodec::None => input.to_vec(),
        CompressionCodec::Gzip => {
            let lvl: u32 = if level <= 0 { 6 } else { level as u32 };
            let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(lvl));
            enc.write_all(input).unwrap();
            enc.finish().unwrap()
        }
        CompressionCodec::Zstd => {
            let lvl: i32 = if level == 0 { 3 } else { level };
            zstd::stream::encode_all(std::io::Cursor::new(input), lvl).unwrap()
        }
    }
}

fn decompress_bytes(codec: CompressionCodec, input: &[u8]) -> Vec<u8> {
    match codec {
        CompressionCodec::None => input.to_vec(),
        CompressionCodec::Gzip => {
            let mut dec = flate2::read::GzDecoder::new(input);
            let mut out = Vec::new();
            dec.read_to_end(&mut out).unwrap();
            out
        }
        CompressionCodec::Zstd => zstd::stream::decode_all(std::io::Cursor::new(input)).unwrap(),
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_bool_01(name: &str, default: bool) -> bool {
    match std::env::var(name).ok().as_deref() {
        None => default,
        Some("0") => false,
        Some("1") => true,
        Some(_) => default,
    }
}

fn transfer_duration(bytes: usize, rtt_ms: u64, bandwidth_mbit: u64) -> std::time::Duration {
    let bandwidth_mbit = bandwidth_mbit.max(1);
    let secs = (rtt_ms as f64 / 1000.0)
        + ((bytes as f64 * 8.0) / (bandwidth_mbit as f64 * 1_000_000.0));
    std::time::Duration::from_secs_f64(secs)
}

fn checksum_mix_u64(mut h: u64, x: u64) -> u64 {
    // Deterministic, dependency-free mixing; not cryptographic.
    h ^= x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    h = h.rotate_left(17).wrapping_mul(0x85eb_ca6b);
    h
}

fn checksum_mix_bytes(mut h: u64, bytes: &[u8]) -> u64 {
    for &b in bytes {
        h = checksum_mix_u64(h, b as u64);
    }
    h
}

fn canon_f64_q9(x: f64) -> i64 {
    // Canonicalize floats for cross-format equality (JSON vs binary).
    // 9 decimal digits is intentionally tolerant of JSON float drift (which can be ~1 ulp).
    if !x.is_finite() {
        return i64::MIN;
    }
    let scaled = x * 1_000_000_000.0_f64;
    if !scaled.is_finite() {
        return i64::MIN.wrapping_add(1);
    }
    let rounded = scaled.round();
    if rounded < (i64::MIN as f64) || rounded > (i64::MAX as f64) {
        return i64::MIN.wrapping_add(2);
    }
    rounded as i64
}

fn mix_opt_f64_canon(h: u64, x: Option<f64>, null_sentinel: i64) -> u64 {
    match x {
        Some(v) => checksum_mix_u64(h, canon_f64_q9(v) as u64),
        None => checksum_mix_u64(h, null_sentinel as u64),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BarsLikeRow {
    id: i64,
    e_ms: i64,
    o: Option<f64>,
    h: Option<f64>,
    l: Option<f64>,
    c: Option<f64>,
    v: Option<f64>,
    pair: Option<String>,
    tf: Option<String>,
}

fn checksum_bars_like(rows: &[BarsLikeRow]) -> u64 {
    let mut h: u64 = 0;
    for row in rows {
        h = checksum_mix_u64(h, row.id as u64);
        h = checksum_mix_u64(h, row.e_ms as u64);
        h = mix_opt_f64_canon(h, row.o, 0x1111_1111_1111_1111u64 as i64);
        h = mix_opt_f64_canon(h, row.h, 0x2222_2222_2222_2222u64 as i64);
        h = mix_opt_f64_canon(h, row.l, 0x3333_3333_3333_3333u64 as i64);
        h = mix_opt_f64_canon(h, row.c, 0x4444_4444_4444_4444u64 as i64);
        h = mix_opt_f64_canon(h, row.v, 0x5555_5555_5555_5555u64 as i64);
        if let Some(s) = row.pair.as_deref() {
            h = checksum_mix_bytes(h, s.as_bytes());
        } else {
            h = checksum_mix_u64(h, 0x6666_6666_6666_6666);
        }
        if let Some(s) = row.tf.as_deref() {
            h = checksum_mix_bytes(h, s.as_bytes());
        } else {
            h = checksum_mix_u64(h, 0x7777_7777_7777_7777);
        }
    }
    h
}

fn debug_row_bits(label: &str, idx: usize, r: &BarsLikeRow) {
    eprintln!(
        "{label}[{idx}]: id={} e_ms={} o={:?} h={:?} l={:?} c={:?} v={:?} pair={:?} tf={:?}",
        r.id,
        r.e_ms,
        r.o.map(|x| x.to_bits()),
        r.h.map(|x| x.to_bits()),
        r.l.map(|x| x.to_bits()),
        r.c.map(|x| x.to_bits()),
        r.v.map(|x| x.to_bits()),
        r.pair.as_deref(),
        r.tf.as_deref()
    );
}

fn checksum_bars_like_columnar(
    batch: &ColumnarBatch,
    rows: usize,
    pair_buf: &mut Vec<u8>,
    tf_buf: &mut Vec<u8>,
) -> u64 {
    let mut h: u64 = 0;

    // 0: id (i64, non-null)
    let id_valid = batch.column_validity_bytes(0).unwrap();
    let id_vals = batch.fixed_i64(0).unwrap();
    // 1: e_ms (i64, non-null)
    let e_valid = batch.column_validity_bytes(1).unwrap();
    let e_vals = batch.fixed_i64(1).unwrap();

    // 2..=6: o,h,l,c,v (f64 bits, nullable)
    let o_valid = batch.column_validity_bytes(2).unwrap();
    let o_vals = batch.fixed_f64_bits(2).unwrap();
    let h_valid = batch.column_validity_bytes(3).unwrap();
    let h_vals = batch.fixed_f64_bits(3).unwrap();
    let l_valid = batch.column_validity_bytes(4).unwrap();
    let l_vals = batch.fixed_f64_bits(4).unwrap();
    let c_valid = batch.column_validity_bytes(5).unwrap();
    let c_vals = batch.fixed_f64_bits(5).unwrap();
    let v_valid = batch.column_validity_bytes(6).unwrap();
    let v_vals = batch.fixed_f64_bits(6).unwrap();

    // 7: pair utf8, nullable
    let pair_valid = batch.column_validity_bytes(7).unwrap();
    let (pair_offsets, _pair_chunks, _pair_total) = batch.var_chunks(7).unwrap();
    batch.coalesce_var_into(7, pair_buf).unwrap();
    // 8: tf utf8, nullable
    let tf_valid = batch.column_validity_bytes(8).unwrap();
    let (tf_offsets, _tf_chunks, _tf_total) = batch.var_chunks(8).unwrap();
    batch.coalesce_var_into(8, tf_buf).unwrap();

    for row in 0..rows {
        let id_is_valid = (id_valid[row / 8] & (1u8 << (row % 8))) != 0;
        let e_is_valid = (e_valid[row / 8] & (1u8 << (row % 8))) != 0;
        if id_is_valid {
            h = checksum_mix_u64(h, id_vals[row] as u64);
        } else {
            h = checksum_mix_u64(h, 0xaaaa_aaaa_aaaa_aaaa);
        }
        if e_is_valid {
            h = checksum_mix_u64(h, e_vals[row] as u64);
        } else {
            h = checksum_mix_u64(h, 0xbbbb_bbbb_bbbb_bbbb);
        }

        let bit = 1u8 << (row % 8);
        if (o_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, canon_f64_q9(f64::from_bits(o_vals[row])) as u64);
        } else {
            h = checksum_mix_u64(h, 0x1111_1111_1111_1111);
        }
        if (h_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, canon_f64_q9(f64::from_bits(h_vals[row])) as u64);
        } else {
            h = checksum_mix_u64(h, 0x2222_2222_2222_2222);
        }
        if (l_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, canon_f64_q9(f64::from_bits(l_vals[row])) as u64);
        } else {
            h = checksum_mix_u64(h, 0x3333_3333_3333_3333);
        }
        if (c_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, canon_f64_q9(f64::from_bits(c_vals[row])) as u64);
        } else {
            h = checksum_mix_u64(h, 0x4444_4444_4444_4444);
        }
        if (v_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, canon_f64_q9(f64::from_bits(v_vals[row])) as u64);
        } else {
            h = checksum_mix_u64(h, 0x5555_5555_5555_5555);
        }

        if (pair_valid[row / 8] & (1u8 << (row % 8))) != 0 {
            let start = pair_offsets[row] as usize;
            let end = pair_offsets[row + 1] as usize;
            h = checksum_mix_bytes(h, &pair_buf[start..end]);
        } else {
            h = checksum_mix_u64(h, 0x6666_6666_6666_6666);
        }

        if (tf_valid[row / 8] & (1u8 << (row % 8))) != 0 {
            let start = tf_offsets[row] as usize;
            let end = tf_offsets[row + 1] as usize;
            h = checksum_mix_bytes(h, &tf_buf[start..end]);
        } else {
            h = checksum_mix_u64(h, 0x7777_7777_7777_7777);
        }
    }

    h
}

fn checksum_bars_like_columnar_bits(
    batch: &ColumnarBatch,
    rows: usize,
    pair_buf: &mut Vec<u8>,
    tf_buf: &mut Vec<u8>,
) -> u64 {
    let mut h: u64 = 0;

    // 0: id (i64, non-null)
    let id_valid = batch.column_validity_bytes(0).unwrap();
    let id_vals = batch.fixed_i64(0).unwrap();
    // 1: e_ms (i64, non-null)
    let e_valid = batch.column_validity_bytes(1).unwrap();
    let e_vals = batch.fixed_i64(1).unwrap();

    // 2..=6: o,h,l,c,v (f64 bits, nullable)
    let o_valid = batch.column_validity_bytes(2).unwrap();
    let o_vals = batch.fixed_f64_bits(2).unwrap();
    let h_valid = batch.column_validity_bytes(3).unwrap();
    let h_vals = batch.fixed_f64_bits(3).unwrap();
    let l_valid = batch.column_validity_bytes(4).unwrap();
    let l_vals = batch.fixed_f64_bits(4).unwrap();
    let c_valid = batch.column_validity_bytes(5).unwrap();
    let c_vals = batch.fixed_f64_bits(5).unwrap();
    let v_valid = batch.column_validity_bytes(6).unwrap();
    let v_vals = batch.fixed_f64_bits(6).unwrap();

    // 7: pair utf8, nullable
    let pair_valid = batch.column_validity_bytes(7).unwrap();
    let (pair_offsets, _pair_chunks, _pair_total) = batch.var_chunks(7).unwrap();
    batch.coalesce_var_into(7, pair_buf).unwrap();
    // 8: tf utf8, nullable
    let tf_valid = batch.column_validity_bytes(8).unwrap();
    let (tf_offsets, _tf_chunks, _tf_total) = batch.var_chunks(8).unwrap();
    batch.coalesce_var_into(8, tf_buf).unwrap();

    for row in 0..rows {
        let id_is_valid = (id_valid[row / 8] & (1u8 << (row % 8))) != 0;
        let e_is_valid = (e_valid[row / 8] & (1u8 << (row % 8))) != 0;
        if id_is_valid {
            h = checksum_mix_u64(h, id_vals[row] as u64);
        } else {
            h = checksum_mix_u64(h, 0xaaaa_aaaa_aaaa_aaaa);
        }
        if e_is_valid {
            h = checksum_mix_u64(h, e_vals[row] as u64);
        } else {
            h = checksum_mix_u64(h, 0xbbbb_bbbb_bbbb_bbbb);
        }

        let bit = 1u8 << (row % 8);
        if (o_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, o_vals[row]);
        } else {
            h = checksum_mix_u64(h, 0x1111_1111_1111_1111);
        }
        if (h_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, h_vals[row]);
        } else {
            h = checksum_mix_u64(h, 0x2222_2222_2222_2222);
        }
        if (l_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, l_vals[row]);
        } else {
            h = checksum_mix_u64(h, 0x3333_3333_3333_3333);
        }
        if (c_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, c_vals[row]);
        } else {
            h = checksum_mix_u64(h, 0x4444_4444_4444_4444);
        }
        if (v_valid[row / 8] & bit) != 0 {
            h = checksum_mix_u64(h, v_vals[row]);
        } else {
            h = checksum_mix_u64(h, 0x5555_5555_5555_5555);
        }

        if (pair_valid[row / 8] & (1u8 << (row % 8))) != 0 {
            let start = pair_offsets[row] as usize;
            let end = pair_offsets[row + 1] as usize;
            h = checksum_mix_bytes(h, &pair_buf[start..end]);
        } else {
            h = checksum_mix_u64(h, 0x6666_6666_6666_6666);
        }

        if (tf_valid[row / 8] & (1u8 << (row % 8))) != 0 {
            let start = tf_offsets[row] as usize;
            let end = tf_offsets[row + 1] as usize;
            h = checksum_mix_bytes(h, &tf_buf[start..end]);
        } else {
            h = checksum_mix_u64(h, 0x7777_7777_7777_7777);
        }
    }

    h
}

async fn setup_bars_like_table(url: &str, rows: usize) {
    let mut conn = sqlx::PgConnection::connect(url).await.unwrap();
    conn.execute("DROP TABLE IF EXISTS orsx_api_perf")
        .await
        .unwrap();
    conn.execute(
        "CREATE TABLE orsx_api_perf (\
            id BIGINT PRIMARY KEY,\
            e_ms BIGINT NOT NULL,\
            o DOUBLE PRECISION NULL,\
            h DOUBLE PRECISION NULL,\
            l DOUBLE PRECISION NULL,\
            c DOUBLE PRECISION NULL,\
            v DOUBLE PRECISION NULL,\
            pair TEXT NULL,\
            tf TEXT NULL\
        )",
    )
    .await
    .unwrap();

    let insert_sql = "\
        INSERT INTO orsx_api_perf \
        SELECT \
            gs::bigint AS id, \
            (1700000000000::bigint + gs::bigint * 60000::bigint) AS e_ms, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.01 + 1.0) END AS o, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.01 + 2.0) END AS h, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.01 + 0.5) END AS l, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.01 + 1.5) END AS c, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE (gs::double precision * 0.1) END AS v, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE 'BTCUSDT' END AS pair, \
            CASE WHEN gs % 10 = 0 THEN NULL ELSE '1m' END AS tf \
        FROM generate_series(1, $1) gs";
    sqlx::query(insert_sql)
        .bind(rows as i64)
        .execute(&mut conn)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore]
async fn columnar_api_pipeline_trials() {
    let url = std::env::var("ORSX_TEST_DATABASE_URL")
        .expect("ORSX_TEST_DATABASE_URL must be set (do not hard-code defaults in this harness)");
    assert!(
        !url.contains(":1364"),
        "refusing to access production DB (localhost:1364)"
    );

    let shape = std::env::var("ORSX_PIPELINE_SHAPE").unwrap_or_else(|_| "bars_like".to_string());
    let runs = env_usize("ORSX_PIPELINE_RUNS", 3).max(1);
    let rows = env_usize("ORSX_PIPELINE_ROWS", 2000).max(1);

    let rtt_ms = env_u64("ORSX_NET_RTT_MS", 30);
    let bandwidth_mbit = env_u64("ORSX_NET_BANDWIDTH_MBIT", 100);
    let net_sleep = env_bool_01("ORSX_NET_SLEEP", false);

    let codec = env_codec("ORSX_PIPELINE_COMPRESS", CompressionCodec::None);
    let gzip_level = env_i32("ORSX_GZIP_LEVEL", 6);
    let zstd_level = env_i32("ORSX_ZSTD_LEVEL", 3);
    let level = match codec {
        CompressionCodec::None => 0,
        CompressionCodec::Gzip => gzip_level,
        CompressionCodec::Zstd => zstd_level,
    };

    if shape != "bars_like" {
        panic!("unsupported ORSX_PIPELINE_SHAPE={shape} (supported: bars_like)");
    }

    setup_bars_like_table(url.as_str(), rows).await;

    // Query text (shared).
    let select_sql = "\
        SELECT id, e_ms, o, h, l, c, v, pair, tf \
        FROM orsx_api_perf \
        ORDER BY id";

    // ORSX schema (must match select-list order).
    let schema = ColumnarSchema::new(vec![
        ColumnarField {
            name: Some("id".to_string()),
            ty: ColumnarType::I64,
        },
        ColumnarField {
            name: Some("e_ms".to_string()),
            ty: ColumnarType::I64,
        },
        ColumnarField {
            name: Some("o".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("h".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("l".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("c".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("v".to_string()),
            ty: ColumnarType::F64,
        },
        ColumnarField {
            name: Some("pair".to_string()),
            ty: ColumnarType::Utf8,
        },
        ColumnarField {
            name: Some("tf".to_string()),
            ty: ColumnarType::Utf8,
        },
    ])
    .unwrap();

    let mut encode_ws = OrsxcolV2EncodeWorkspace::default();
    let mut decode_ws = OrsxcolV2DecodeWorkspace::default();

    let enable_dict_utf8 = env_bool_01("ORSXCOL2_ENABLE_DICT_UTF8", false);
    let enable_delta_i64 = env_bool_01("ORSXCOL2_ENABLE_DELTA_VARINT_I64", false);
    encode_ws
        .set_enable_dict_utf8(enable_dict_utf8)
        .set_enable_delta_varint_i64(enable_delta_i64);

    let mut json_out = Vec::<u8>::new();
    let mut ors_out = Vec::<u8>::new();
    let mut json_wire: Vec<u8>;
    let mut ors_wire: Vec<u8>;
    let mut json_wire_decomp: Vec<u8>;
    let mut ors_wire_decomp: Vec<u8>;

    // Client-side ORSX decode target (reused).
    let mut ors_client_batch = ColumnarBatch::new(schema.clone(), rows.max(1)).unwrap();
    let mut pair_buf = Vec::<u8>::new();
    let mut tf_buf = Vec::<u8>::new();

    let mut conn_json = sqlx::PgConnection::connect(&url).await.unwrap();
    let mut conn_copy = sqlx::PgConnection::connect(&url).await.unwrap();

    for run_idx in 0..runs {
        // -----------------------------
        // JSON pipeline
        // -----------------------------
        let t0 = std::time::Instant::now();
        let mut stream = sqlx::query(select_sql).fetch(&mut conn_json);
        let mut json_rows = Vec::<BarsLikeRow>::with_capacity(rows);
        while let Some(row) = stream.try_next().await.unwrap() {
            json_rows.push(BarsLikeRow {
                id: row.try_get::<i64, _>(0).unwrap(),
                e_ms: row.try_get::<i64, _>(1).unwrap(),
                o: row.try_get::<Option<f64>, _>(2).unwrap(),
                h: row.try_get::<Option<f64>, _>(3).unwrap(),
                l: row.try_get::<Option<f64>, _>(4).unwrap(),
                c: row.try_get::<Option<f64>, _>(5).unwrap(),
                v: row.try_get::<Option<f64>, _>(6).unwrap(),
                pair: row.try_get::<Option<String>, _>(7).unwrap(),
                tf: row.try_get::<Option<String>, _>(8).unwrap(),
            });
        }
        assert_eq!(json_rows.len(), rows);
        let dt_json_db_fetch = t0.elapsed();

        let t1 = std::time::Instant::now();
        json_out.clear();
        serde_json::to_writer(&mut json_out, &json_rows).unwrap();
        let dt_json_server_encode = t1.elapsed();
        let bytes_json = json_out.len();

        let (json_client_rows, dt_json_server_compress, bytes_json_wire, t_transfer_json_wire, dt_json_client_decompress, dt_json_client_parse) =
            if codec == CompressionCodec::None {
                let bytes_wire = bytes_json;
                let t_transfer = transfer_duration(bytes_wire, rtt_ms, bandwidth_mbit);
                if net_sleep {
                    tokio::time::sleep(t_transfer).await;
                }
                let t_parse = std::time::Instant::now();
                let client_rows: Vec<BarsLikeRow> = serde_json::from_slice(&json_out).unwrap();
                let dt_parse = t_parse.elapsed();
                (
                    client_rows,
                    Duration::ZERO,
                    bytes_wire,
                    t_transfer,
                    Duration::ZERO,
                    dt_parse,
                )
            } else {
                let t_comp = std::time::Instant::now();
                json_wire = compress_bytes(codec, level, &json_out);
                let dt_comp = t_comp.elapsed();

                let bytes_wire = json_wire.len();
                let t_transfer = transfer_duration(bytes_wire, rtt_ms, bandwidth_mbit);
                if net_sleep {
                    tokio::time::sleep(t_transfer).await;
                }

                let t_decomp = std::time::Instant::now();
                json_wire_decomp = decompress_bytes(codec, &json_wire);
                let dt_decomp = t_decomp.elapsed();

                let t_parse = std::time::Instant::now();
                let client_rows: Vec<BarsLikeRow> =
                    serde_json::from_slice(&json_wire_decomp).unwrap();
                let dt_parse = t_parse.elapsed();

                (
                    client_rows,
                    dt_comp,
                    bytes_wire,
                    t_transfer,
                    dt_decomp,
                    dt_parse,
                )
            };

        let json_db_checksum = checksum_bars_like(&json_rows);
        let json_checksum = checksum_bars_like(&json_client_rows);
        if json_db_checksum != json_checksum {
            let mut first_mismatch: Option<usize> = None;
            for i in 0..rows {
                let a = &json_rows[i];
                let b = &json_client_rows[i];
                let eq = a.id == b.id
                    && a.e_ms == b.e_ms
                    && a.o.map(|x| x.to_bits()) == b.o.map(|x| x.to_bits())
                    && a.h.map(|x| x.to_bits()) == b.h.map(|x| x.to_bits())
                    && a.l.map(|x| x.to_bits()) == b.l.map(|x| x.to_bits())
                    && a.c.map(|x| x.to_bits()) == b.c.map(|x| x.to_bits())
                    && a.v.map(|x| x.to_bits()) == b.v.map(|x| x.to_bits())
                    && a.pair.as_deref() == b.pair.as_deref()
                    && a.tf.as_deref() == b.tf.as_deref();
                if !eq {
                    first_mismatch = Some(i);
                    break;
                }
            }
            if let Some(i) = first_mismatch {
                debug_row_bits("db", i, &json_rows[i]);
                debug_row_bits("client", i, &json_client_rows[i]);
            } else {
                let show = rows.min(5);
                for i in 0..show {
                    debug_row_bits("db", i, &json_rows[i]);
                    debug_row_bits("client", i, &json_client_rows[i]);
                }
            }
            panic!(
                "json did not round-trip (db vs client) on run {run_idx}: db_checksum={json_db_checksum} client_checksum={json_checksum}"
            );
        }

        // -----------------------------
        // ORSX pipeline
        // -----------------------------
        let t5 = std::time::Instant::now();
        let mut reader =
            CopyBinaryBatchReader::new_select_unchecked(&mut conn_copy, select_sql, schema.clone())
                .await
                .unwrap();
        let mut server_batch = ColumnarBatch::new(schema.clone(), rows.max(1)).unwrap();
        let got_rows = reader.next_batch_into(&mut server_batch).await.unwrap();
        assert_eq!(got_rows, rows);
        // Ensure the COPY stream is fully consumed so the connection can be reused.
        let mut drain_batch = ColumnarBatch::new(schema.clone(), 1).unwrap();
        let got_rows_tail = reader.next_batch_into(&mut drain_batch).await.unwrap();
        assert_eq!(got_rows_tail, 0);
        let dt_ors_db_fetch = t5.elapsed();

        let t6 = std::time::Instant::now();
        ors_out.clear();
        encode_orsxcol_v2_into_with_workspace(&server_batch, &mut ors_out, &mut encode_ws).unwrap();
        let dt_ors_server_encode = t6.elapsed();
        let bytes_ors = ors_out.len();

        let (dt_ors_server_compress, bytes_ors_wire, t_transfer_ors_wire, dt_ors_client_decompress, dt_ors_client_decode) =
            if codec == CompressionCodec::None {
                let bytes_wire = bytes_ors;
                let t_transfer = transfer_duration(bytes_wire, rtt_ms, bandwidth_mbit);
                if net_sleep {
                    tokio::time::sleep(t_transfer).await;
                }

                let t_decode = std::time::Instant::now();
                decode_orsxcol_v2_into_with_workspace(&ors_out, &mut ors_client_batch, &mut decode_ws)
                    .unwrap();
                let dt_decode = t_decode.elapsed();

                (
                    Duration::ZERO,
                    bytes_wire,
                    t_transfer,
                    Duration::ZERO,
                    dt_decode,
                )
            } else {
                let t_comp = std::time::Instant::now();
                ors_wire = compress_bytes(codec, level, &ors_out);
                let dt_comp = t_comp.elapsed();

                let bytes_wire = ors_wire.len();
                let t_transfer = transfer_duration(bytes_wire, rtt_ms, bandwidth_mbit);
                if net_sleep {
                    tokio::time::sleep(t_transfer).await;
                }

                let t_decomp = std::time::Instant::now();
                ors_wire_decomp = decompress_bytes(codec, &ors_wire);
                let dt_decomp = t_decomp.elapsed();

                let t_decode = std::time::Instant::now();
                decode_orsxcol_v2_into_with_workspace(
                    &ors_wire_decomp,
                    &mut ors_client_batch,
                    &mut decode_ws,
                )
                .unwrap();
                let dt_decode = t_decode.elapsed();

                (
                    dt_comp,
                    bytes_wire,
                    t_transfer,
                    dt_decomp,
                    dt_decode,
                )
            };

        // ORSXCOL2 must round-trip bitwise between server and client batches.
        let ors_server_bits_checksum =
            checksum_bars_like_columnar_bits(&server_batch, rows, &mut pair_buf, &mut tf_buf);
        let ors_client_bits_checksum =
            checksum_bars_like_columnar_bits(&ors_client_batch, rows, &mut pair_buf, &mut tf_buf);
        assert_eq!(
            ors_server_bits_checksum, ors_client_bits_checksum,
            "orsxcol_v2 did not round-trip bitwise (server vs client) on run {run_idx}"
        );

        // Cross-format correctness: JSON vs ORSX are compared via canonicalized floats.
        let ors_checksum =
            checksum_bars_like_columnar(&ors_client_batch, rows, &mut pair_buf, &mut tf_buf);

        assert_eq!(
            json_checksum, ors_checksum,
            "checksum mismatch (json vs ors) on run {run_idx}"
        );

        let json_cpu_total =
            dt_json_db_fetch + dt_json_server_encode + dt_json_server_compress + dt_json_client_decompress + dt_json_client_parse;
        let ors_cpu_total =
            dt_ors_db_fetch + dt_ors_server_encode + dt_ors_server_compress + dt_ors_client_decompress + dt_ors_client_decode;

        let json_est_total_wire = json_cpu_total + t_transfer_json_wire;
        let ors_est_total_wire = ors_cpu_total + t_transfer_ors_wire;

        eprintln!(
            "api_pipeline shape={shape} run={run_idx}/{runs} rows={rows} \
            net_rtt_ms={rtt_ms} net_mbit={bandwidth_mbit} net_sleep={net_sleep} \
            compress={:?} level={level} \
            orsxcol2_dict_utf8={enable_dict_utf8} orsxcol2_delta_i64={enable_delta_i64} \
            | json: db_fetch={:?} server_encode={:?} server_compress={:?} client_decompress={:?} client_parse={:?} cpu_total={:?} bytes_raw={} bytes_wire={} t_transfer={:?} t_est_total={:?} checksum={} \
            | ors_v2: db_fetch={:?} server_encode={:?} server_compress={:?} client_decompress={:?} client_decode={:?} cpu_total={:?} bytes_raw={} bytes_wire={} t_transfer={:?} t_est_total={:?} checksum={}",
            codec,
            dt_json_db_fetch,
            dt_json_server_encode,
            dt_json_server_compress,
            dt_json_client_decompress,
            dt_json_client_parse,
            json_cpu_total,
            bytes_json,
            bytes_json_wire,
            t_transfer_json_wire,
            json_est_total_wire,
            json_checksum,
            dt_ors_db_fetch,
            dt_ors_server_encode,
            dt_ors_server_compress,
            dt_ors_client_decompress,
            dt_ors_client_decode,
            ors_cpu_total,
            bytes_ors,
            bytes_ors_wire,
            t_transfer_ors_wire,
            ors_est_total_wire,
            ors_checksum
        );
    }
}
