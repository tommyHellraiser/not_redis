# not_redis

- Multiple queues supported, the maximum queues amount and items amount in each queue are configurable by external params
- User can retrieve the status of a single queue, or for all of them. Only the UUIDs and Identifiers for each item (job) will be returned
- User can request to kill a full queue and also remove a single element from a queue, identifiable by its UUID
- All sleep params, sizes and retries are configurable
- User can create queues on demand, and a worker will be spawned for each queue to allow parallel queuing and dequeuing and improve scalability. When a queue is deleted, the spawned worker will be killed as well

## Two main working modes:
- Automatic release

    A worker will automatically release each item in each queue after a configurable sleep_time. Items should be already being awaited for when they're released

    If an item is not being awaited, it'll be retried a configurable amount of times, with a configurable interval

    If all retries fail, then the item will be sent to a retry queue where the retry worker will attempt to release it another configurable amount of retries, using the same sleep time of the main queue. This retry queue will only activate once the main queue is depleted, and will yield to background when new items enter the main queue

    If all retries fail in the retry queue, then the item will be logged in a dedicated file using its UUID and Identifier, sent by the user, to allow recovering

- Release by Request

    There will be no worker releasing the items, instead, they will be released only when a dequeue request arrives. There is a configurable, and togglable timeout to avoid items from being queued indefinitely

    This mode is less resource intensive, but it can potentially lock the queues and slow down the workflow.

    It's also safer and more "forgiving" when dequeuing, because the chances of a job falling under the FAILED condition are lower, since they're released from the queue on demand

The user cannot hot switch between the two working modes, because thay would involve killing all workers and restarting the service completely, considering also that if there's items in a queue, these items could be lost. Configuration parameters cannot be hot reloaded for the same reasons