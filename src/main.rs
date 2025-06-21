use error_mapper::TheResult;
use the_logger::{log_info, TheLogger};

use crate::modules::config::Config;

mod modules;
mod utils;

#[tokio::main]
async fn main() {
    if let Err(error) = init_app().await {
        std::panic::panic_any(error.to_string())
    }
}

async fn init_app() -> TheResult<()> {
    let logger = TheLogger::instance();
    log_info!(logger, "Loading JSON configuration");

    Config::load()?;

    log_info!(logger, "Initializing Api");
    modules::api::start_api().await?;

    Ok(())
}
