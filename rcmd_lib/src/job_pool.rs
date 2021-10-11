use std::{
    collections::HashMap,
    process::{ExitStatus, Stdio},
    sync::{atomic::AtomicU64, Arc},
};

pub use rcmd_data::{JobOutput, JobSpec, JobStatus};
use tokio::{
    io,
    process::Command,
    sync::{
        mpsc::{self, UnboundedReceiver},
        oneshot, Mutex,
    },
    time::Instant,
};
use tracing::{error, info, instrument};

use crate::util::{manage_process, receive_all_lines, receive_lines_until};

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
        // TODO: unbounded channels can lead to excessive memory usage
        //       a more advanced implementation would spill to disk
        stdout_rx: UnboundedReceiver<(String, Instant)>,
        stderr_rx: UnboundedReceiver<(String, Instant)>,
        exit_rx: oneshot::Receiver<io::Result<ExitStatus>>,
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
    pid: Option<u32>,
    spec: JobSpec,
    state: JobState,
    output: JobOutput,
}

pub struct JobPool {
    // using counter instead of uuid for more convenient usage from client
    // amount of jobs should not be considered private
    next_job_id: AtomicU64,
    jobs: Arc<Mutex<HashMap<u64, Job>>>,
}

impl JobPool {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            next_job_id: AtomicU64::new(0),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// submit a job for execution
    /// always succeeds with a job id, errors have to be checked with status
    #[instrument(skip(self))]
    pub async fn submit(&self, command: &str, args: &[&str]) -> u64 {
        let id = self
            .next_job_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        info!("try to spawn process of job with id {}", id);
        // spawn process and pipe stdout/stderr
        let process = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        let mut pid = None;
        let state = match process {
            Ok(process) => {
                pid = process.id();
                info!("process spawned, pid: {:?}", pid);
                // channels for std streams, exit and kill signal
                let (stdout_tx, stdout_rx) = mpsc::unbounded_channel::<(String, Instant)>();
                let (stderr_tx, stderr_rx) = mpsc::unbounded_channel::<(String, Instant)>();
                let (exit_tx, exit_rx) = oneshot::channel::<io::Result<ExitStatus>>();
                let (kill_tx, kill_rx) = oneshot::channel::<()>();
                // spawn manager task that updates stream/exit channels and listens for kill signal
                let _ = tokio::spawn(manage_process(
                    id, process, stdout_tx, stderr_tx, exit_tx, kill_rx,
                ));
                JobState::Running {
                    stdout_rx,
                    stderr_rx,
                    exit_rx,
                    kill_tx,
                }
            }
            Err(err) => {
                info!("process could not be spawned, error: {:?}", err);
                JobState::Error {
                    msg: err.to_string(),
                }
            }
        };
        let output = JobOutput::new();
        let spec = JobSpec::new(command, args);
        let job = Job {
            id,
            pid,
            spec,
            state,
            output,
        };
        self.jobs.lock().await.insert(id, job);
        id
    }

    /// deletes job if exists and returns None
    /// associated process is guaranteed to have been terminated
    /// if job ends up in error state, returns Some(error message)
    #[instrument(skip(self))]
    pub async fn delete(&self, id: u64) -> Option<String> {
        info!("try to delete job");
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        if let JobState::Running { .. } = job.state {
            let job = self.update_job_state(job, true).await;
            if let JobState::Error { msg } = job.state {
                let msg = format!("deletion resulted in error state: {}", msg);
                error!("{}", &msg);
                return Some(msg);
            }
        }
        info!("deleted job");
        None
    }

    /// gets job status if job exists
    #[instrument(skip(self))]
    pub async fn status(&self, id: u64) -> Option<JobStatus> {
        info!("try to get status");
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        let job = self.update_job_state(job, false).await;
        let status = Some(JobStatus::from(&job.state));
        jobs.insert(id, job);
        info!("returning status");
        status
    }

    /// gets job output if job exists
    #[instrument(skip(self))]
    pub async fn output(&self, id: u64) -> Option<JobOutput> {
        info!("try to get output");
        let mut jobs = self.jobs.lock().await;
        let job = jobs.remove(&id)?;
        let job = self.update_job_state(job, false).await;
        let output = Some(job.output.clone());
        jobs.insert(id, job);
        info!("got output");
        output
    }

    /// get a mapping of all jobs and their specs
    #[instrument(skip_all)]
    pub async fn list(&self) -> HashMap<u64, JobSpec> {
        info!("get a list of jobs");
        let jobs = self.jobs.lock().await;
        jobs.iter()
            .map(|(id, job)| (*id, job.spec.clone()))
            .collect()
    }

    /// update job's state and output
    /// if kill is true, send kill signal to job's process and collect all outstanding output
    /// returns updated job
    async fn update_job_state(&self, mut job: Job, kill: bool) -> Job {
        job.state = match job.state {
            JobState::Running {
                mut stdout_rx,
                mut stderr_rx,
                mut exit_rx,
                kill_tx,
            } => {
                if kill {
                    info!(
                        "send kill signal for job {}'s process with pid {:?}",
                        job.id, job.pid
                    );
                    if kill_tx.send(()).is_err() {
                        info!("kill signal channel receiver dropped, process already exited");
                    }
                    match exit_rx.await {
                        Ok(exit_result) => {
                            let (state, output) =
                                finish_job(exit_result, job.output, stdout_rx, stderr_rx).await;
                            job.output = output;
                            state
                        }
                        // TODO: handle error instead of panic
                        // this should never happen, manager task should never complete without sending
                        // could either return job error state or add extra "internal error" state to return here
                        Err(_err) => {
                            panic!("exit channel sender unexpectedly dropped without sending")
                        }
                    }
                } else {
                    match exit_rx.try_recv() {
                        Ok(exit_result) => {
                            let (state, output) =
                                finish_job(exit_result, job.output, stdout_rx, stderr_rx).await;
                            job.output = output;
                            state
                        }
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

/// reads all remaining lines from stdout/stderr channels and returns full job output
async fn finish_job(
    exit_status: io::Result<ExitStatus>,
    mut job_output: JobOutput,
    mut stdout_rx: UnboundedReceiver<(String, Instant)>,
    mut stderr_rx: UnboundedReceiver<(String, Instant)>,
) -> (JobState, JobOutput) {
    let stdout_lines = receive_all_lines(&mut stdout_rx).await;
    let stderr_lines = receive_all_lines(&mut stderr_rx).await;
    job_output.append(stdout_lines, stderr_lines);
    match exit_status {
        Ok(exit_status) if exit_status.code().is_some() => {
            let exit_code = exit_status.code().unwrap();
            (JobState::Completed { exit_code }, job_output)
        }
        Ok(_exit_status) => (JobState::Terminated, job_output),
        Err(io_err) => {
            let error_state = JobState::Error {
                msg: format!(
                    "unexpected io error when waiting for job process {:?}",
                    io_err
                ),
            };
            (error_state, job_output)
        }
    }
}

#[cfg(test)]
mod test {
    use std::{sync::Once, time::Duration};

    use lazy_static::lazy_static;

    use rcmd_data::{JobSpec, JobStatus};
    use tokio::{runtime::Runtime, time::sleep};

    use super::JobPool;

    lazy_static! {
        static ref RUNTIME: Runtime = Runtime::new().unwrap();
    }
    static INIT: Once = Once::new();

    pub fn setup() {
        INIT.call_once(|| {
            tracing_subscriber::fmt::init();
        });
    }

    // testing output of oneshot echo command
    #[test]
    fn test_output_echo() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("echo", &["hi"]).await;
            sleep(Duration::from_millis(100)).await;
            let output = pool.output(id).await;
            assert!(output.is_some());
            let output = output.unwrap();
            assert_eq!("hi\n".to_string(), output.stdout())
        });
    }

    // testing status of oneshot ls command
    #[test]
    fn test_status_ls() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("ls", &[]).await;
            sleep(Duration::from_millis(100)).await;
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

    // testing status of sleep job before and after delete
    #[test]
    fn test_status_sleep_deleted() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("sleep", &["5"]).await;
            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            assert_eq!(JobStatus::Running, status);
            let err = pool.delete(id).await;
            assert_eq!(None, err);
            let status = pool.status(id).await;
            assert!(status.is_none());
        });
    }

    // testing output of echo loop
    #[test]
    fn test_output_repeated_echo() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool
                .submit("bash", &["-c", "while true; do echo hi; sleep 1; done"])
                .await;
            sleep(Duration::from_millis(100)).await;

            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            assert_eq!(JobStatus::Running, status);
            let output = pool.output(id).await.unwrap();
            assert_eq!("hi\n", &output.stdout());
            sleep(Duration::from_millis(1000)).await;

            let output = pool.output(id).await.unwrap();
            assert_eq!("hi\nhi\n", &output.stdout());
            let err = pool.delete(id).await;
            assert_eq!(None, err);
            let status = pool.status(id).await;
            assert!(status.is_none());
        });
    }

    // testing invalid command
    #[test]
    fn test_invalid_command() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("abcdfg", &[]).await;

            let status = pool.status(id).await;
            assert!(status.is_some());
            let status = status.unwrap();
            match status {
                JobStatus::Error { msg: _ } => {}
                s => panic!("expected error job status, got: {:?}", s),
            }
        });
    }

    // testing listing of jobs
    #[test]
    fn test_list_jobs() {
        setup();
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let first = pool.submit("abcdfg", &[]).await;
            let second = pool.submit("ls", &[]).await;
            let third = pool.submit("echo", &["hi"]).await;
            let delete_response = pool.delete(second).await;
            let listed = pool.list().await;
            assert!(delete_response.is_none());
            assert!(listed.contains_key(&first));
            assert!(listed.contains_key(&third));
            assert_eq!(2, listed.len());
            let first_spec = JobSpec::new("abcdfg", &[]);
            let third_spec = JobSpec::new("echo", &["hi"]);
            assert_eq!(&first_spec, listed.get(&first).unwrap());
            assert_eq!(&third_spec, listed.get(&third).unwrap());
        });
    }
}
