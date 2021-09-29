use std::{
    collections::HashMap,
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

    pub async fn submit(&self, command: &str, args: &[String]) -> u64 {
        let c = Command::new(command).args(args).spawn().unwrap();
        let id = self
            .next_job_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.jobs.lock().await.insert(id, c);
        id
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
    use tokio::runtime::Runtime;

    use crate::JobPool;

    #[test]
    fn test_output() {
        let pool = JobPool::new();
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let id = pool.submit("ls", &[]).await;
            let output = pool.output(id).await;
            println!("{:?}", output);
        });
    }
}
