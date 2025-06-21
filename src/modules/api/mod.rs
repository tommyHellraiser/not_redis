use crate::modules::queuer::logic::SharedQueues;
use actix_web::{App, HttpServer, dev::ServerHandle, web};
use error_mapper::{TheResult, create_new_error};
use the_logger::{TheLogger, log_info};
use tokio::sync::mpsc::{Receiver, Sender};

use crate::modules::{self, api::middleware::AuthMiddleware, config::Config};

mod life_services;
mod middleware;

pub(super) struct ApiData {
    pub(super) stop_sender: Sender<()>,
}

pub async fn start_api() -> TheResult<()> {
    let app_config = Config::get()?;

    let (sender, receiver) = tokio::sync::mpsc::channel::<()>(5);

    SharedQueues::init();

    let server = HttpServer::new(move || {
        let sender = sender.clone();
        App::new()
            .app_data(web::Data::new(ApiData {
                stop_sender: sender,
            }))
            .service(
                web::scope("/api")
                    .service(web::scope("/life").configure(life_services::services))
                    .service(web::scope("/queue").configure(modules::queuer::services)),
            )
            .wrap(AuthMiddleware)
    })
    .bind(app_config.api.get_bind())
    .map_err(|error| create_new_error!(error))?
    .run();

    tokio::task::spawn(stop_handler(server.handle(), receiver));

    server.await.map_err(|error| create_new_error!(error))
}

async fn stop_handler(server_handler: ServerHandle, mut stop_signal: Receiver<()>) {
    let _ = stop_signal.recv().await;

    let logger = TheLogger::instance();
    log_info!(logger, "Stop signal received!");

    server_handler.stop(true).await;
    log_info!(logger, "Service terminated. Shutting down..");
}
