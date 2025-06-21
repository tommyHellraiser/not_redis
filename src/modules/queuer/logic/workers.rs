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