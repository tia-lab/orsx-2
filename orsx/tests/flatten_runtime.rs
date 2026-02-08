use orsx::flatten::{OrsxValueVisitor, PgArgumentsVisitor};
use sha2::Digest;

#[orsx::orsx_flatten_module]
mod outputs {
    #[derive(Clone)]
    pub struct Nested {
        pub x: f64,
    }

    #[derive(Clone)]
    pub struct Fam {
        pub a: f64,
        pub b: Option<i64>,
        pub nested: Nested,
    }

    #[derive(Clone)]
    pub struct Osc {
        pub rsi: f64,
    }

    #[orsx_processor_id("proc_a")]
    #[derive(Clone)]
    pub struct Out {
        pub pair: String,
        pub e_ms: i64,

        #[orsx_family(prefix = "ma_")]
        pub fam: Fam,

        #[orsx_family(prefix = "osc_")]
        pub osc: Osc,
    }
}

#[derive(Default)]
struct RecordingVisitor {
    cols: Vec<&'static str>,
}

impl<'q> OrsxValueVisitor<'q> for RecordingVisitor {
    fn visit_i16(&mut self, col: &'static str, _value: Option<i16>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_i32(&mut self, col: &'static str, _value: Option<i32>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_i64(&mut self, col: &'static str, _value: Option<i64>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_f32(&mut self, col: &'static str, _value: Option<f32>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_f64(&mut self, col: &'static str, _value: Option<f64>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_bool(&mut self, col: &'static str, _value: Option<bool>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_text(&mut self, col: &'static str, _value: Option<&'q str>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_bytes(&mut self, col: &'static str, _value: Option<&'q [u8]>) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_uuid(
        &mut self,
        col: &'static str,
        _value: Option<&'q orsx::sqlx::types::Uuid>,
    ) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_sqlx_timestamp(
        &mut self,
        col: &'static str,
        _value: Option<&'q orsx::SqlxTimestamp>,
    ) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_json_value(
        &mut self,
        col: &'static str,
        _value: Option<&'q orsx::sqlx::types::JsonValue>,
    ) -> orsx::Result<()> {
        self.cols.push(col);
        Ok(())
    }
    fn visit_json<T>(
        &mut self,
        col: &'static str,
        _value: Option<&'q orsx::sqlx::types::Json<T>>,
    ) -> orsx::Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        self.cols.push(col);
        Ok(())
    }
}

#[test]
fn flattened_columns_and_visit_order_match() {
    assert_eq!(
        outputs::Out::COLUMNS_IN_ORDER,
        &["pair", "e_ms", "ma_a", "ma_b", "ma_nested_x", "osc_rsi"]
    );
    assert_eq!(
        outputs::Out::METRIC_COLUMNS_IN_ORDER,
        &["ma_a", "ma_b", "ma_nested_x", "osc_rsi"]
    );

    let out = outputs::Out {
        pair: "BTCUSDT".to_string(),
        e_ms: 123,
        fam: outputs::Fam {
            a: 1.0,
            b: Some(7),
            nested: outputs::Nested { x: 2.0 },
        },
        osc: outputs::Osc { rsi: 55.0 },
    };

    let mut rec = RecordingVisitor::default();
    out.visit_values_in_order(&mut rec).unwrap();
    assert_eq!(rec.cols.as_slice(), outputs::Out::COLUMNS_IN_ORDER);
}

#[test]
fn schema_hash_matches_runtime_reference() {
    let generation_version = "orsx_flatten_module_v1";
    let processor_id = "proc_a";
    let metrics: &[(&str, &str, bool)] = &[
        ("ma_a", "DoublePrecision", false),
        ("ma_b", "BigInt", true),
        ("ma_nested_x", "DoublePrecision", false),
        ("osc_rsi", "DoublePrecision", false),
    ];

    let mut hasher = sha2::Sha256::new();
    hasher.update(generation_version.as_bytes());
    hasher.update(b"\n");
    hasher.update(processor_id.as_bytes());
    hasher.update(b"\n");
    for (id, ty, nullable) in metrics {
        hasher.update(id.as_bytes());
        hasher.update(b":");
        hasher.update(ty.as_bytes());
        hasher.update(b":");
        hasher.update(if *nullable { b"1" } else { b"0" });
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut expected = [0u8; 32];
    expected.copy_from_slice(digest.as_ref());
    assert_eq!(outputs::Out::SCHEMA_HASH, expected);
}

#[test]
fn pg_arguments_visitor_adds_all_columns() {
    let out = outputs::Out {
        pair: "BTCUSDT".to_string(),
        e_ms: 123,
        fam: outputs::Fam {
            a: 1.0,
            b: Some(7),
            nested: outputs::Nested { x: 2.0 },
        },
        osc: outputs::Osc { rsi: 55.0 },
    };

    let mut v = PgArgumentsVisitor::new();
    out.visit_values_in_order(&mut v).unwrap();
    let args = v.into_arguments();
    assert_eq!(
        orsx::sqlx::Arguments::len(&args),
        outputs::Out::COLUMNS_IN_ORDER.len()
    );
}
