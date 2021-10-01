use std::{
    collections::HashMap,
    process::Stdio,
    sync::{atomic::AtomicU64, Arc},
};

use tokio::{
    io::AsyncReadExt,
    process::{Child, Command},
    sync::Mutex,
};

pub struct JobPool {
    pub next_job_id: AtomicU64,
    pub jobs: Arc<Mutex<HashMap<u64, Child>>>,
}

impl JobPool {
    pub fn new() -> Self {
        Self {
            next_job_id: AtomicU64::new(0),
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn submit(&self, command: &str, args: &[&str]) -> u64 {
        let c = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let id = self
            .next_job_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.jobs.lock().await.insert(id, c);
        id
    }

    pub async fn delete(&self, id: u64) -> Option<String> {
        let mut jobs = self.jobs.lock().await;
        let c = jobs.remove(&id);
        if let Some(mut c) = c {
            if let Ok(Some(_exit_status)) = c.try_wait() {
                return None;
            }
            match c.kill().await {
                Ok(_) => None,
                Err(err) => Some(err.to_string()),
            }
        } else {
            None
        }
    }

    pub async fn status(&self, id: u64) -> Option<i32> {
        let mut jobs = self.jobs.lock().await;
        let c = jobs.get_mut(&id);
        if let Some(c) = c {
            if let Ok(Some(exit_status)) = c.try_wait() {
                exit_status.code()
            } else {
                None
            }
        } else {
            None
        }
    }

    pub async fn output(&self, id: u64) -> Option<String> {
        let mut jobs = self.jobs.lock().await;
        let c = jobs.get_mut(&id);
        if let Some(Some(stdout)) = c.map(|c| &mut c.stdout) {
            let mut buffer = String::new();
            stdout.read_to_string(&mut buffer).await.unwrap();
            Some(buffer)
        } else {
            None
        }
    }
}

impl Default for JobPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use lazy_static::lazy_static;

    use tokio::{runtime::Runtime, time::sleep};

    use crate::JobPool;

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
            assert_eq!(Some("hi\n".to_string()), output)
        });
    }

    #[test]
    fn test_status() {
        let pool = JobPool::new();
        RUNTIME.block_on(async {
            let id = pool.submit("ls", &[]).await;
            sleep(Duration::from_millis(500)).await;
            let status = pool.status(id).await;
            assert_eq!(Some(0), status)
        });
    }
}
