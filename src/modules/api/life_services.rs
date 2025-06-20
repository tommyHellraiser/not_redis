use actix_web::{HttpResponse, get, web::ServiceConfig};

pub(super) fn services(cfg: &mut ServiceConfig) {
    cfg.service(alive);
}

#[get("alive")]
async fn alive() -> HttpResponse {
    HttpResponse::Ok().body("I'm alive!!!")
}
