//! A single worker, with bounded channels and cleanup on every server exit path.
use crate::sample::{Batch, Identity, Job};
use crossbeam_channel::{Receiver, Sender, bounded};
use std::thread::{self, JoinHandle};

pub(crate) struct Worker {
    jobs: Option<Sender<Job>>,
    results: Option<Receiver<Batch>>,
    thread: Option<JoinHandle<()>>,
    pub active: Option<Identity>,
}
impl Worker {
    pub fn new(execute: impl Fn(Job) -> Batch + Send + 'static) -> Self {
        let (jobs, incoming) = bounded(1);
        let (outgoing, results) = bounded(1);
        let thread = thread::spawn(move || {
            while let Ok(job) = incoming.recv() {
                let result = execute(job);
                if outgoing.send(result).is_err() {
                    break;
                }
            }
        });
        Self {
            jobs: Some(jobs),
            results: Some(results),
            thread: Some(thread),
            active: None,
        }
    }
    pub fn results(&self) -> &Receiver<Batch> {
        self.results
            .as_ref()
            .expect("worker receiver exists until cleanup")
    }
    pub fn dispatch(&mut self, job: Job) -> crate::Result<()> {
        self.active = Some(job.identity.clone());
        self.jobs
            .as_ref()
            .ok_or("sample worker shut down")?
            .send(job)
            .map_err(|_| "sample worker disconnected")?;
        Ok(())
    }
    pub fn stop(&mut self) {
        if let Some(identity) = &self.active {
            identity.cancellation.cancel();
        }
        self.jobs.take();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
        self.results.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
