use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::modules::{
    config::Config,
    queuer::logic::workers::{QUEUES_CREATION, QUEUES_DELETION},
};
use chrono::NaiveDateTime;
use dashmap::{DashMap, Entry};
use error_mapper::{TheResult, create_new_error};
use serde::Serialize;
use tokio::sync::RwLock;
use tokio::sync::mpsc::Sender as MpscSender;
use tokio::sync::oneshot::Sender as OneShotsender;
use uuid::Uuid;

pub mod workers;

pub type QueueNameType = String;

static SHARED_QUEUES: OnceLock<Arc<SharedQueuesType>> = OnceLock::new();
static NEW_ENTRIES_ENABLE: AtomicBool = AtomicBool::new(true);

type SharedQueuesType = DashMap<QueueNameType, RwLock<SingleQueueType>>;
type SingleQueueType = VecDeque<Queue>;

pub struct SharedQueues;

pub struct DequeueSyncInfo {
    pub queue_name: QueueNameType,
    pub uuid: Uuid,
    pub dequeue_confirmation_sender: OneShotsender<Queue>,
}

pub struct NewJobInfo {
    queue_name: QueueNameType,
    job: Queue,
}

pub enum SharedQueuesResult<S: Serialize> {
    Ok,
    Content(S),
    RequestError(String),
}

/// Any type that can be serialized will be accepted, giving the user the flexibility
/// to either queue and wait actively until the job is dequeued, or simply sending the job
/// to this queue system, and handling the job execution later with a cron job, in a separate
/// thread/process
#[derive(Clone, Debug)]
pub struct Queue {
    uuid: Uuid,
    /// This identifier must be sent by the user requesting to queue, to uniquely identify each job on their side
    /// When an item pop fails and cannot be freed, this identifier will be logged together with the contents of the job
    /// to recover the failed job
    identifier: String,
    /// Optional to allow the user to either use this service as a lock handler, or use the full
    /// capabilities of queuing
    job: Option<serde_json::Value>,
    /// Registered in UTC format for consistency
    job_creation: NaiveDateTime,
    retry_count: u8,
}

#[derive(Debug, Serialize, Clone)]
pub struct QueueExternal {
    uuid: String,
    identifier: String,
    job: Option<serde_json::Value>,
    job_creation: NaiveDateTime,
}

impl From<Queue> for QueueExternal {
    fn from(value: Queue) -> Self {
        Self {
            uuid: value.uuid.to_string(),
            identifier: value.identifier.clone(),
            job: value.job.clone(),
            job_creation: value.job_creation,
        }
    }
}

impl SharedQueues {
    pub fn init() {
        let _ = SHARED_QUEUES.get_or_init(|| Arc::new(DashMap::new()));
        //  Enable reception of new entries by default
        NEW_ENTRIES_ENABLE.store(true, Ordering::Relaxed);
        let _ = QUEUES_CREATION.get_or_init(|| RwLock::new(VecDeque::new()));
        let _ = QUEUES_DELETION.get_or_init(|| RwLock::new(VecDeque::new()));
    }

    pub fn create_queue(queue_name: QueueNameType) -> TheResult<SharedQueuesResult<usize>> {
        let app_config = Config::get()?;

        let shared = Self::get_shared()?;

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

    pub fn delete_queue(queue_name: QueueNameType) -> TheResult<SharedQueuesResult<String>> {
        let shared = Self::get_shared()?;

        if shared.remove(&queue_name).is_none() {
            return Ok(SharedQueuesResult::Content(
                "No queue found with the requested name".to_string(),
            ));
        }

        Ok(SharedQueuesResult::Ok)
    }

    pub async fn add_to_queue(// queue_name: QueueNameType,
        // identifier: String,
        // job: Option<web::Bytes>,
    ) {
        //  Get the queue first
        // let shared = Self::get_shared()?;

        // let Some(queue) = shared.get(&queue_name) else {
        //     return Ok(SharedQueuesResult::RequestError(
        //         "Queue name not found".to_string(),
        //     ));
        // };

        // queue.write().await.push_back(Queue::new(identifier, job));

        // Ok(SharedQueuesResult::Ok)
    }

    /// Returns the UUIDs of every item in a single queue
    pub async fn get_queue_status(
        queue_name: QueueNameType,
    ) -> TheResult<SharedQueuesResult<Vec<String>>> {
        let shared = Self::get_shared()?;
        let Some(queue) = shared.get(&queue_name) else {
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
        let shared = Self::get_shared()?;

        let mut queues = HashMap::with_capacity(shared.len());

        for content in shared.iter() {
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

    fn get_shared() -> TheResult<Arc<DashMap<QueueNameType, RwLock<VecDeque<Queue>>>>> {
        SHARED_QUEUES
            .get()
            .ok_or(create_new_error!("Could not get shared queues"))
            .map(Arc::clone)
    }

    pub async fn retrieve_from_queue_and_await(
        queue_name: String,
        uuid: Uuid,
        dequeue_sender: MpscSender<DequeueSyncInfo>,
    ) -> TheResult<Option<Queue>> {
        //  First, create a channel that the worker will use to notify this service function when the job is dequeued
        let (dequeue_confirmation_sender, dequeue_confirmation_receiver) =
            tokio::sync::oneshot::channel::<Queue>();

        //  Build the sync info for the worker to correctly process the dequeuing
        let sync_info = DequeueSyncInfo {
            queue_name,
            uuid,
            dequeue_confirmation_sender,
        };

        //  Send the message thorugh the channel to notify the worker manager that we're waiting for our job to be dequeued
        dequeue_sender
            .send(sync_info)
            .await
            .map_err(|error| create_new_error!(error))?;

        let wait_timeout = std::time::Duration::from_secs(
            Config::get()?.queue.dequeue_timeout_wait_seconds as u64,
        );

        //  Await both the timeout and the reception of the dequeue confirmation. Whichever comes first, will unlock the processing
        tokio::select! {
            _ = tokio::time::sleep(wait_timeout) => {
                //  If we timed out, we need to return accordingly
                Ok(None)
            }
            result = dequeue_confirmation_receiver => {
                //  Otherwise, the job was dequeued correctly, we return it to the caller
                let queued_job = result.map_err(|error| create_new_error!(error))?;
                Ok(Some(queued_job))
            }
        }
    }
}

impl Queue {
    fn new(identifier: String, job: Option<serde_json::Value>) -> Self {
        Self {
            uuid: uuid::Uuid::new_v4(),
            identifier,
            job,
            job_creation: chrono::Utc::now().naive_utc(),
            retry_count: 0,
        }
    }
}
