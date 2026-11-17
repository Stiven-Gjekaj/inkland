//! A worker thread that does only the latest job.
//!
//! The app sends a new job each time the config or the view changes. A job
//! that waits is replaced by a newer job, so the worker never does work that
//! is already old.

use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

struct Slot<J> {
    job: Option<J>,
    closed: bool,
}

type Shared<J> = Arc<(Mutex<Slot<J>>, Condvar)>;

/// A thread that runs a function on the latest job.
pub struct Worker<J> {
    shared: Shared<J>,
    thread: Option<JoinHandle<()>>,
}

impl<J: Send + 'static> Worker<J> {
    /// Start a thread that calls `run` for each job that it takes.
    pub fn spawn(name: &str, mut run: impl FnMut(J) + Send + 'static) -> Worker<J> {
        let shared: Shared<J> = Arc::new((
            Mutex::new(Slot {
                job: None,
                closed: false,
            }),
            Condvar::new(),
        ));
        let theirs = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, cv) = &*theirs;
                        let mut slot = lock.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if slot.closed {
                                return;
                            }
                            if let Some(job) = slot.job.take() {
                                break job;
                            }
                            slot = cv.wait(slot).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    run(job);
                }
            })
            .expect("the system starts a thread");
        Worker {
            shared,
            thread: Some(thread),
        }
    }

    /// Give a job to the worker. A job that waits is replaced.
    pub fn submit(&self, job: J) {
        let (lock, cv) = &*self.shared;
        lock.lock().unwrap_or_else(|e| e.into_inner()).job = Some(job);
        cv.notify_one();
    }
}

impl<J> Drop for Worker<J> {
    /// Stop the thread after its current job. A job that waits is dropped.
    fn drop(&mut self) {
        let (lock, cv) = &*self.shared;
        lock.lock().unwrap_or_else(|e| e.into_inner()).closed = true;
        cv.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn the_worker_does_the_last_job_and_skips_replaced_jobs() {
        let (tx, rx) = mpsc::channel();
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let w = Worker::spawn("test", move |job: u32| {
            // Job 0 waits for the gate, so jobs 1 to 9 come while it runs.
            if job == 0 {
                gate_rx.recv().unwrap();
            }
            tx.send(job).unwrap();
        });
        w.submit(0);
        // Wait until the worker has taken job 0.
        while w.shared.0.lock().unwrap().job.is_some() {
            std::thread::yield_now();
        }
        for job in 1..10 {
            w.submit(job);
        }
        gate_tx.send(()).unwrap();
        assert_eq!(rx.recv().unwrap(), 0);
        assert_eq!(rx.recv().unwrap(), 9);
        drop(w);
        assert!(rx.recv().is_err(), "no other job runs");
    }
}
