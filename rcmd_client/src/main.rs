use std::{
    fs,
    path::{Path, PathBuf},
};

use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "rcmd-client")]
struct Opt {
    #[structopt(name = "CERTIFICATES DIRECTORY", parse(from_os_str))]
    certs_dir: PathBuf,

    #[structopt(name = "REMOTE_URL:PORT")]
    remote_url_port: String,

    #[structopt(subcommand)]
    operation: Operation,
}

#[derive(Debug, StructOpt)]
enum Operation {
    Exec(ExecuteOperation),
    Show(ShowOperation),
    List(ListOperation),
}

#[derive(Debug, StructOpt)]
struct ExecuteOperation {
    #[structopt(name = "COMMAND")]
    command: String,
    #[structopt(name = "ARGS")]
    args: Vec<String>,
}

#[derive(Debug, StructOpt)]
struct ShowOperation {}

#[derive(Debug, StructOpt)]
struct ListOperation {}

const CA_CERT_NAME: &str = "rootCA.crt";
const CLIENT_IDENTITY_NAME: &str = "clientKeyCert.pem";

fn main() {
    let opt = Opt::from_args();
    // println!("{:#?}", opt);
    let ca_cert_path: PathBuf = [opt.certs_dir.as_path(), Path::new(CA_CERT_NAME)]
        .iter()
        .collect();
    let client_cert_path: PathBuf = [opt.certs_dir.as_path(), Path::new(CLIENT_IDENTITY_NAME)]
        .iter()
        .collect();
    let ca_cert = fs::read(ca_cert_path).expect("could not find CA certificate");
    let ca_cert =
        reqwest::Certificate::from_pem(&ca_cert).expect("could not read CA certificate as PEM");
    let client_identity = fs::read(client_cert_path).expect("could not find client certificate");
    let client_identity =
        reqwest::Identity::from_pem(&client_identity).expect("could not read client key/cert");

    let client = reqwest::blocking::Client::builder()
        .add_root_certificate(ca_cert)
        .identity(client_identity)
        .use_rustls_tls()
        .build()
        .expect("could not build http client");
    let request = client
        .get(opt.remote_url_port)
        .build()
        .expect("unexpected error building the request");
    match client.execute(request) {
        Ok(response) => println!("{}", response.text().unwrap()),
        Err(e) => println!("{:?}", e),
    }
}
