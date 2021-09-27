use std::{
    collections::HashMap,
    process::Stdio,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use rcmd_shared::{CommandSpec, Job, JobStatus};
use tokio::{io, process::Command};

pub struct JobsState {
    pub next_job_id: AtomicU64,
    pub jobs: Arc<Mutex<HashMap<u64, Job>>>,
}

impl JobsState {
    pub async fn submit(&self, cmd_spec: CommandSpec) -> u64 {
        let id = self
            .next_job_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let job = Job::new(id, cmd_spec.clone());
        self.jobs.lock().unwrap().insert(id, job);

        let jobs = self.jobs.clone();
        tokio::spawn(async move {
            if let Err(e) = run_job(jobs, cmd_spec, id).await {
                todo!()
            }
        });

        id
    }
}

async fn run_job(
    jobs: Arc<Mutex<HashMap<u64, Job>>>,
    cmd_spec: CommandSpec,
    cmd_id: u64,
) -> Result<(), io::Error> {
    // spawn process
    let process = Command::new(cmd_spec.cmd)
        .args(cmd_spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // wait for process to finish
    let output = process.wait_with_output().await?;
    // update job
    let mut jobs_guard = jobs.lock().unwrap();
    let job = jobs_guard.get_mut(&cmd_id);
    if let Some(j) = job {
        j.status = JobStatus::Completed(output.status.code());
        j.stderr = String::from_utf8(output.stderr)
            .ok()
            .unwrap_or_else(|| "".to_string());
        j.stdout = String::from_utf8(output.stdout)
            .ok()
            .unwrap_or_else(|| "".to_string());
    }
    drop(jobs_guard);
    Ok(())
}
