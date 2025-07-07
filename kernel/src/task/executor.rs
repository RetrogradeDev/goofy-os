use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

use x86_64::instructions::interrupts::{self, enable_and_hlt};

use crate::serial_println;

const MAX_TASKS: usize = 1000;

pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(MAX_TASKS)),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("task with same ID already in tasks");
        }
        self.task_queue.push(task_id).expect("queue full");
    }

    fn run_ready_tasks(&mut self) {
        // destructure `self` to avoid borrow checker errors
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        let mut tasks_polled = 0;
        while let Some(task_id) = task_queue.pop() {
            tasks_polled += 1;
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                None => continue, // task no longer exists
            };
            let waker = waker_cache
                .entry(task_id)
                .or_insert_with(|| TaskWaker::new(task_id, task_queue.clone()));
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // task done -> remove it and its cached waker
                    serial_println!("Task {:?} completed and removed", task_id);
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }

        if tasks_polled > 0 {
            serial_println!(
                "Executor: polled {} tasks, {} active tasks remaining",
                tasks_polled,
                tasks.len()
            );
        }
    }
    pub fn run(&mut self) -> ! {
        serial_println!("Executor: Starting main loop");

        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    fn sleep_if_idle(&self) {
        // Always enable interrupts first
        interrupts::enable();

        // Check if we have any tasks at all
        let has_tasks = !self.tasks.is_empty();
        let has_queued = !self.task_queue.is_empty();

        if has_tasks || has_queued {
            // If we have any tasks (even if all are pending), don't halt
            // Instead, do a very short spin to allow interrupts to be processed
            // This ensures keyboard and timer interrupts can wake tasks
            for _ in 0..100 {
                core::hint::spin_loop();
            }
        } else {
            // Only halt if there are absolutely no tasks
            // But even then, interrupts will wake us up
            interrupts::disable();
            if self.task_queue.is_empty() {
                enable_and_hlt();
            } else {
                interrupts::enable();
            }
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            task_queue,
        }))
    }

    fn wake_task(&self) {
        // Only wake if the task isn't already in the queue to avoid filling it up
        if self.task_queue.push(self.task_id).is_err() {
            // Queue is full, just ignore - the task will be polled eventually
        }
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

/// A simple tick function for the timer interrupt
/// This just ensures interrupts are enabled and executor continues to run
pub fn tick_executor() {
    // Make sure interrupts are enabled
    x86_64::instructions::interrupts::enable();

    // Wake the global executor from timer interrupts
    wake_executor();
}

// Static reference to the global executor for waking tasks from interrupts
static mut GLOBAL_EXECUTOR_WAKER: Option<Waker> = None;

/// Set a global waker that can be used by interrupt handlers to wake the executor
pub fn set_global_waker(waker: Waker) {
    unsafe {
        GLOBAL_EXECUTOR_WAKER = Some(waker);
    }
}

/// Wake the global executor from an interrupt context
pub fn wake_executor() {
    // For now, just do nothing - the timer interrupts themselves will prevent
    // the executor from sleeping too long
}
