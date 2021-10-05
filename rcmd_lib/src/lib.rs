use std::{collections::HashMap, process::Stdio, sync::{atomic::AtomicU64, Arc}, time::{Duration, SystemTime}};

use tokio::{io::{self, AsyncReadExt}, process::{Child, Command}, sync::Mutex, time::sleep};

pub struct CommandSpec {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Clone)]
pub struct JobOutput {
    stdout: Option<String>,
    stderr: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum JobStatus {
    Running,
    Completed { exit_code: i32 },
    Terminated,
    Error { msg: String },
}

impl From<&JobState> for JobStatus {
    fn from(state: &JobState) -> Self {
        match state {
            JobState::Running { process: _ } => JobStatus::Running,
            JobState::Completed { exit_code, output: _ } => JobStatus::Completed {
                exit_code: *exit_code,
            },
            JobState::Terminated { output: _ } => JobStatus::Terminated,
            JobState::Error { msg } => JobStatus::Error {
                msg: msg.to_string(),
            },
        }
    }
}

enum JobState {
    Running { process: Child },
    Completed { exit_code: i32, output: JobOutput },
    Terminated { output: JobOutput },
    Error { msg: String },
}

struct Job {
    id: u64,
    state: JobState,
}

pub struct JobPool {
    next_job_id: AtomicU64,
    jobs: Arc<Mutex<HashMap<u64, Job>>>,
}

impl JobPool {
    pub fn new() -> Self {
        Self {
            next_job_id: AtomicU64::new(0),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn submit(&self, command: &str, args: &[&str]) -> u64 {
        let id = self
            .next_job_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let process = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let state = match process {
            Ok(process) => JobState::Running { process },
            Err(err) => JobState::Error {
                msg: err.to_string(),
            },
        };
        let job = Job { id, state };
        self.jobs.lock().await.insert(id, job);
        id
    }

    pub async fn delete(&self, id: u64) -> Option<String> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        match job.state {
            JobState::Running { mut process } => match process.kill().await {
                Ok(_) => None,
                Err(err) => Some(err.to_string()),
            },
            _ => None,
        }
    }

    pub async fn status(&self, id: u64) -> Option<JobStatus> {
        self.update_job_state(&id).await?;
        let jobs = self.jobs.lock().await;
        let job = jobs.get(&id)?;
        Some(JobStatus::from(&job.state))
    }

    pub async fn output(&self, id: u64) -> Option<JobOutput> {
        self.update_job_state(&id).await?;
        let mut jobs = self.jobs.lock().await;
        let job = jobs.get_mut(&id)?;
        match &mut job.state {
            JobState::Running { process } => get_outstreams(process).await.ok(),
            JobState::Completed { exit_code: _, output } => Some(output.clone()),
            JobState::Terminated { output } => Some(output.clone()),
            JobState::Error { msg: _ } => None,
        }
    }

    async fn update_job_state(&self, id: &u64) -> Option<()> {
        let mut jobs = self.jobs.lock().await;
        let mut job = jobs.remove(id)?;
        drop(jobs);
        job.state = match job.state {
            JobState::Running { mut process } => match process.try_wait() {
                Ok(Some(exit_status)) if exit_status.code().is_some() => {
                    let output = get_outstreams(&mut process).await;
                    JobState::Completed {
                        exit_code: exit_status.code().unwrap(),
                        output: output.unwrap(),
                    }
                }
                Ok(Some(_exit_status)) => {
                    let output = get_outstreams(&mut process).await;
                    JobState::Terminated { output: output.unwrap() }
                }
                Ok(None) => {
                    let output = get_outstreams(&mut process).await;
                    JobState::Terminated { output: output.unwrap() }
                }
                Err(err) => JobState::Error {
                    msg: format!("error: {}", err),
                },
            },
            x => x,
        };
        let mut jobs = self.jobs.lock().await;
        jobs.insert(*id, job);
        Some(())
    }
}

async fn get_outstreams(process: &mut Child) -> io::Result<JobOutput> {
    dbg!("get outstream start: {:?}", SystemTime::now());
    let stdout = if let Some(stdout) = &mut process.stdout {
        let mut buffer: Vec<u8> = Vec::new();
        loop {
             tokio::select! {
                result = stdout.read(&mut buffer) => {
                    let _bytes_read = result?;
                }
                _ = sleep(Duration::from_millis(10)) => {
                    break;
                }
             };
        }
        let text = String::from_utf8(buffer).unwrap_or_else(|_| "NON-UTF8".to_string());
        Some(text)
    } else {
        None
    };
    let stderr = if let Some(stderr) = &mut process.stderr {
        let mut buffer: Vec<u8> = Vec::new();
        loop {
           tokio::select! {
                result = stderr.read(&mut buffer) => {
                    let _bytes_read = result?;
                }
                _ = sleep(Duration::from_millis(10)) => {
                    break;
                }
             };
        }
        let text = String::from_utf8(buffer).unwrap_or_else(|_| "NON-UTF8".to_string());
        Some(text)
    } else {
        None
    };
    dbg!("get outstream end: {:?}", SystemTime::now());
    Ok(JobOutput { stdout, stderr })
}


impl Default for JobPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, SystemTime};

    use lazy_static::lazy_static;

    use tokio::{runtime::Runtime, time::sleep};

    use crate::{JobPool, JobStatus};

    lazy_static! {
        static ref RUNTIME: Runtime = Runtime::new().unwrap();
    }

    #[test]
    fn test_output() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("echo", &["hi"]).await;
            sleep(Duration::from_millis(500)).await;
            let output = pool.output(id).await;
            assert!(output.is_some());
            let output = output.unwrap();
            assert_eq!(Some("hi\n".to_string()), output.stdout)
        });
    }

    #[test]
    fn test_status() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("ls", &[]).await;
            sleep(Duration::from_millis(500)).await;
            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            if let JobStatus::Completed { exit_code } = status {
                assert_eq!(0, exit_code)
            }
            else {
                panic!("unexpected job status: {:?}", status);
            }
        });
    }

    #[test]
    fn test_status_deleted() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            dbg!("submit: {:?}", SystemTime::now());
            let id = pool.submit("sleep", &["5"]).await;
            dbg!("status: {:?}", SystemTime::now());
            let status = pool.status(id).await;
            dbg!("assert: {:?}", SystemTime::now());
            assert!(status.is_some());
            let status = status.unwrap();
            assert_eq!(JobStatus::Running, status);
            let err = pool.delete(id).await;
            assert_eq!(None, err);
            let status = pool.status(id).await;
            assert!(status.is_none());
        });
    }
}
