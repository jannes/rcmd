use std::collections::HashMap;

use rcmd_lib::job::{JobOutput, JobSpec, JobStatus};
use rocket::{
    config::{CipherSuite, MutualTls, TlsConfig},
    serde::json::Json,
    Config, State,
};
use state::JobPools;

use crate::client::ClientJobPool;

#[macro_use]
extern crate rocket;

mod client;
mod data;
mod state;

#[get("/")]
fn index(client: ClientJobPool) -> String {
    format!("Hello, {}!", client.client.name)
}

#[post("/jobs", format = "json", data = "<command>")]
async fn start_job(
    client_job_pool: ClientJobPool,
    command: Json<data::JobSpec>,
    jobs_state: &State<JobPools>,
) -> Json<u64> {
    todo!()
}

#[get("/jobs")]
async fn get_jobs(
    client_job_pool: ClientJobPool,
    jobs_state: &State<JobPools>,
) -> Json<HashMap<u64, data::JobSpec>> {
    todo!()
    // Json(jobs_state.jobs.lock().unwrap().values().cloned().collect())
}

#[get("/jobs/<id>/status")]
async fn get_status(
    client_job_pool: ClientJobPool,
    id: u64,
    jobs_state: &State<JobPools>,
) -> Option<Json<data::JobStatus>> {
    todo!()
}

#[get("/jobs/<id>/output")]
async fn get_output(
    client_job_pool: ClientJobPool,
    id: u64,
    jobs_state: &State<JobPools>,
) -> Option<Json<data::JobOutput>> {
    todo!()
}

#[delete("/jobs/<id>")]
async fn remove_job(
    client_job_pool: ClientJobPool,
    id: u64,
    jobs_state: &State<JobPools>,
) -> Option<String> {
    todo!()
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

    rocket::custom(config).manage(JobPools::new()).mount(
        "/",
        routes![index, start_job, get_jobs, get_status, get_output, remove_job],
    )
}
