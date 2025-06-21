use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use actix_web::web;
use chrono::NaiveDateTime;
use dashmap::{DashMap, Entry};
use error_mapper::{TheResult, create_new_error};
use serde::Serialize;
use tokio::sync::RwLock;
use uuid::Uuid;
use crate::modules::config::Config;

mod workers;

static SHARED_QUEUES: OnceLock<Arc<SharedQueuesType>> = OnceLock::new();
static NEW_ENTRIES_ENABLE: AtomicBool = AtomicBool::new(true);

type SharedQueuesType = DashMap<String, RwLock<SingleQueueType>>;
type SingleQueueType = VecDeque<Queue>;

pub struct SharedQueues;

pub enum SharedQueuesResult<S: Serialize> {
    Ok,
    Content(S),
    RequestError(String),
}

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
    /// Registered in UTC format for consistency
    job_creation: NaiveDateTime,
}

impl SharedQueues {
    pub fn init() {
        let _ = SHARED_QUEUES.get_or_init(|| Arc::new(DashMap::new()));
        //  Enable reception of new entries by default
        NEW_ENTRIES_ENABLE.store(true, Ordering::Relaxed);
    }

    pub fn create_queue(queue_name: String) -> TheResult<SharedQueuesResult<usize>> {
        let app_config = Config::get()?;

        let Some(shared) = SHARED_QUEUES.get() else {
            return Err(create_new_error!("Could not get queues map"));
        };

        //  Validate there's room to create a new queue first. Compare the current amount with the max amount
        // If they're equal, then we're at the limit and no other queues can be created
        if app_config.queue.max_queue_amounts as usize == shared.len() {
            return Ok(SharedQueuesResult::RequestError(
                "Cannot create any more queues. Limit reached!".to_string(),
            ));
        }

        //  Create the new queue only if queue did not exist If it did, return error
        match shared.entry(queue_name) {
            Entry::Occupied(_) => {
                return Ok(SharedQueuesResult::RequestError(
                    "Queue name already in use".to_string(),
                ));
            }
            Entry::Vacant(entry) => entry.insert(RwLock::new(VecDeque::new())),
        };

        //  Return the amount of queue places available
        let available = app_config.queue.max_queue_amounts as usize - shared.len();
        Ok(SharedQueuesResult::Content(available))
    }

    pub fn delete_queue(queue_name: String) -> TheResult<SharedQueuesResult<String>> {
        let Some(shared) = SHARED_QUEUES.get() else {
            return Err(create_new_error!("Could not get queues map"));
        };

        if shared.remove(&queue_name).is_none() {
            return Ok(SharedQueuesResult::Content(
                "No queue found with the requested name".to_string(),
            ));
        }

        Ok(SharedQueuesResult::Ok)
    }

    pub async fn send_to_queue_back(
        queue_name: String,
        identifier: String,
        job: Option<web::Bytes>,
    ) -> TheResult<SharedQueuesResult<String>> {
        //  Get the queue first
        let Some(shared_queues) = SHARED_QUEUES.get() else {
            return Err(create_new_error!("Could not get queues map"));
        };
        let Some(queue) = shared_queues.get(&queue_name) else {
            return Ok(SharedQueuesResult::RequestError(
                "Queue name not found".to_string(),
            ));
        };

        queue.write().await.push_back(Queue::new(identifier, job));

        Ok(SharedQueuesResult::Ok)
    }

    pub fn pop_from_queue_front() {}

    /// Returns the UUIDs of every item in a single queue
    pub async fn get_queue_status(
        queue_name: String,
    ) -> TheResult<SharedQueuesResult<Vec<String>>> {
        let Some(shared_queues) = SHARED_QUEUES.get() else {
            return Err(create_new_error!("Could not get queues map"));
        };
        let Some(queue) = shared_queues.get(&queue_name) else {
            return Ok(SharedQueuesResult::RequestError(
                "Queue name not found".to_string(),
            ));
        };

        let uuids = queue
            .read()
            .await
            .iter()
            .map(|item| item.uuid.to_string())
            .collect::<Vec<String>>();

        Ok(SharedQueuesResult::Content(uuids))
    }

    /// Returns a list of the curently available queues, with the UUIDs of each element present for each queue
    pub async fn list_queues() -> TheResult<SharedQueuesResult<HashMap<String, Vec<String>>>> {
        let Some(shared_queues) = SHARED_QUEUES.get() else {
            return Err(create_new_error!("Could not get queues map"));
        };

        let mut queues = HashMap::with_capacity(shared_queues.len());

        for content in shared_queues.iter() {
            let (queue_name, queue_content) = content.pair();
            let uuids = queue_content
                .read()
                .await
                .iter()
                .map(|content| content.uuid.to_string())
                .collect::<Vec<String>>();
            queues.insert(queue_name.clone(), uuids);
        }

        Ok(SharedQueuesResult::Content(queues))
    }

    pub fn new_entries_enable(enable: bool) {
        NEW_ENTRIES_ENABLE.store(enable, Ordering::Relaxed);
    }

    pub fn validate_new_entries_and_handle() -> Option<String> {
        if !NEW_ENTRIES_ENABLE.load(Ordering::Relaxed) {
            return Some("New entries are disabled".to_string());
        }

        None
    }
}

impl Queue {
    fn new(identifier: String, job: Option<web::Bytes>) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4(),
            identifier,
            job,
            job_creation: chrono::Utc::now().naive_utc(),
        }
    }
}
