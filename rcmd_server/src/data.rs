use rocket::serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct JobOutput {
    stdout: String,
    stderr: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct JobStatus { }

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct JobSpec { }