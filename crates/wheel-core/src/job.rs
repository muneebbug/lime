use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::info;
use uuid::Uuid;
use chrono::{DateTime, Utc};

pub type JobId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub action_id: String,
    /// Input file paths
    pub inputs: Vec<PathBuf>,
    /// Action-specific parameters
    pub params: serde_json::Value,
    pub status: JobStatus,
    pub progress: f32, // 0.0 to 1.0
    pub error: Option<String>,
    /// Output files produced
    pub outputs: Vec<PathBuf>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Job {
    pub fn new(action_id: impl Into<String>, inputs: Vec<PathBuf>, params: serde_json::Value) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            action_id: action_id.into(),
            inputs,
            params,
            status: JobStatus::Queued,
            progress: 0.0,
            error: None,
            outputs: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Event sent from job runner to listeners
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobEvent {
    pub job_id: JobId,
    pub kind: JobEventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobEventKind {
    Started,
    Progress { value: f32 },
    Completed { outputs: Vec<PathBuf> },
    Failed { error: String },
    Cancelled,
}

/// A simple bounded job queue backed by tokio.
/// The actual execution logic is injected via the executor closure.
pub struct JobQueue {
    jobs: Arc<Mutex<Vec<Job>>>,
    sender: mpsc::Sender<Job>,
    event_tx: tokio::sync::broadcast::Sender<JobEvent>,
}

impl JobQueue {
    /// Create a new queue with a given concurrency limit.
    pub fn new(concurrency: usize) -> (Self, tokio::sync::broadcast::Receiver<JobEvent>) {
        let (sender, mut receiver) = mpsc::channel::<Job>(128);
        let (event_tx, event_rx) = tokio::sync::broadcast::channel::<JobEvent>(256);
        let jobs = Arc::new(Mutex::new(Vec::<Job>::new()));

        let jobs_clone = Arc::clone(&jobs);
        let event_tx_clone = event_tx.clone();

        tokio::spawn(async move {
            use tokio::task::JoinSet;
            let mut set = JoinSet::new();
            let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));

            while let Some(job) = receiver.recv().await {
                let job_id = job.id;
                let action_id = job.action_id.clone();
                let sem = Arc::clone(&sem);
                let event_tx = event_tx_clone.clone();
                let jobs = Arc::clone(&jobs_clone);

                // Update status to Queued in the jobs list
                {
                    let mut lock = jobs.lock().await;
                    if let Some(j) = lock.iter_mut().find(|j| j.id == job_id) {
                        j.status = JobStatus::Queued;
                    }
                }

                set.spawn(async move {
                    let _permit = sem.acquire().await.unwrap();
                    let _ = event_tx.send(JobEvent {
                        job_id,
                        kind: JobEventKind::Started,
                    });

                    // Placeholder: actual execution is handled by the Tauri command layer
                    // which processes the job and sends events directly.
                    info!("Job {} ({}) dispatched to executor", job_id, action_id);
                });
            }

            while set.join_next().await.is_some() {}
        });

        (
            Self {
                jobs,
                sender,
                event_tx,
            },
            event_rx,
        )
    }

    /// Enqueue a job; returns the job id
    pub async fn enqueue(&self, job: Job) -> anyhow::Result<JobId> {
        let id = job.id;
        {
            let mut lock = self.jobs.lock().await;
            lock.push(job.clone());
        }
        self.sender.send(job).await?;
        Ok(id)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<JobEvent> {
        self.event_tx.subscribe()
    }

    pub async fn get_job(&self, id: JobId) -> Option<Job> {
        let lock = self.jobs.lock().await;
        lock.iter().find(|j| j.id == id).cloned()
    }

    pub async fn all_jobs(&self) -> Vec<Job> {
        let lock = self.jobs.lock().await;
        lock.clone()
    }

    pub async fn update_job<F>(&self, id: JobId, f: F)
    where
        F: FnOnce(&mut Job),
    {
        let mut lock = self.jobs.lock().await;
        if let Some(job) = lock.iter_mut().find(|j| j.id == id) {
            f(job);
            job.updated_at = Utc::now();
        }
    }

    pub fn event_sender(&self) -> tokio::sync::broadcast::Sender<JobEvent> {
        self.event_tx.clone()
    }
}
