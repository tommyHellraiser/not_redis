use error_mapper::TheResult;

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
    Config::load()?;

    modules::api::start_api().await?;

    Ok(())
}
