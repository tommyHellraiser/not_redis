use actix_web::{
    HttpResponse, get, put,
    web::{self, ServiceConfig},
};
use the_logger::{TheLogger, log_error, log_info};

use crate::modules::api::ApiData;

pub(super) fn services(cfg: &mut ServiceConfig) {
    cfg.service(alive).service(stop);
}

#[get("alive")]
async fn alive() -> HttpResponse {
    HttpResponse::Ok().body("I'm alive!!!")
}

#[put("/stop")]
async fn stop(api_data: web::Data<ApiData>) -> HttpResponse {
    let logger = TheLogger::instance();
    log_info!(logger, "Stopping service...");

    if let Err(error) = api_data.stop_sender.send(()) {
        log_error!(logger, "Error sending stop signal: {}", error);
        return HttpResponse::InternalServerError().finish();
    };

    log_info!(logger, "Stop signal processed!");

    HttpResponse::Ok().body("Stop signal processed. Killing service..")
}
