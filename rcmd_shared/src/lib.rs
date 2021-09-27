use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Job {
    pub id: u64,
    pub cmd: CommandSpec,
    pub status: JobStatus,
    pub stdout: String,
    pub stderr: String,
}

impl Job {
    pub fn new(id: u64, cmd: CommandSpec) -> Self {
        Job {
            id,
            cmd,
            status: JobStatus::Submitted,
            stdout: "".to_string(),
            stderr: "".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub enum JobStatus {
    Submitted,
    Completed(Option<i32>),
}

#[derive(Serialize, Deserialize, Clone)]
pub struct CommandSpec {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct JobCreatedResponse {
    pub job_id: u64,
}
