use std::collections::HashMap;

use rcmd_data::{JobOutput, JobSpec, JobStatus};
use reqwest::blocking::{Client, Response};

const JOB_NOT_FOUND_MSG: &str = "Job not found";

pub fn submit(http_client: &Client, url: String, command: &str, args: &[&str]) -> String {
    let job_spec = JobSpec::new(command, args);
    let request = http_client
        .post(format!("https://{}:8000/jobs", &url))
        .json(&job_spec)
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => response.text().unwrap(),
        Ok(response) => unexpected_response_msg(response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn list(http_client: &Client, url: String) -> String {
    let request = http_client
        .get(format!("https://{}:8000/jobs", &url))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let jobs: HashMap<u64, JobSpec> = response.json().unwrap();
            let mut lines: Vec<String> = jobs
                .iter()
                .map(|(id, spec)| format!("{}: {} {}", id, spec.command, spec.arguments.join(" ")))
                .collect();
            lines.sort();
            lines.join("\n")
        }
        Ok(response) => unexpected_response_msg(response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn status(http_client: &Client, url: String, job_id: u64) -> String {
    let request = http_client
        .get(format!("https://{}:8000/jobs/{}/status", &url, job_id))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let status: JobStatus = response.json().unwrap();
            format!("{:?}", status)
        }
        Ok(response) if response.status().as_u16() == 404 => JOB_NOT_FOUND_MSG.to_string(),
        Ok(response) => unexpected_response_msg(response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn output(http_client: &Client, url: String, job_id: u64) -> String {
    let request = http_client
        .get(format!("https://{}:8000/jobs/{}/output", &url, job_id))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let output: JobOutput = response.json().unwrap();
            format!(
                "___STDOUT___\n{}\n___STDERR___\n{}",
                output.stdout(),
                output.stderr()
            )
        }
        Ok(response) if response.status().as_u16() == 404 => JOB_NOT_FOUND_MSG.to_string(),
        Ok(response) => unexpected_response_msg(response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn delete(http_client: &Client, url: String, job_id: u64) -> String {
    let request = http_client
        .delete(format!("https://{}:8000/jobs/{}", &url, job_id))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            format!("{} deleted", job_id)
        }
        Ok(response) if response.status().as_u16() == 404 => JOB_NOT_FOUND_MSG.to_string(),
        Ok(response) => unexpected_response_msg(response),
        Err(e) => format!("error executing request: {}", e),
    }
}

fn unexpected_response_msg(response: Response) -> String {
    format!(
        "unexpected response (status {}): {:?}",
        response.status().as_u16(),
        response.text()
    )
}
