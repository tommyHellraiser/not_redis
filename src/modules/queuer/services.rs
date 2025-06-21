use actix_web::{HttpResponse, get, post, put, web};
use the_logger::{TheLogger, log_error, log_warning};

use crate::modules::queuer::logic::{SharedQueues, SharedQueuesResult};

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
pub(super) async fn create_queue(path: web::Path<String>) -> HttpResponse {
    let queue_name = path.into_inner();

    //  Validate new entries creation is enabled
    if let Some(result) = SharedQueues::validate_new_entries_and_handle() {
        return HttpResponse::BadRequest().body(result);
    }

    let logger = TheLogger::instance();

    let result = match SharedQueues::create_queue(queue_name) {
        Ok(result) => result,
        Err(error) => {
            log_error!(logger, "Failed to get create queue: {}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    let available = match result {
        SharedQueuesResult::Content(available) => available,
        SharedQueuesResult::RequestError(msg) => return HttpResponse::BadRequest().body(msg),
        SharedQueuesResult::Ok => {
            log_warning!(logger, "Unexpected result!");
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().body(format!("{} queue places remain available", available))
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

#[put("/new_entries_enable/{enable}")]
pub(super) async fn new_entries_enable(path: web::Path<bool>) -> HttpResponse {
    let enable = path.into_inner();

    SharedQueues::new_entries_enable(enable);

    HttpResponse::Ok().body(format!("New entries enabled: {}", enable))
}
