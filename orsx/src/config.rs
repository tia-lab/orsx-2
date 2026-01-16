#[derive(Debug, Clone)]
pub struct Config {
    pub test_database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let test_database_url = std::env::var("ORSX_TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://orsx:orsx@localhost:15432/orsx2_test".to_string());
        Self { test_database_url }
    }
}

