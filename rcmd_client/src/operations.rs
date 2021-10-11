use std::collections::HashMap;

use rcmd_data::{JobOutput, JobSpec, JobStatus};
use reqwest::blocking::Client;

pub fn submit(http_client: &Client, url: String, command: &str, args: &[&str]) -> String {
    let job_spec = JobSpec::new(command, args);
    let request = http_client
        .post(format!("{}:8000/jobs", &url))
        .json(&job_spec)
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let job_id = response.text().unwrap();
            job_id
        }
        Ok(response) => format!("unexpected response: {:?}", response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn list(http_client: &Client, url: String) -> String {
    let request = http_client
        .get(format!("{}:8000/jobs", &url))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let jobs: HashMap<u64, JobSpec> = response.json().unwrap();
            let lines: Vec<String> = jobs
                .iter()
                .map(|(id, spec)| format!("{}: {} {:#?}", id, spec.command, spec.arguments))
                .collect();
            lines.join("\n")
        }
        Ok(response) => format!("unexpected response: {:?}", response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn status(http_client: &Client, url: String, job_id: u64) -> String {
    let request = http_client
        .get(format!("{}:8000/jobs/{}/status", &url, job_id))
        .build()
        .expect("unexpected error building the request");

    match http_client.execute(request) {
        Ok(response) if response.status().is_success() => {
            let status: JobStatus = response.json().unwrap();
            format!("{:?}", status)
        }
        Ok(response) if response.status().as_u16() == 404 => "Job not found".to_string(),
        Ok(response) => format!("unexpected response: {:?}", response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn output(http_client: &Client, url: String, job_id: u64) -> String {
    let request = http_client
        .get(format!("{}:8000/jobs/{}/output", &url, job_id))
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
        Ok(response) if response.status().as_u16() == 404 => "Job not found".to_string(),
        Ok(response) => format!("unexpected response: {:?}", response),
        Err(e) => format!("error executing request: {}", e),
    }
}

pub fn delete(http_client: &Client, url: String, job_id: u64) -> String {
    todo!()
}
