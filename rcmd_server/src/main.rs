use std::{collections::HashMap, env};

use rcmd_lib::job_pool::{JobOutput, JobSpec, JobStatus};
use rocket::{
    config::{CipherSuite, MutualTls, TlsConfig},
    http::Status,
    response::status,
    serde::json::Json,
    Config,
};
use state::JobPools;

use crate::auth::ClientJobPool;

#[macro_use]
extern crate rocket;

mod auth;
mod state;

#[get("/")]
fn index(client: ClientJobPool) -> String {
    format!("Hello, {}!", client.client.name)
}

#[post("/jobs", format = "json", data = "<job_spec>")]
async fn start_job(client_job_pool: ClientJobPool, job_spec: Json<JobSpec>) -> Json<u64> {
    let command = &job_spec.command;
    let args: Vec<&str> = job_spec.arguments.iter().map(|arg| arg.as_str()).collect();
    Json(client_job_pool.job_pool.submit(command, &args).await)
}

#[get("/jobs")]
async fn get_jobs(client_job_pool: ClientJobPool) -> Json<HashMap<u64, JobSpec>> {
    Json(client_job_pool.job_pool.list().await)
}

#[get("/jobs/<id>/status")]
async fn get_status(client_job_pool: ClientJobPool, id: u64) -> Option<Json<JobStatus>> {
    client_job_pool.job_pool.status(id).await.map(Json)
}

#[get("/jobs/<id>/output")]
async fn get_output(client_job_pool: ClientJobPool, id: u64) -> Option<Json<JobOutput>> {
    client_job_pool.job_pool.output(id).await.map(Json)
}

#[delete("/jobs/<id>")]
async fn delete_job(
    client_job_pool: ClientJobPool,
    id: u64,
) -> Option<Result<(), status::Custom<String>>> {
    match client_job_pool.job_pool.delete(id).await {
        Some(Ok(_)) => Some(Ok(())),
        Some(Err(err)) => Some(Err(status::Custom(Status::InternalServerError, err))),
        None => None,
    }
}

#[launch]
fn rocket() -> _ {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        panic!("required argument: path to tls certs without trailing slash");
    }
    let tls_dir = args.get(1).unwrap();

    let tls_config = TlsConfig::from_paths(
        format!("{}/server.crt", tls_dir),
        format!("{}/server.pkcs8.key", tls_dir),
    )
    .with_ciphers(CipherSuite::TLS_V13_SET)
    .with_mutual(MutualTls::from_path(format!("{}/rootCA.crt", tls_dir)).mandatory(true));

    let config = Config {
        tls: Some(tls_config),
        ..Default::default()
    };

    rocket::custom(config).manage(JobPools::new()).mount(
        "/",
        routes![index, start_job, get_jobs, get_status, get_output, delete_job],
    )
}
