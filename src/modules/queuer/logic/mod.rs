use std::{
    collections::VecDeque,
    sync::{Arc, OnceLock},
};

use actix_web::web;
use chrono::NaiveDateTime;
use dashmap::DashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

static SHARED_QUEUES: OnceLock<Arc<SharedQueuesType>> = OnceLock::new();

type SharedQueuesType = DashMap<String, RwLock<SingleQueueType>>;
type SingleQueueType = VecDeque<Queue>;

struct SharedQueues;

/// Any type that can be serialized will be accepted, giving the user the flexibility
/// to either queue and wait actively until the job is dequeued, or simply sending the job
/// to this queue system, and handling the job execution later with a cron job, in a separate
/// thread/process
struct Queue {
    uuid: Uuid,
    /// This identifier must be sent by the user requesting to queue, to uniquely identify each job on their side
    /// When an item pop fails and cannot be freed, this identifier will be logged together with the contents of the job
    /// to recover the failed job
    identifier: String,
    /// Optional to allow the user to either use this service as a lock handler, or use the full
    /// capabilities of queuing
    job: Option<web::Bytes>,
    job_creation: NaiveDateTime,
}

impl SharedQueues {
    fn init() {}

    fn send_to_back() {}

    fn pop_from_front() {}

    fn get_queue_status() {}

    fn list_queues() {}
}

impl Queue {
    fn new_uuid() {
        let uuid = uuid::Uuid::new_v4();
    }
}
