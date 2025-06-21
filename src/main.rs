use error_mapper::TheResult;
use the_logger::{TheLogger, log_info};

use crate::modules::{config::Config, queuer::logic::SharedQueues};

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

    let (sender, receiver) = tokio::sync::broadcast::channel::<()>(5);
    SharedQueues::init();

    //  Initialize the workers manager, that will handle every worker and their shutdowns
    tokio::task::spawn(modules::queuer::logic::workers::workers_manager_handler(
        receiver.resubscribe(),
    ));

    log_info!(logger, "Initializing Api");
    modules::api::start_api(sender, receiver).await?;

    Ok(())
}
