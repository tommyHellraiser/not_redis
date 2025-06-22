/*
I need to have a worker manager that will be spawned at the start of the service

That worker manager also has to have the ability to die with the stop signal

This worker manager needs to create workers every time a new queue is created. One worker per queue

Every time a queue is deleted, the worker that was handling that queue has to die, or be, let's say fired

The worker manager will probably have to loop around checking a main queue that will contain the general queues, and
when a new queue is created, spawn a worker

Maybe add another queue that indicates that a queue needs to be deleted, and submit the kill (or fire) request. We need
to wait until the queue is empty to delete it, otherwise we risk losing data

When a delete request for a queue is received, that queue has to enter a sort of closed-incoming state, to avoid more
jobs getting queued and never getting deleted

And we'll probably need a third queue, that can have a tuple, to indicate where a new job has to be queued by name, and
the manager will send the job to the correct worker. Which will probably be organized by a HashMap with a mpsc or
something like that. The worker will receive that job and queue it

TODO determine the logic for the dequeuing, that can be a bit more complicated and involve more channels, but in the inverse direction
from workers to dequeue requestors.
This logic will have two working functions, auto dequeuing and on demand dequeuing. The README details both of them
*/

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use error_mapper::{TheResult, create_new_error};
use the_logger::{TheLogger, log_critical, log_error, log_info};
use tokio::sync::{
    RwLock,
    broadcast::Receiver,
    mpsc::{Receiver as MpscReceiver, error::TryRecvError},
};
use uuid::Uuid;

use crate::modules::{
    config::Config,
    queuer::logic::{DequeueSyncInfo, NewJobInfo, Queue, QueueNameType, SharedQueues},
};

use tokio::sync::oneshot::Sender as OneShotSender;

//  Necessary statics that will externally receive the queuing directives. For workers, there will be
// internal channels
pub(super) static QUEUES_CREATION: OnceLock<RwLock<VecDeque<String>>> = OnceLock::new();
pub(super) static QUEUES_DELETION: OnceLock<RwLock<VecDeque<String>>> = OnceLock::new();
// pub(super) static JOBS_TO_ADD_TO_QUEUE: OnceLock<RwLock<VecDeque<Queue>>> = OnceLock::new();
pub(super) static FAILED_JOBS: OnceLock<RwLock<HashMap<QueueNameType, Vec<Queue>>>> =
    OnceLock::new();

static SHUTDOWN_INITIATED: AtomicBool = AtomicBool::new(true);

/// Struct used to transmit info between the worker manager and the workers. The one shot channel will be used
/// by the worker to notify the Api service when the job they requested was dequeued, and send the Queue content
/// thorugh it
struct InternalConfirmSyncInfo {
    /// UUID to identify the job to search
    uuid: Uuid,
    /// One Shot Sender channel to send the info, notifying the Api service that the job they requested is dequeued
    dequeue_sync_info_sender: OneShotSender<Queue>,
}

struct InternalQueueSyncInfo {
    item: Queue,
}

/// Enum used to send different tasks to the workers. Each task will be sent by a channel
enum TaskContent {
    NewJob(Queue),
    DequeueConfirmation(InternalConfirmSyncInfo),
    StopSignal,
}

pub async fn workers_manager_handler(
    stop_receiver: Receiver<()>,
    mut new_jobs_receiver: MpscReceiver<NewJobInfo>,
    mut dequeue_receiver: MpscReceiver<DequeueSyncInfo>,
) {
    /*
    I need mappings for:
    - Sending the new items to queue
    - Sending the confirmation requests for each workers
    - Sending the delete job? -> TODO pending
    - Sending the stop signal when a queue is
    */

    //  Used to send new items to each worker for them to queue up
    //  Uses the JOBS_TO_ADD_TO_QUEUE queue
    // let mut new_jobs_map = HashMap::new();

    // //  Used to send the confirmations OneShot channels to each worker
    // //  Uses the dequeue_receiver MPSC channel
    // let mut dequeue_confirmations_map = HashMap::new();

    // //  Used to command the worker to stop queuing new items in a queue and kill it, to then resign themselves
    // //  Uses the QUEUES_DELETION queue
    // let mut stop_worker_map = HashMap::new();

    /*
    When a new queue is created, and hence a worker, I need to create and link up for each new worker:
    - new jobs channel
    - a dequeue confirmations channel
    - a stop channel to delete the queue and resign the worker
    This can be done using an enum
    */

    //  Initialize the shutdown monitor status
    SHUTDOWN_INITIATED.store(false, Ordering::Relaxed);
    tokio::task::spawn(shutdown_monitor(stop_receiver));

    let logger = TheLogger::instance();
    log_info!(logger, "Initialized workers manager");

    let mut workers_map = HashMap::<QueueNameType, tokio::sync::mpsc::Sender<TaskContent>>::new();

    loop {
        if SHUTDOWN_INITIATED.load(Ordering::Relaxed) {
            //  TODO Handle the shutdown process for all workers here
            log_info!(logger, "Killing all worker processes...");
        }

        //  Check and handle the creation queue
        if let Some(creation_queue) = QUEUES_CREATION.get() {
            if let Some(new_queue) = creation_queue.write().await.pop_front() {
                log_info!(logger, "Creating new worker for queue: {}", new_queue);
                let (sender, receiver) = tokio::sync::mpsc::channel::<TaskContent>(20);

                //  Insert the sender half of the channel to keep track of it, and spawn the async worker
                workers_map.insert(new_queue.clone(), sender);
                tokio::task::spawn(job_worker(new_queue.clone(), receiver));
            }
        } else {
            log_critical!(
                logger,
                "Cannot access Creation Queue. Continuing processing"
            );
        }

        //  Check and handle the deletion queue
        if let Some(deletion_queue) = QUEUES_DELETION.get() {
            if let Some(delete_queue) = deletion_queue.write().await.pop_front() {
                log_info!(logger, "Deleting worker for queue: {}", delete_queue);

                if let Some(channel) = workers_map.get(&delete_queue) {
                    if let Err(error) = channel.send(TaskContent::StopSignal).await {
                        log_error!(
                            logger,
                            "Could not send a stop message to queue worker: {}. Cause: {}",
                            delete_queue,
                            error
                        );
                    }
                }
            }
        } else {
            log_critical!(
                logger,
                "Cannot access Deletion Queue. Continuing processing"
            );
        }

        //  Check and handle the new jobs channel
        //  Remain here while every message in the channel is routed to their corresponding worker
        while let Some(new_job) = new_jobs_receiver.recv().await {
            let uuid = new_job.job.uuid;
            if let Err(error) = add_job_to_shared_queue(new_job).await {
                log_error!(
                    logger,
                    "Error trying to queue new job with UUID: {}. Cause: {}",
                    uuid,
                    error
                );
            }
        }

        //  Check and handle the confirmation channel
        while let Some(dequeue_info) = dequeue_receiver.recv().await {
            let queue_name = dequeue_info.queue_name.clone();
            if let Err(error) = send_dequeue_request_to_worker(&workers_map, dequeue_info).await {
                log_error!(
                    logger,
                    "Could not send dequeue confirmation request to worker for queue: {}. cause: {}",
                    queue_name,
                    error
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn add_job_to_shared_queue(new_job: NewJobInfo) -> TheResult<()> {
    let shared = SharedQueues::get_shared()?;

    let Some(main_queue) = shared.get(&new_job.queue_name) else {
        return Err(create_new_error!(format!(
            "Could not find queue named: {}",
            new_job.queue_name
        )));
    };

    main_queue.write().await.push_back(new_job.job);

    Ok(())
}

async fn send_dequeue_request_to_worker(
    workers_map: &HashMap<QueueNameType, tokio::sync::mpsc::Sender<TaskContent>>,
    dequeue_info: DequeueSyncInfo,
) -> TheResult<()> {
    let Some(worker_sender) = workers_map.get(&dequeue_info.queue_name) else {
        return Err(create_new_error!(format!(
            "Could not find main queue named: {}",
            dequeue_info.queue_name
        )));
    };

    let internal_sync_info = InternalConfirmSyncInfo {
        uuid: dequeue_info.uuid,
        dequeue_sync_info_sender: dequeue_info.dequeue_confirmation_sender,
    };
    worker_sender
        .send(TaskContent::DequeueConfirmation(internal_sync_info))
        .await
        .map_err(|error| create_new_error!(error))?;

    Ok(())
}

/// Job that monitors when we receive a shutdown signal and raises the shutdown alarm for all workers to finish their tasks
async fn shutdown_monitor(mut stop_receiver: Receiver<()>) {
    //  Await the receiver for any Ok or Err. If all the senders were dropped w have nothing else to do, they're all gone
    // and we need to kill the process
    if stop_receiver.recv().await.is_err() {
        //  Do nothing, we just need to kill all workers
    }

    SHUTDOWN_INITIATED.store(true, Ordering::Relaxed);
}

async fn job_worker(
    queue_name: String,
    mut tasks_receiver: tokio::sync::mpsc::Receiver<TaskContent>,
) -> TheResult<()> {
    let app_config = Config::get()?;
    let sleep_time =
        std::time::Duration::from_millis(app_config.queue.workers_sleep_time_millis as u64);

    let logger = TheLogger::instance();

    let mut stop_requested = false;

    //  Queue to store the pending items to dequeue
    let mut awaiting_dequeue_confirmation_channels = HashMap::<Uuid, OneShotSender<Queue>>::new();

    loop {
        let mut channel_busy = true;
        while channel_busy {
            match tasks_receiver.try_recv() {
                Ok(task) => match task {
                    TaskContent::StopSignal => resign_worker_handler(&mut stop_requested),
                    TaskContent::NewJob(job_to_queue) => {
                        queue_new_job_handler(&queue_name, stop_requested, job_to_queue).await?
                    }
                    TaskContent::DequeueConfirmation(confirm_dequeue_sync_info) => {
                        confirm_dequeue_handler(
                            &mut awaiting_dequeue_confirmation_channels,
                            confirm_dequeue_sync_info,
                        );
                    }
                },
                Err(channel_error) => {
                    //  Receive and process all messages in the channel until there are no more messaes
                    //  This is thought to populate the confirmation channels with awaiting services before dequeuing the jobs
                    match channel_error {
                        TryRecvError::Empty => {
                            //  If the channel is empty, continue with dequeueing the jobs
                            channel_busy = false;
                        }
                        TryRecvError::Disconnected => {
                            let msg = "Channel disconnected, worker has to resign...";
                            return Err(create_new_error!(msg));
                        }
                    }
                }
            }
        }

        //  Try to dequeue jobs from the existing queue
        //  TODO conditional. If there's a request to pop a job from the queue, execute this....
        //  TODO, implement an automatic dequeuing? analyze if it's worth it..
        //  Try to dequeue only if there is at least one confirmation channel waiting
        'queue_handle: {
            match (
                awaiting_dequeue_confirmation_channels.is_empty(),
                stop_requested,
            ) {
                (true, true) => {
                    //  If stop signal was received and there are no more jobs in the queue, return
                    //  This worker will handle the deletion of the queue in the Shared Queues space, and after that, it'll resign
                    return remove_queue_from_shared_and_resign(&queue_name);
                }
                (true, false) => break 'queue_handle, // If stop signal was not received and the queue is empty, continue looping
                (false, _) => {} // If the queue is not empty continue processing to dequeue the jobs
            }

            let shared_queues = SharedQueues::get_shared()?;
            let Some(main_queue) = shared_queues.get(&queue_name) else {
                return Err(create_new_error!(format!(
                    "Could not find queue named: {}",
                    queue_name
                )));
            };

            //  If there's a job that was dequeued, then check if there was a requestor for said job, otherwise,
            // send it into the retry queue
            if let Some(mut dequeued_job) = main_queue.write().await.pop_front() {
                if let Some(confirmation_channel) =
                    awaiting_dequeue_confirmation_channels.remove(&dequeued_job.uuid)
                {
                    if let Err(dequeued_job) = confirmation_channel.send(dequeued_job) {
                        //  If the confirmation channel is closed, there's no recovery. Log and insert as failed job
                        insert_into_failed_jobs_or_log(logger, &queue_name, dequeued_job).await;
                    }
                } else {
                    //  Since we just popped this job from the main queue, the retry count here is zero
                    //  Check the retries amount
                    if dequeued_job.retry_count
                        >= app_config
                            .queue
                            .main_queue_retry
                            .on_item_release_not_requested
                    {
                        insert_into_failed_jobs_or_log(logger, &queue_name, dequeued_job).await;
                        break 'queue_handle;
                    }
                    dequeued_job.retry_count += 1;

                    //  Send the job to the back of the queue
                    main_queue.write().await.push_back(dequeued_job);
                }
            }
        }

        //  TODO otherwise, implement the auto dequeue feature, configurable from config.json. To determine if it's worth it...

        tokio::time::sleep(sleep_time).await;
    }
}

fn resign_worker_handler(stop_requested: &mut bool) {
    if !*stop_requested {
        *stop_requested = true;
    }
}

async fn queue_new_job_handler(
    queue_name: &str,
    stop_requested: bool,
    job_to_queue: Queue,
) -> TheResult<()> {
    //  For adding more jobs to the queue, the stop_requested has more priority.
    //  If a stop was requested, do not add more jobs to the queue, only dequeue from now on
    if stop_requested {
        return Ok(());
    }

    //  Try to receive a new job to queue. If there's no items waiting to be queued, continue with other tasks
    //  ALWAYS use and release the refs to avoid deadlocks. It's better to always try to access the ref each time
    // we need to use it, rather than risking a deadlock and freezing the worker, and if the worker is freezed,
    // it won't exit when it's prompted to exit by the manager, hence it'll have to be forcibly killed
    let shared_queue = SharedQueues::get_shared()?;
    //  If there was a job in the channel, then add it to the queue
    let Some(shared_queue) = shared_queue.get(queue_name) else {
        //  If the queue cannot be found, then either the shared queues are broken or the queue was deleted.
        // Return from this worker, it has nothing more to do
        let msg = format!(
            "Could not find the queue named: {}. Exiting the worker process",
            queue_name
        );
        return Err(create_new_error!(msg));
    };
    shared_queue.write().await.push_back(job_to_queue);

    Ok(())
}

fn confirm_dequeue_handler(
    awaiting_dequeue_confirmation_channels: &mut HashMap<Uuid, OneShotSender<Queue>>,
    internal_confirm_sync_info: InternalConfirmSyncInfo,
) {
    //  Insert the job in the HashMap to later send the confirmation through the OneShot Channel
    awaiting_dequeue_confirmation_channels.insert(
        internal_confirm_sync_info.uuid,
        internal_confirm_sync_info.dequeue_sync_info_sender,
    );
}

/// Helper function to try inserting failed dequeued jobs into the FAILED queue. If the job fails to be inserted,
/// log error with the UUID and discard it
async fn insert_into_failed_jobs_or_log(logger: &TheLogger, queue_name: &str, dequeued_job: Queue) {
    let uuid = dequeued_job.uuid;
    if let Some(failed_jobs_queue) = FAILED_JOBS.get() {
        failed_jobs_queue
            .write()
            .await
            .entry(queue_name.to_string())
            .and_modify(|content| content.push(dequeued_job.clone()))
            .or_insert_with(|| vec![dequeued_job]);
    } else {
        //  If the lock on the failed jobs queue cannot be acquired, drop the job and log error
        log_error!(
            logger,
            "Could not handle a failed dequeued job with UUID: {}",
            uuid
        );
    }
}

fn remove_queue_from_shared_and_resign(queue_name: &str) -> TheResult<()> {
    let shared = SharedQueues::get_shared()?;
    let _ = shared.remove(queue_name);
    //  We can ignore the result because at this point it doesn't matter if the queue is present or not

    Ok(())
}
