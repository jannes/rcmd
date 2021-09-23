use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "rcmd-client")]
struct Opt {
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

fn main() {
    let opt = Opt::from_args();
    println!("{:#?}", opt);

    let client = reqwest::blocking::Client::new();
    let request = client
        .get(opt.remote_url_port)
        .build()
        .expect("unexpected error building the request");
    match client.execute(request) {
        Ok(response) => println!("{:?}", response),
        Err(e) => println!("{:?}", e),
    }
}
