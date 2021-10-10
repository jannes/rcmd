use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use rcmd_lib::job_pool::JobPool;

pub struct JobPools {
    pub job_pools: Arc<RwLock<HashMap<String, Arc<JobPool>>>>,
}

impl JobPools {
    pub fn new() -> Self {
        Self {
            job_pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn has_pool(&self, client: &str) -> bool {
        self.job_pools.read().unwrap().contains_key(client)
    }

    pub fn get_pool(&self, client: &str) -> Option<Arc<JobPool>> {
        self.job_pools.read().unwrap().get(client).cloned()
    }

    pub fn create_pool(&self, client: &str) -> Arc<JobPool> {
        let pool = JobPool::new();
        self.job_pools
            .write()
            .unwrap()
            .insert(client.to_string(), Arc::new(pool));
        self.get_pool(client).unwrap()
    }
}

impl Default for JobPools {
    fn default() -> Self {
        Self::new()
    }
}
