use orsx::prelude::*;
use orsx::columnar::OrsxColumnar;

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

fn main() {
    let _ = outputs::Out::COLUMNS_IN_ORDER;
    let _ = outputs::Out::METRIC_COLUMNS_IN_ORDER;
    let _ = outputs::Out::SCHEMA_HASH;
    let _ = outputs::Out::columnar_schema().unwrap();
    let _ = outputs::Out::spec();
}
