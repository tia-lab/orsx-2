use criterion::{criterion_group, criterion_main, Criterion};
use orsx::flatten::{OrsxValueVisitor, PgArgumentsVisitor};

#[orsx::orsx_flatten_module]
mod outputs {
    #[derive(Clone)]
    pub struct Fam {
        pub m0: f64,
        pub m1: f64,
        pub m2: f64,
        pub m3: f64,
        pub m4: f64,
        pub m5: f64,
        pub m6: f64,
        pub m7: f64,
        pub m8: f64,
        pub m9: f64,
        pub m10: f64,
        pub m11: f64,
        pub m12: f64,
        pub m13: f64,
        pub m14: f64,
        pub m15: f64,
        pub m16: f64,
        pub m17: f64,
        pub m18: f64,
        pub m19: f64,
        pub m20: f64,
        pub m21: f64,
        pub m22: f64,
        pub m23: f64,
        pub m24: f64,
        pub m25: f64,
        pub m26: f64,
        pub m27: f64,
        pub m28: f64,
        pub m29: f64,
        pub m30: f64,
        pub m31: f64,
        pub m32: f64,
        pub m33: f64,
        pub m34: f64,
        pub m35: f64,
        pub m36: f64,
        pub m37: f64,
        pub m38: f64,
        pub m39: f64,
        pub m40: f64,
        pub m41: f64,
        pub m42: f64,
        pub m43: f64,
        pub m44: f64,
        pub m45: f64,
        pub m46: f64,
        pub m47: f64,
        pub m48: f64,
        pub m49: f64,
        pub m50: f64,
        pub m51: f64,
        pub m52: f64,
        pub m53: f64,
        pub m54: f64,
        pub m55: f64,
        pub m56: f64,
        pub m57: f64,
        pub m58: f64,
        pub m59: f64,
        pub m60: f64,
        pub m61: f64,
        pub m62: f64,
        pub m63: f64,
    }

    #[orsx_processor_id("bench_proc")]
    #[derive(Clone)]
    pub struct Out {
        pub pair: String,
        pub e_ms: i64,

        #[orsx_family(prefix = "m_")]
        pub fam: Fam,
    }
}

#[derive(Default)]
struct RecordingVisitor {
    n: usize,
}

impl<'q> OrsxValueVisitor<'q> for RecordingVisitor {
    fn visit_i16(&mut self, _col: &'static str, _value: Option<i16>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_i32(&mut self, _col: &'static str, _value: Option<i32>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_i64(&mut self, _col: &'static str, _value: Option<i64>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_f32(&mut self, _col: &'static str, _value: Option<f32>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_f64(&mut self, _col: &'static str, _value: Option<f64>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_bool(&mut self, _col: &'static str, _value: Option<bool>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_text(&mut self, _col: &'static str, _value: Option<&'q str>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_bytes(&mut self, _col: &'static str, _value: Option<&'q [u8]>) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_uuid(
        &mut self,
        _col: &'static str,
        _value: Option<&'q orsx::sqlx::types::Uuid>,
    ) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_sqlx_timestamp(
        &mut self,
        _col: &'static str,
        _value: Option<&'q orsx::SqlxTimestamp>,
    ) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_json_value(
        &mut self,
        _col: &'static str,
        _value: Option<&'q orsx::sqlx::types::JsonValue>,
    ) -> orsx::Result<()> {
        self.n += 1;
        Ok(())
    }
    fn visit_json<T>(
        &mut self,
        _col: &'static str,
        _value: Option<&'q orsx::sqlx::types::Json<T>>,
    ) -> orsx::Result<()>
    where
        T: serde::Serialize + Send + Sync,
    {
        self.n += 1;
        Ok(())
    }
}

fn bench_flatten(c: &mut Criterion) {
    let out = outputs::Out {
        pair: "BTCUSDT".to_string(),
        e_ms: 123,
        fam: outputs::Fam {
            m0: 0.0,
            m1: 1.0,
            m2: 2.0,
            m3: 3.0,
            m4: 4.0,
            m5: 5.0,
            m6: 6.0,
            m7: 7.0,
            m8: 8.0,
            m9: 9.0,
            m10: 10.0,
            m11: 11.0,
            m12: 12.0,
            m13: 13.0,
            m14: 14.0,
            m15: 15.0,
            m16: 16.0,
            m17: 17.0,
            m18: 18.0,
            m19: 19.0,
            m20: 20.0,
            m21: 21.0,
            m22: 22.0,
            m23: 23.0,
            m24: 24.0,
            m25: 25.0,
            m26: 26.0,
            m27: 27.0,
            m28: 28.0,
            m29: 29.0,
            m30: 30.0,
            m31: 31.0,
            m32: 32.0,
            m33: 33.0,
            m34: 34.0,
            m35: 35.0,
            m36: 36.0,
            m37: 37.0,
            m38: 38.0,
            m39: 39.0,
            m40: 40.0,
            m41: 41.0,
            m42: 42.0,
            m43: 43.0,
            m44: 44.0,
            m45: 45.0,
            m46: 46.0,
            m47: 47.0,
            m48: 48.0,
            m49: 49.0,
            m50: 50.0,
            m51: 51.0,
            m52: 52.0,
            m53: 53.0,
            m54: 54.0,
            m55: 55.0,
            m56: 56.0,
            m57: 57.0,
            m58: 58.0,
            m59: 59.0,
            m60: 60.0,
            m61: 61.0,
            m62: 62.0,
            m63: 63.0,
        },
    };

    c.bench_function("flatten/visit_values_recording_66cols", |b| {
        b.iter(|| {
            let mut v = RecordingVisitor::default();
            out.visit_values_in_order(&mut v).unwrap();
            v.n
        })
    });

    c.bench_function("flatten/visit_values_pg_args_66cols", |b| {
        b.iter(|| {
            let mut v = PgArgumentsVisitor::new();
            v.reserve(outputs::Out::COLUMNS_IN_ORDER.len(), 0);
            out.visit_values_in_order(&mut v).unwrap();
            let args = v.into_arguments();
            orsx::sqlx::Arguments::len(&args)
        })
    });
}

criterion_group!(flatten_benches, bench_flatten);
criterion_main!(flatten_benches);
