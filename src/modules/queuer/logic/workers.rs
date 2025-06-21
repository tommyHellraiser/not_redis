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
    collections::VecDeque,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use the_logger::{TheLogger, log_error, log_info};
use tokio::sync::{broadcast::Receiver, mpsc::error::TryRecvError};

use crate::modules::{config::Config, queuer::logic::Queue};

//  Necessary statics that will externally receive the queuing directives. For workers, there will be
// internal channels
pub(super) static QUEUES_CREATION: OnceLock<VecDeque<String>> = OnceLock::new();
pub(super) static QUEUES_DELETION: OnceLock<VecDeque<String>> = OnceLock::new();
pub(super) static JOBS_QUEUING: OnceLock<VecDeque<Queue>> = OnceLock::new();

static SHUTDOWN_INITIATED: AtomicBool = AtomicBool::new(true);

pub async fn workers_manager_handler(stop_receiver: Receiver<()>) {
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
    if let Err(_) = stop_receiver.recv().await {
        //  Do nothing, we just need to kill all workers
    }

    SHUTDOWN_INITIATED.store(true, Ordering::Relaxed);
}

async fn job_worker(
    queue_name: &str,
    mut job_receiver: tokio::sync::mpsc::Receiver<Queue>,
    mut stop_receiver: tokio::sync::mpsc::Receiver<()>,
) {
    let sleep_time_millis = Config::get().unwrap().queue.workers_sleep_time_millis;
    let sleep_time = std::time::Duration::from_millis(sleep_time_millis as u64);

    let logger = TheLogger::instance();

    let mut stop_requested = false;

    loop {
        match stop_receiver.try_recv() {
            Ok(_) => {
                //  Stop this channel from receiving further stop signals. Shutdown process for this worker starts now
                stop_receiver.close();
                stop_requested = true;
            }
            Err(ch_error) => {
                match ch_error {
                    TryRecvError::Empty => {
                        //  If the channel is empty, no stop signal was received. Continue
                    }
                    TryRecvError::Disconnected => {
                        //  If the stop channel was disconnected, the worker can't stop later on. Abort the worker now
                        log_error!(logger, "Channel disconnected, worker has to resign...");
                        return;
                    }
                }
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
                //  If there was a job in the channel, then add it to the queue
                let Some(shared_queue) = super::SHARED_QUEUES
                    .get()
                    .and_then(|shared_queue| shared_queue.get(queue_name))
                else {
                    //  If the queue cannot be found, then either the shared queues are broken or the queue was deleted.
                    // Return from this worker, it has nothing more to do
                    log_error!(
                        logger,
                        "Could not find the queue named: {}. Exiting the worker process",
                        queue_name
                    );
                    return;
                };
                shared_queue.write().await.push_back(job_to_queue);
            }
            (Err(channel_error), _) => {
                match channel_error {
                    TryRecvError::Empty => {
                        //  If the channel is empty, continue with another task
                    }
                    TryRecvError::Disconnected => {
                        log_error!(logger, "Channel disconnected, worker has to resign...");
                        return;
                    }
                }
            }
        }

        //  Try to dequeue jobs from the existing queue
        //  TODO

        tokio::time::sleep(sleep_time).await;
    }
}
