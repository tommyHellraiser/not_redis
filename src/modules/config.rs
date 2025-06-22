use error_mapper::{TheResult, create_new_error};
use serde::Deserialize;
use std::sync::OnceLock;

pub static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Deserialize)]
pub struct Config {
    pub api: ApiConfig,
    pub queue: QueueConfig,
}

#[derive(Deserialize)]
pub struct ApiConfig {
    pub url: String,
    pub port: u16,
    pub token: String,
}

#[derive(Deserialize)]
pub struct QueueConfig {
    pub max_queue_amounts: u8,
    pub queue_size_limit: u16,
    pub workers_sleep_time_millis: u32,
    pub workers_kill_timeout_seconds: u16,
    pub dequeue_timeout_wait_seconds: u16,
    pub release: ReleaseConfig,
    pub main_queue_retry: MainQueueRetryConfig,
    pub retry_queue: RetryQueueConfig,
}

#[derive(Deserialize)]
pub struct ReleaseConfig {
    pub enable_automatic_release: bool,
    pub item_release_sleep_millis: u32,
    pub item_timeout: ItemTimeoutConfig,
}

#[derive(Deserialize)]
pub struct ItemTimeoutConfig {
    pub enable: bool,
    pub millis: u32,
}

#[derive(Deserialize)]
pub struct MainQueueRetryConfig {
    pub on_item_release_not_requested: u8,
    pub on_item_sleep_millis: u16,
}

#[derive(Deserialize)]
pub struct RetryQueueConfig {
    pub enable: bool,
    pub retry_amount: u8,
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
