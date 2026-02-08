use orsx::prelude::*;

#[orsx::orsx_flatten_module]
mod outputs {
    use super::*;

    pub struct Fam {
        pub a: f64,
    }

    #[orsx_processor_id("proc_a")]
    pub struct Out {
        pub pair: String,

        #[orsx_family(prefix = "ma_")]
        pub fam: Option<Fam>,
    }
}

fn main() {}

