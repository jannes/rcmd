use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use rcmd_shared::{CommandSpec, Job, JobCreatedResponse};
use rocket::{
    config::{CipherSuite, MutualTls, TlsConfig},
    serde::json::Json,
    Config, State,
};

use crate::state::JobsState;

#[macro_use]
extern crate rocket;

mod state;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/jobs", format = "json", data = "<command>")]
async fn start_job(
    command: Json<CommandSpec>,
    jobs_state: &State<JobsState>,
) -> Json<JobCreatedResponse> {
    Json(JobCreatedResponse {
        job_id: jobs_state.submit(command.into_inner()).await,
    })
}

#[get("/jobs")]
fn get_jobs(jobs_state: &State<JobsState>) -> Json<Vec<Job>> {
    Json(jobs_state.jobs.lock().unwrap().values().cloned().collect())
}

#[get("/jobs/<id>")]
fn get_job(id: u64, jobs_state: &State<JobsState>) -> Option<Json<Job>> {
    jobs_state
        .jobs
        .lock()
        .unwrap()
        .get(&id)
        .map(|j| Json(j.clone()))
}

#[launch]
fn rocket() -> _ {
    let tls_config =
        TlsConfig::from_paths("../tls-certs/server.crt", "../tls-certs/server.pkcs8.key")
            .with_ciphers(CipherSuite::TLS_V13_SET)
            .with_mutual(MutualTls::from_path("../tls-certs/rootCA.crt").mandatory(true));

    let config = Config {
        tls: Some(tls_config),
        ..Default::default()
    };

    rocket::custom(config)
        .manage(JobsState {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            next_job_id: AtomicU64::new(0),
        })
        .mount("/", routes![index, start_job, get_job, get_jobs])
}
