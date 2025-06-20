use error_mapper::{TheResult, create_new_error};
use serde::Deserialize;
use std::sync::OnceLock;

pub static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Deserialize)]
pub struct Config {
    pub api: ApiConfig,
}

#[derive(Deserialize)]
pub struct ApiConfig {
    pub url: String,
    pub port: u16,
    pub token: String,
}

impl Config {
    pub fn load() -> TheResult<()> {
        let content = std::fs::read_to_string("config/config.json")
            .map_err(|error| create_new_error!(error))?;
        let config =
            serde_json::from_str::<Config>(&content).map_err(|error| create_new_error!(error))?;

        let _ = CONFIG.get_or_init(|| config);

        Ok(())
    }

    pub fn get() -> TheResult<&'static Self> {
        CONFIG
            .get()
            .ok_or(create_new_error!("Could not get app configuration"))
    }
}
impl ApiConfig {
    pub fn get_bind(&self) -> (String, u16) {
        (self.url.clone(), self.port)
    }
}
