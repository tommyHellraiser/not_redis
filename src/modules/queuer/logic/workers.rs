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
};

use error_mapper::{TheResult, create_new_error};
use the_logger::{TheLogger, log_error, log_info};
use tokio::sync::{
    RwLock,
    broadcast::Receiver,
    mpsc::{Receiver as MpscReceiver, error::TryRecvError},
};
use uuid::Uuid;

use crate::modules::{
    config::Config,
    queuer::logic::{DequeueSyncInfo, Queue, QueueNameType, SharedQueues},
};

use tokio::sync::oneshot::Sender as OneShotSender;

//  Necessary statics that will externally receive the queuing directives. For workers, there will be
// internal channels
pub(super) static QUEUES_CREATION: OnceLock<VecDeque<String>> = OnceLock::new();
pub(super) static QUEUES_DELETION: OnceLock<VecDeque<String>> = OnceLock::new();
pub(super) static JOBS_QUEUING: OnceLock<VecDeque<Queue>> = OnceLock::new();
pub(super) static FAILED_JOBS: OnceLock<RwLock<HashMap<QueueNameType, Vec<Queue>>>> =
    OnceLock::new();

static SHUTDOWN_INITIATED: AtomicBool = AtomicBool::new(true);

/// Struct used to transmit info between the worker manager and the workers. The one shot channel will be used
/// by the worker to notify the Api service when the job they requested was dequeued, and send the Queue content
/// thorugh it
struct InternalSyncInfo {
    /// UUID to identify the job to search
    uuid: Uuid,
    /// One Shot Sender channel to send the info, notifying the Api service that the job they requested is dequeued
    dequeue_sync_info_sender: OneShotSender<Queue>,
}

pub async fn workers_manager_handler(
    stop_receiver: Receiver<()>,
    dequeue_receiver: MpscReceiver<DequeueSyncInfo>,
) {
    //  Initialize the shutdown monitor status
    SHUTDOWN_INITIATED.store(false, Ordering::Relaxed);
    tokio::task::spawn(shutdown_monitor(stop_receiver));

    let logger = TheLogger::instance();
    log_info!(logger, "Initialized workers manager");

    loop {
        if SHUTDOWN_INITIATED.load(Ordering::Relaxed) {
            //  TODO Handle the shutdown process for all workers here
            log_info!(logger, "Killing all worker processes...");
        }
    }
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
    queue_name: &str,
    mut job_receiver: tokio::sync::mpsc::Receiver<Queue>,
    mut stop_receiver: tokio::sync::mpsc::Receiver<()>,
    mut dequeue_receiver: MpscReceiver<(Uuid, OneShotSender<Queue>)>,
) -> TheResult<()> {
    let app_config = Config::get()?;
    let sleep_time =
        std::time::Duration::from_millis(app_config.queue.workers_sleep_time_millis as u64);

    let logger = TheLogger::instance();

    let mut stop_requested = false;

    //  Queue to store the pending items to dequeue
    let mut awaiting_dequeue_confirmation_channels = HashMap::<Uuid, OneShotSender<Queue>>::new();

    loop {
        match stop_receiver.try_recv() {
            Ok(_) => {
                //  Stop this channel from receiving further stop signals. Shutdown process for this worker starts now
                stop_receiver.close();
                stop_requested = true;
            }
            Err(channel_error) => {
                channel_empty_or_return(logger, channel_error).await?;
            }
        }

        //  Try to receive a new job to queue. If there's no items waiting to be queued, continue with other tasks
        //  ALWAYS use and release the refs to avoid deadlocks. It's better to always try to access the ref each time
        // we need to use it, rather than risking a deadlock and freezing the worker, and if the worker is freezed,
        // it won't exit when it's prompted to exit by the manager, hence it'll have to be forcibly killed
        match (job_receiver.try_recv(), stop_requested) {
            (_, true) => {
                //  For adding more jobs to the queue, the stop_requested has more priority.
                //  If a stop was requested, do not add more jobs to the queue, only dequeue from now on
            }
            (Ok(job_to_queue), _) => {
                let shared_queue = SharedQueues::get_shared()?;
                //  If there was a job in the channel, then add it to the queue
                let Some(shared_queue) = shared_queue.get(queue_name) else {
                    //  If the queue cannot be found, then either the shared queues are broken or the queue was deleted.
                    // Return from this worker, it has nothing more to do
                    let msg = format!(
                        "Could not find the queue named: {}. Exiting the worker process",
                        queue_name
                    );
                    log_error!(logger, "{}", msg);
                    return Err(create_new_error!(msg));
                };
                shared_queue.write().await.push_back(job_to_queue);
            }
            (Err(channel_error), _) => {
                channel_empty_or_return(logger, channel_error).await?;
            }
        }

        //  Check if there's any job requested to be dequeued in the internal channel
        match dequeue_receiver.try_recv() {
            Ok((uuid, dequeue_confirmation_channel)) => {
                //  If there is, then insert the job in the hashmap to later confirm when it's popped from the queue
                awaiting_dequeue_confirmation_channels.insert(uuid, dequeue_confirmation_channel);
            }
            Err(channel_error) => {
                channel_empty_or_return(logger, channel_error).await?;
            }
        }

        //  Try to dequeue jobs from the existing queue
        //  TODO conditional. If there's a request to pop a job from the queue, execute this....
        //  TODO, implement an automatic dequeuing? analyze if it's worth it..
        //  Try to dequeue only if there is at least one confirmation channel waiting
        'queue_handle: {
            if awaiting_dequeue_confirmation_channels.is_empty() {
                break 'queue_handle;
            }

            let shared_queues = SharedQueues::get_shared()?;
            let Some(main_queue) = shared_queues.get(queue_name) else {
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
                        insert_into_failed_jobs_or_log(logger, queue_name, dequeued_job).await;
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
                        insert_into_failed_jobs_or_log(logger, queue_name, dequeued_job).await;
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

/// Helper function to reduce duplicated code, it simply checks if the channel is empty. Empty is good. If
/// it's disconnected, then we have to call off the worker.
/// The channel being empty only means there were no messages sent, and we have nothing to keep awaiting for
async fn channel_empty_or_return(logger: &TheLogger, channel_error: TryRecvError) -> TheResult<()> {
    match channel_error {
        TryRecvError::Empty => {
            //  If the channel is empty, continue with another task
            Ok(())
        }
        TryRecvError::Disconnected => {
            let msg = "Channel disconnected, worker has to resign...";
            log_error!(logger, "{}", msg);
            Err(create_new_error!(msg))
        }
    }
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
