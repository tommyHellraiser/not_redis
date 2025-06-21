use actix_web::{HttpResponse, delete, get, post, put, web};
use the_logger::{TheLogger, log_error, log_warning};

use crate::modules::queuer::logic::{SharedQueues, SharedQueuesResult};

#[get("/status")]
pub(super) async fn get_status_for_all() -> HttpResponse {
    let logger = TheLogger::instance();

    let queues = match SharedQueues::list_queues().await {
        Ok(SharedQueuesResult::Content(queues)) => queues,
        Err(error) => {
            log_error!(logger, "Error getting queues status: {}", error);
            return HttpResponse::InternalServerError().finish();
        }
        _ => {
            log_warning!(logger, "Unexpected result!");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let response = match serde_json::to_string_pretty(&queues) {
        Ok(response) => response,
        Err(error) => {
            log_error!(
                logger,
                "Could not build response from queues status: {}",
                error
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().body(response)
}

#[get("/status/{queue_name}")]
pub(super) async fn get_status_for_queue(path: web::Path<String>) -> HttpResponse {
    let queue_name = path.into_inner();
    let logger = TheLogger::instance();

    let result = match SharedQueues::get_queue_status(queue_name).await {
        Ok(result) => result,
        Err(error) => {
            log_error!(
                logger,
                "Could not get status for requested queue: {}",
                error
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    let content = match result {
        SharedQueuesResult::Content(content) => content,
        SharedQueuesResult::RequestError(request_error) => {
            return HttpResponse::BadRequest().body(request_error);
        }
        _ => {
            log_warning!(logger, "Unexpected result!");
            return HttpResponse::InternalServerError().finish();
        }
    };

    let response = match serde_json::to_string_pretty(&content) {
        Ok(response) => response,
        Err(error) => {
            log_error!(
                logger,
                "Could not build response from queue status: {}",
                error
            );
            return HttpResponse::InternalServerError().finish();
        }
    };

    HttpResponse::Ok().body(response)
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

#[post("/queue_job")]
pub(super) async fn add_to_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[post("/dequeue_job/{uuid}")]
pub(super) async fn remove_from_queue() -> HttpResponse {
    HttpResponse::Ok().finish()
}

#[delete("/delete/{queue_name}")]
pub(super) async fn delete_queue(path: web::Path<String>) -> HttpResponse {
    let queue_name = path.into_inner();
    let logger = TheLogger::instance();

    let result = match SharedQueues::delete_queue(queue_name) {
        Ok(result) => result,
        Err(error) => {
            log_error!(logger, "Could not delete queue from queues map: {}", error);
            return HttpResponse::InternalServerError().finish();
        }
    };

    match result {
        SharedQueuesResult::Ok => {
            //  All good, deleted successfully
        }
        SharedQueuesResult::RequestError(bad_request) => {
            return HttpResponse::BadRequest().body(bad_request);
        }
        SharedQueuesResult::Content(_) => {
            log_warning!(logger, "Unexpected result!");
            return HttpResponse::InternalServerError().finish();
        }
    }

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
