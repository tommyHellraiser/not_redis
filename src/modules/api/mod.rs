use crate::modules::{
    api::req_logger::RequestLogger,
    queuer::logic::{DequeueSyncInfo, NewJobInfo},
};
use actix_web::{App, HttpServer, dev::ServerHandle, web};
use error_mapper::{TheResult, create_new_error};
use the_logger::{TheLogger, log_info};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    mpsc::Sender as MpscSender,
};

use crate::modules::{self, api::middleware::AuthMiddleware, config::Config};

mod life_services;
mod middleware;
mod req_logger;

pub(super) struct ApiData {
    pub(super) stop_sender: Sender<()>,
    pub(super) dequeue_sender: MpscSender<DequeueSyncInfo>,
    pub(super) new_jobs_sender: MpscSender<NewJobInfo>,
}

pub async fn start_api(
    sender: Sender<()>,
    receiver: Receiver<()>,
    dequeue_sender: MpscSender<DequeueSyncInfo>,
    new_jobs_sender: MpscSender<NewJobInfo>,
) -> TheResult<()> {
    let app_config = Config::get()?;

    let server = HttpServer::new(move || {
        let stop_sender = sender.clone();
        let dequeue_sender = dequeue_sender.clone();
        let new_jobs_sender = new_jobs_sender.clone();
        App::new()
            .app_data(web::Data::new(ApiData {
                stop_sender,
                dequeue_sender,
                new_jobs_sender,
            }))
            .service(
                web::scope("/api")
                    .service(web::scope("/life").configure(life_services::services))
                    .service(web::scope("/queue").configure(modules::queuer::services)),
            )
            .wrap(AuthMiddleware)
            .wrap(RequestLogger)
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
