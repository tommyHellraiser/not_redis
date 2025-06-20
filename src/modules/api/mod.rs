use actix_web::{App, HttpServer, web};
use error_mapper::{TheResult, create_new_error};

use crate::modules::{api::middleware::Validation, config::Config};

mod life_services;
mod middleware;

pub async fn start_api() -> TheResult<()> {
    let app_config = Config::get()?;

    let server = HttpServer::new(|| {
        App::new()
            .service(
                web::scope("/api").service(web::scope("/life").configure(life_services::services)),
            )
            .wrap(Validation)
    })
    .bind(app_config.api.get_bind())
    .map_err(|error| create_new_error!(error))?
    .run();

    server.await.map_err(|error| create_new_error!(error))
}
