use orsx::prelude::*;

#[orsx::orsx_flatten_module]
mod outputs {
    use super::*;

    #[orsx_processor_id("Bad-Id")]
    pub struct Out {
        pub pair: String,
    }
}

fn main() {}

