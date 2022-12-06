use std::{
    fmt,
    sync::{mpsc, Arc, Mutex},
    thread::{self, JoinHandle},
};

pub const MAX_THREAD_POOL_SIZE: usize = 10;

#[derive(Debug)]
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<JobSender>,
}

type Job = Box<dyn FnOnce() + Send + 'static>;
type JobSender = mpsc::Sender<Job>;

#[derive(Debug)]
struct Worker {
    id: WorkerId,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Worker {
        let thread = Some(thread::spawn(move || loop {
            let message = receiver.lock().unwrap().recv();

            match message {
                Ok(job) => {
                    println!("Worker {id} got a job; executing.");

                    job();
                }
                Err(_) => {
                    println!("Worker {id} disconnected; shutting down.");
                    break;
                }
            }
        }));

        Worker { id, thread }
    }
}

type WorkerId = usize;

impl ThreadPool {
    pub fn build(size: usize) -> Result<ThreadPool, PoolCreationError> {
        match size {
            1..=MAX_THREAD_POOL_SIZE => {
                let (sender, receiver) = mpsc::channel();
                let receiver = Arc::new(Mutex::new(receiver));
                let workers = (0..=size)
                    .into_iter()
                    .enumerate()
                    .map(|(id, _)| Worker::new(id, Arc::clone(&receiver)))
                    .collect();

                Ok(ThreadPool {
                    workers,
                    sender: Some(sender),
                })
            }
            _ => Err(PoolCreationError),
        }
    }

    pub fn execute<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let job = Box::new(f);
        self.sender
            .as_ref()
            .unwrap()
            .send(job)
            .map_err(|err| println!("Error occured: {err}"))
            .expect("Should not fail");
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());

        for worker in &mut self.workers {
            println!("Shutting down worker {}", worker.id);

            if let Some(thread) = worker.thread.take() {
                thread.join().unwrap();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PoolCreationError;

impl fmt::Display for PoolCreationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "tried to build a thread pool with invalid size")
    }
}
