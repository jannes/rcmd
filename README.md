# RCMD

Just a simple learning project to understand TLS and Unix processes better.
See e2e.sh for some examples of using the client to execute, query and stop commands.

# Development setup

Required: 
- Rust (tested with stable 1.55) installed through [rustup](https://rustup.rs/)  
- openssl (tested with version 3.0.0)

Tested on Linux/MacOS

## setting up private keys, TLS certificates and root CA

- run `tls-certs/generate.sh`
- for running client/server remote, either of:
  - copy `clientKeyCert.pem` and `rootCA.crt` to tls-certs folder on client
  - copy `server.key`, `server.crt` and `rootCA.crt` to tls-certs folder on server
  - add rcmd-server host entry to etc/hosts with IP of the machine running server binary

## Running client/server

Run server: `cargo run -p rcmd_server tls-certs`  
Run client on same machine: `cargo run -p rcmd_client tls-certs localhost <operation>`  
Run client on different machine: `cargo run -p rcmd_client tls-certs rcmd-server <operation>`

where `<operation` is one of:
- `exec <command> <arg1> <arg2> ...`
- `list`
- `status <job_id>`
- `output <job_id>`
- `delete <job_id>`

## Running tests

Library unit tests:
```
cargo test -p rcmd_lib
```
or with logging output:
```
RUST_LOG=INFO cargo test -p rcmd_lib
RUST_LOG=DEBUG cargo test -p rcmd_lib
```

End-to-end test for local client/server:
```
./e2e.sh
```

## Codestyle

Code is formatted with `cargo fmt` and linted with `cargo clippy` to adhere to a standard style.

# Possible improvements

The job pool should implement the Drop trait to do a more proper cleanup;
it should be ensured that all processes are terminated by sending a terminate signal
to the management tasks and waiting for their completions 
(the JoinHandles could be saved for each job and in the drop function all of them can be joined).

Currently management tasks will be triggered (by closed channels) to exit 
as a result of the job pool being dropped,
but they most likely don't finish in time if the whole process exits right away.

Error handling can also be improved, different error states could be captured
by different job state error types.