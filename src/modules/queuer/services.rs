use actix_web::{HttpResponse, get, post};

#[get("/status")]
pub(super) async fn get_status_for_all() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[get("/status/{queue_name}")]
pub(super) async fn get_status_for_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[get("/list")]
pub(super) async fn list_queues() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/create/{queue_name}")]
pub(super) async fn create_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/enqueue")]
pub(super) async fn add_to_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/dequeue/{uuid}")]
pub(super) async fn remove_from_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/kill/{queue_name}")]
pub(super) async fn kill_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/delete/{queue_name}/{uuid}")]
pub(super) async fn delete_from_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}
