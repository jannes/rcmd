use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc, Mutex},
};

use rcmd_shared::{Job, JobCreatedResponse, CommandSpec};
use rocket::{serde::json::Json, State};

use crate::state::JobsState;

#[macro_use]
extern crate rocket;

mod state;

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[post("/jobs", format = "json", data = "<command>")]
async fn start_job(command: Json<CommandSpec>, jobs_state: &State<JobsState>) -> Json<JobCreatedResponse> {
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
    rocket::build()
        .manage(JobsState {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            next_job_id: AtomicU64::new(0),
        })
        .mount("/", routes![index, start_job, get_job, get_jobs])
}
