use actix_web::{HttpResponse, get, put, web::ServiceConfig};

pub(super) fn services(cfg: &mut ServiceConfig) {
    cfg.service(alive);
}

#[get("alive")]
async fn alive() -> HttpResponse {
    HttpResponse::Ok().body("I'm alive!!!")
}

#[put("/stop")]
async fn stop() -> HttpResponse {
    HttpResponse::Ok().finish()
}
