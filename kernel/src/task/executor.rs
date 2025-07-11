use super::{Task, TaskId};
use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

use x86_64::instructions::interrupts::{self, enable_and_hlt};

const MAX_TASKS: usize = 100;

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

        while let Some(task_id) = task_queue.pop() {
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
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => {}
            }
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.run_ready_tasks();
            self.sleep_if_idle();
        }
    }

    /// Run a single batch of ready tasks and then yield
    /// This version is designed for use as a kernel process
    pub fn run_batch(&mut self) {
        // Process a limited number of tasks to avoid blocking too long
        let mut tasks_processed = 0;
        const MAX_TASKS_PER_BATCH: usize = 10;

        while tasks_processed < MAX_TASKS_PER_BATCH {
            if let Some(task_id) = self.task_queue.pop() {
                let task = match self.tasks.get_mut(&task_id) {
                    Some(task) => task,
                    None => continue, // task no longer exists
                };
                let waker = self
                    .waker_cache
                    .entry(task_id)
                    .or_insert_with(|| TaskWaker::new(task_id, self.task_queue.clone()));
                let mut context = Context::from_waker(waker);
                match task.poll(&mut context) {
                    Poll::Ready(()) => {
                        // task done -> remove it and its cached waker
                        self.tasks.remove(&task_id);
                        self.waker_cache.remove(&task_id);
                    }
                    Poll::Pending => {}
                }
                tasks_processed += 1;
            } else {
                // No more tasks ready, yield
                break;
            }
        }
        // After processing tasks, this function will return and allow the process scheduler
        // to switch to other processes. The scheduler will call this function again later.
    }

    fn sleep_if_idle(&self) {
        if self.tasks.is_empty() {
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
        self.task_queue.push(self.task_id).expect("task_queue full");
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

/// Global executor instance for use as a kernel process
static mut GLOBAL_EXECUTOR: Option<Executor> = None;
static mut EXECUTOR_INITIALIZED: bool = false;

/// Initialize the global executor with tasks
pub fn init_global_executor() {
    use crate::task::Task;

    unsafe {
        if !EXECUTOR_INITIALIZED {
            let mut executor = Executor::new();
            executor.spawn(Task::new(crate::example_task()));
            executor.spawn(Task::new(crate::task::keyboard::print_keypresses()));
            GLOBAL_EXECUTOR = Some(executor);
            EXECUTOR_INITIALIZED = true;
        }
    }
}

/// Entry point for the executor kernel process
/// This function will be called repeatedly by the process scheduler
extern "C" fn executor_entry_point() -> ! {
    crate::serial_println!("Executor kernel process started!");
    loop {
        crate::serial_println!("Executor running batch...");
        unsafe {
            let executor_ptr = &raw mut GLOBAL_EXECUTOR;
            if let Some(executor) = &mut *executor_ptr {
                executor.run_batch();
            } else {
                crate::serial_println!("Executor not initialized!");
            }
        }
        crate::serial_println!("Executor batch complete, halting...");
        // Use a simple pause to avoid busy-waiting
        // This allows other processes to run while keeping this process alive
        x86_64::instructions::hlt();
    }
}

/// Get the entry point function for the executor kernel process
pub fn get_executor_entry_point() -> extern "C" fn() -> ! {
    executor_entry_point
}
