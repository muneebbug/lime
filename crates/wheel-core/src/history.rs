use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

use crate::job::{Job, JobId, JobStatus};

/// SQLite-backed persistent history store for Wheel jobs.
#[derive(Clone)]
pub struct HistoryDb {
    conn: Arc<Mutex<Connection>>,
}

impl HistoryDb {
    /// Open or create the SQLite database at the specified path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create history DB directory: {:?}", parent))?;
        }

        let conn = Connection::open(path)
            .with_context(|| format!("Failed to open history DB at {:?}", path))?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    /// Open an in-memory SQLite database (ideal for tests).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .with_context(|| "Failed to open in-memory SQLite DB")?;
        
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;

            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                action_id TEXT NOT NULL,
                inputs TEXT NOT NULL,
                params TEXT NOT NULL,
                status TEXT NOT NULL,
                progress REAL NOT NULL,
                error TEXT,
                outputs TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_jobs_created_at ON jobs (created_at DESC);
            "#,
        )?;
        Ok(())
    }

    /// Insert a newly created job.
    pub fn insert_job(&self, job: &Job) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let inputs_json = serde_json::to_string(&job.inputs)?;
        let params_json = serde_json::to_string(&job.params)?;
        let status_str = serde_json::to_string(&job.status)?.trim_matches('"').to_string();
        let outputs_json = serde_json::to_string(&job.outputs)?;
        let created_at_str = job.created_at.to_rfc3339();
        let updated_at_str = job.updated_at.to_rfc3339();

        conn.execute(
            r#"
            INSERT INTO jobs (id, action_id, inputs, params, status, progress, error, outputs, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                progress = excluded.progress,
                error = excluded.error,
                outputs = excluded.outputs,
                updated_at = excluded.updated_at
            "#,
            params![
                job.id.to_string(),
                job.action_id,
                inputs_json,
                params_json,
                status_str,
                job.progress,
                job.error,
                outputs_json,
                created_at_str,
                updated_at_str,
            ],
        )?;
        Ok(())
    }

    /// Update an existing job.
    pub fn update_job(&self, job: &Job) -> Result<()> {
        self.insert_job(job)
    }

    /// Retrieve a job by its ID.
    pub fn get_job(&self, id: JobId) -> Result<Option<Job>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, action_id, inputs, params, status, progress, error, outputs, created_at, updated_at
            FROM jobs
            WHERE id = ?1
            "#,
        )?;

        let mut rows = stmt.query(params![id.to_string()])?;
        if let Some(row) = rows.next()? {
            let id_str: String = row.get(0)?;
            let action_id: String = row.get(1)?;
            let inputs_json: String = row.get(2)?;
            let params_json: String = row.get(3)?;
            let status_str: String = row.get(4)?;
            let progress: f32 = row.get(5)?;
            let error: Option<String> = row.get(6)?;
            let outputs_json: String = row.get(7)?;
            let created_at_str: String = row.get(8)?;
            let updated_at_str: String = row.get(9)?;

            let inputs: Vec<PathBuf> = serde_json::from_str(&inputs_json)?;
            let params: serde_json::Value = serde_json::from_str(&params_json)?;
            let status: JobStatus = serde_json::from_value(serde_json::Value::String(status_str))?;
            let outputs: Vec<PathBuf> = serde_json::from_str(&outputs_json)?;
            let created_at = DateTime::parse_from_rfc3339(&created_at_str)?.with_timezone(&Utc);
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)?.with_timezone(&Utc);

            Ok(Some(Job {
                id: Uuid::parse_str(&id_str)?,
                action_id,
                inputs,
                params,
                status,
                progress,
                error,
                outputs,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// Retrieve recent jobs ordered by creation time descending.
    pub fn get_recent_jobs(&self, limit: usize) -> Result<Vec<Job>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT id, action_id, inputs, params, status, progress, error, outputs, created_at, updated_at
            FROM jobs
            ORDER BY created_at DESC
            LIMIT ?1
            "#,
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f32>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;

        let mut jobs = Vec::new();
        for item in rows {
            let (id_str, action_id, inputs_json, params_json, status_str, progress, error, outputs_json, created_at_str, updated_at_str) = item?;
            let inputs: Vec<PathBuf> = serde_json::from_str(&inputs_json).unwrap_or_default();
            let params: serde_json::Value = serde_json::from_str(&params_json).unwrap_or_default();
            let status: JobStatus = serde_json::from_value(serde_json::Value::String(status_str)).unwrap_or(JobStatus::Failed);
            let outputs: Vec<PathBuf> = serde_json::from_str(&outputs_json).unwrap_or_default();
            let created_at = DateTime::parse_from_rfc3339(&created_at_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());
            let updated_at = DateTime::parse_from_rfc3339(&updated_at_str).map(|dt| dt.with_timezone(&Utc)).unwrap_or_else(|_| Utc::now());

            jobs.push(Job {
                id: Uuid::parse_str(&id_str).unwrap_or_else(|_| Uuid::new_v4()),
                action_id,
                inputs,
                params,
                status,
                progress,
                error,
                outputs,
                created_at,
                updated_at,
            });
        }

        Ok(jobs)
    }

    /// Delete a single job by ID.
    pub fn delete_job(&self, id: JobId) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM jobs WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    /// Clear all job history.
    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM jobs", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_history_db_crud() {
        let db = HistoryDb::open_in_memory().unwrap();

        let mut job = Job::new("convert.png", vec![PathBuf::from("C:/test.jpg")], serde_json::json!({}));
        assert_eq!(job.status, JobStatus::Queued);

        // Insert
        db.insert_job(&job).unwrap();

        // Retrieve
        let retrieved = db.get_job(job.id).unwrap().expect("Job should exist");
        assert_eq!(retrieved.id, job.id);
        assert_eq!(retrieved.action_id, "convert.png");
        assert_eq!(retrieved.inputs, vec![PathBuf::from("C:/test.jpg")]);
        assert_eq!(retrieved.status, JobStatus::Queued);

        // Update
        job.status = JobStatus::Completed;
        job.progress = 1.0;
        job.outputs = vec![PathBuf::from("C:/test.converted.png")];
        db.update_job(&job).unwrap();

        let updated = db.get_job(job.id).unwrap().expect("Job should exist");
        assert_eq!(updated.status, JobStatus::Completed);
        assert_eq!(updated.progress, 1.0);
        assert_eq!(updated.outputs, vec![PathBuf::from("C:/test.converted.png")]);

        // Recent jobs
        let recent = db.get_recent_jobs(10).unwrap();
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].id, job.id);

        // Delete
        db.delete_job(job.id).unwrap();
        assert!(db.get_job(job.id).unwrap().is_none());
        assert_eq!(db.get_recent_jobs(10).unwrap().len(), 0);
    }
}
