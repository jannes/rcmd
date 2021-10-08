use std::{
    collections::HashMap,
    process::{ExitStatus, Stdio},
    sync::{atomic::AtomicU64, Arc},
};

use tokio::{
    io,
    process::Command,
    sync::{
        mpsc::{self, UnboundedReceiver},
        oneshot::{self, Receiver},
        Mutex,
    },
    time::Instant,
};
use util::{manage_process, receive_lines_until};

use crate::util::receive_all_lines;

mod util;

pub struct CommandSpec {
    pub cmd: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct JobOutput {
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
}

impl JobOutput {
    pub fn new() -> Self {
        Self {
            stdout_lines: Vec::new(),
            stderr_lines: Vec::new(),
        }
    }

    pub fn append(&mut self, mut stdout_lines: Vec<String>, mut stderr_lines: Vec<String>) {
        self.stdout_lines.append(&mut stdout_lines);
        self.stderr_lines.append(&mut stderr_lines);
    }

    pub fn stdout(&self) -> String {
        self.stdout_lines.join("")
    }

    pub fn stderr(&self) -> String {
        self.stderr_lines.join("")
    }
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
            JobState::Running { .. } => JobStatus::Running,
            JobState::Completed { exit_code } => JobStatus::Completed {
                exit_code: *exit_code,
            },
            JobState::Terminated => JobStatus::Terminated,
            JobState::Error { msg } => JobStatus::Error {
                msg: msg.to_string(),
            },
        }
    }
}

enum JobState {
    Running {
        stdout_rx: UnboundedReceiver<(String, Instant)>,
        stderr_rx: UnboundedReceiver<(String, Instant)>,
        exit_rx: Receiver<io::Result<ExitStatus>>,
        kill_tx: oneshot::Sender<()>,
    },
    Completed {
        exit_code: i32,
    },
    Terminated,
    Error {
        msg: String,
    },
}

struct Job {
    id: u64,
    state: JobState,
    output: JobOutput,
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
        let (stdout_tx, stdout_rx) = mpsc::unbounded_channel::<(String, Instant)>();
        let (stderr_tx, stderr_rx) = mpsc::unbounded_channel::<(String, Instant)>();
        let (exit_tx, exit_rx) = oneshot::channel::<io::Result<ExitStatus>>();
        let (kill_tx, kill_rx) = oneshot::channel::<()>();
        let state = match process {
            Ok(process) => {
                let _ = tokio::spawn(manage_process(
                    process, stdout_tx, stderr_tx, exit_tx, kill_rx,
                ));
                JobState::Running {
                    stdout_rx,
                    stderr_rx,
                    exit_rx,
                    kill_tx,
                }
            }
            Err(err) => JobState::Error {
                msg: err.to_string(),
            },
        };
        let output = JobOutput::new();
        let job = Job { id, state, output };
        self.jobs.lock().await.insert(id, job);
        id
    }

    pub async fn delete(&self, id: u64) -> Option<String> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        if let JobState::Running { .. } = job.state {
            // TODO: decide error handling
            let _job = self.update_job_state(job, true).await;
        }
        None
    }

    pub async fn status(&self, id: u64) -> Option<JobStatus> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        let job = self.update_job_state(job, false).await;
        let status = Some(JobStatus::from(&job.state));
        jobs.insert(id, job);
        status
    }

    pub async fn output(&self, id: u64) -> Option<JobOutput> {
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        let job = self.update_job_state(job, false).await;
        let output = Some(job.output.clone());
        jobs.insert(id, job);
        output
    }

    async fn update_job_state(&self, mut job: Job, kill: bool) -> Job {
        job.state = match job.state {
            JobState::Running {
                mut stdout_rx,
                mut stderr_rx,
                mut exit_rx,
                kill_tx,
            } => {
                if kill {
                    if kill_tx.send(()).is_err() {
                        todo!()
                    }
                    match exit_rx.await {
                        Ok(Ok(exit_status)) => {
                            let (state, output) =
                                finish_job(exit_status, job.output, stdout_rx, stderr_rx).await;
                            job.output = output;
                            state
                        }
                        Ok(Err(err)) => todo!(),
                        Err(_) => todo!(),
                    }
                } else {
                    match exit_rx.try_recv() {
                        Ok(Ok(exit_status)) => {
                            let (state, output) =
                                finish_job(exit_status, job.output, stdout_rx, stderr_rx).await;
                            job.output = output;
                            state
                        }
                        Ok(Err(err)) => todo!(),
                        _ => {
                            let now = Instant::now();
                            let stdout_lines = receive_lines_until(&mut stdout_rx, &now).await;
                            let stderr_lines = receive_lines_until(&mut stderr_rx, &now).await;
                            job.output.append(stdout_lines, stderr_lines);
                            JobState::Running {
                                stdout_rx,
                                stderr_rx,
                                exit_rx,
                                kill_tx,
                            }
                        }
                    }
                }
            }
            x => x,
        };
        job
    }
}

impl Default for JobPool {
    fn default() -> Self {
        Self::new()
    }
}

async fn finish_job(
    exit_status: ExitStatus,
    mut job_output: JobOutput,
    mut stdout_rx: UnboundedReceiver<(String, Instant)>,
    mut stderr_rx: UnboundedReceiver<(String, Instant)>,
) -> (JobState, JobOutput) {
    let stdout_lines = receive_all_lines(&mut stdout_rx).await;
    let stderr_lines = receive_all_lines(&mut stderr_rx).await;
    job_output.append(stdout_lines, stderr_lines);
    if let Some(exit_code) = exit_status.code() {
        (JobState::Completed { exit_code }, job_output)
    } else {
        (JobState::Terminated, job_output)
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
            sleep(Duration::from_millis(1000)).await;
            let output = pool.output(id).await;
            assert!(output.is_some());
            let output = output.unwrap();
            assert_eq!("hi\n".to_string(), output.stdout())
        });
    }

    #[test]
    fn test_status() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("ls", &[]).await;
            sleep(Duration::from_millis(1000)).await;
            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            if let JobStatus::Completed { exit_code } = status {
                assert_eq!(0, exit_code)
            } else {
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

    #[test]
    fn test_output_long_running() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool
                .submit("bash", &["-c", "while true; do echo hi; sleep 1; done"])
                .await;
            sleep(Duration::from_millis(1000)).await;

            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            assert_eq!(JobStatus::Running, status);
            let output = pool.output(id).await.unwrap();
            assert_eq!("hi\n", &output.stdout());
            sleep(Duration::from_millis(1000)).await;

            let output = pool.output(id).await.unwrap();
            assert_eq!("hi\nhi\n", &output.stdout());
            sleep(Duration::from_millis(1000)).await;

            let err = pool.delete(id).await;
            assert_eq!(None, err);
            let status = pool.status(id).await;
            assert!(status.is_none());
        });
    }
}
