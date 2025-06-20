use actix_web::{App, HttpServer, web};
use error_mapper::{TheResult, create_new_error};

use crate::modules::api::middleware::Validation;

mod life_services;
mod middleware;

pub async fn start_api() -> TheResult<()> {
    let server = HttpServer::new(|| {
        App::new().service(
            web::scope("/api").service(web::scope("/life").configure(life_services::services)),
        )
        .wrap(Validation)
    })
    .bind(("127.0.0.1", 8090))
    .map_err(|error| create_new_error!(error))?
    .run();

    server.await.map_err(|error| create_new_error!(error))
}
