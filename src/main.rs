use error_mapper::TheResult;
use the_logger::{TheLogger, log_info};

use crate::modules::{
    config::Config,
    queuer::logic::{QueueNameType, SharedQueues, UuidType},
};

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

    let (stop_sender, stop_receiver) = tokio::sync::broadcast::channel::<()>(5);
    //  Channel types are (String: Queue Name, String: UUID)
    //  Dequeue sender will go to the endpoints inside ApiData
    //  Dequeue receiver will go to the workers manager cron, to derive the work to the corresponding worker
    let (dequeue_sender, dequeue_receiver) =
        tokio::sync::mpsc::channel::<(QueueNameType, UuidType)>(10);
    SharedQueues::init();

    //  Initialize the workers manager, that will handle every worker and their shutdowns
    tokio::task::spawn(modules::queuer::logic::workers::workers_manager_handler(
        stop_receiver.resubscribe(),
    ));

    log_info!(logger, "Initializing Api");
    modules::api::start_api(stop_sender, stop_receiver, dequeue_sender).await?;

    Ok(())
}
