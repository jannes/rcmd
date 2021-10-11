use std::{
    fs,
    path::{Path, PathBuf},
};

use structopt::StructOpt;

use crate::operations::{delete, list, output, status, submit};

mod operations;

const CA_CERT_NAME: &str = "rootCA.crt";
const CLIENT_IDENTITY_NAME: &str = "clientKeyCert.pem";

#[derive(StructOpt, Debug)]
#[structopt(name = "rcmd-client")]
struct Opt {
    #[structopt(name = "CERTIFICATES_DIRECTORY", parse(from_os_str))]
    certs_dir: PathBuf,

    #[structopt(name = "REMOTE_URL")]
    remote_url: String,

    #[structopt(subcommand)]
    operation: Operation,
}

#[derive(Debug, StructOpt)]
enum Operation {
    Exec {
        #[structopt(name = "COMMAND")]
        command: String,
        #[structopt(name = "ARGS")]
        args: Vec<String>,
    },
    List,
    Status {
        #[structopt(name = "JOB_ID")]
        id: u64,
    },
    Output {
        #[structopt(name = "JOB_ID")]
        id: u64,
    },
    Delete {
        #[structopt(name = "JOB_ID")]
        id: u64,
    },
}

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

    let output = match opt.operation {
        Operation::Exec { command, args } => {
            let args: Vec<&str> = args.iter().map(AsRef::as_ref).collect();
            submit(&client, opt.remote_url, &command, &args)
        }
        Operation::List => list(&client, opt.remote_url),
        Operation::Status { id } => status(&client, opt.remote_url, id),
        Operation::Output { id } => output(&client, opt.remote_url, id),
        Operation::Delete { id } => delete(&client, opt.remote_url, id),
    };

    println!("{}", output);
}
