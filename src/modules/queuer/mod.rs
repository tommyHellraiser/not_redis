use actix_web::web::ServiceConfig;

pub(super) mod logic;
mod services;

pub(super) fn services(cfg: &mut ServiceConfig) {
    cfg.service(services::get_status_for_all)
        .service(services::get_status_for_queue)
        .service(services::create_queue)
        .service(services::add_to_queue)
        .service(services::remove_from_queue)
        .service(services::delete_queue)
        .service(services::delete_from_queue)
        .service(services::new_entries_enable);
}
