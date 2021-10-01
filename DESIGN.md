# Design Document for Level 2 - Teleport Challenge

This document aims to deliver a concise technical design proposal for the Level 2 challenge.
Implementation approaches and design choices for the library, server (HTTP API) 
and client (CLI) implementations are given.

## Library

The library needs to support creation, deletion and querying status info and output of jobs.
A job is defined by a command and arguments to be executed as a process, with the output being the process's stdout/stderr.

### Interface and semantics

The library should support creation of a job execution pool, which serves as the single abstraction
that is used to create, delete and query jobs. 
The job pool is responsible for keeping and updating the states of the created jobs.

The following is proposed as the interface for the job pool (which must be thread-safe):

- `create(command, arguments) -> job id`  
  (a job object should always be created, errors are captured by its status)  
- `delete(job id) -> optional error`  
  (job should be guaranteed to be deleted on no error response)  
- `status(job id) -> status or error`  
- `output(job id) -> output or error`  

The `create` command could also have different behavior, e.g only return a job id if the process was
created successfully, but with the proposed behavior the creation process is fast 
and every created job is guaranteed to be available to be queried later. 
All error cases are represented with the `status`/`output` error responses.

The job status should reflect the job's state, which is one of
- running
- error: error message
- completed: exit status

### Implementation approach

There are two approaches for the behavior and implied implementation for the job pool.
- Lazy evaluation of job status/output
  (when status/output functions are called, in caller's thread of execution)
- Eager evaluation of job status/output
  (when creating a pool a manager thread is created that continously updates 
  running job's statuses and outputs)

The job's processes themselves are of course running on their own, but their status/output
data needs to be proactively queried by the process using the library.
The lazy evaluation model has lower complexity, the eager evaluation model could potentially
reduce the status/output functions' latency (depending on how the data is synchronized).
Considering the scope of this challenge I'd opt for the lazy evaluation approach.

As all operations on the job pool must be thread-safe, the internal collection of jobs
must be synchronized. As a single job pool should be used by a single client and
high performance is not a requirement, I propose to just use a single lock to protect the
whole collection of jobs. This makes the code less complex at the cost of limiting clients
having their commands being submitted/queried in parallel.

## Server

The server should expose the library's operations through a HTTPS API secured with mTLS.
Each distinct client should have a access only to the created jobs by itself.

### API

#### Create
endpoint: `PUT /jobs`  
request body: 
```
{
    "cmd": command_name
    "args": [arg1, arg2, ...]
}
```
response:  
```
{ 
    "id": <id> 
}
```

### Delete
endpoint: `DELETE /jobs/<id>`  
no request body  
success response: 200, no body  
error response: 
some error code
```
{
    "error": <message describing error> (e.g command doesn't exist)
}
```

#### Status
endpoint: `GET /jobs/<id>/status`  
no request body  
success response:  
```
{ 
    "status": "running"/"error"/"completed"
    "error_msg": "..." (optional, set on error)
    "exit_code": ... (optional, set on completed)
}
```
error response: 404, no body

#### Output
endpoint: `GET /jobs/<id>/output`  
no request body  
success response:  
```
{ 
    "stdout": "..." (empty for jobs in error state)
    "stderr": "..." (empty for jobs in error state)
}
```
error response: 404, no body

### Implementation approach

The server makes use of the library to create job pools for each client on their
first job creation request. The collection of job pools must be synchronized as
multiple clients may be calling the API concurrently.
Since all client requests after the first one do not cause the creation of a new job
pool, I propose to synchronize the collection of job pools with the use of a single read-write lock.
Using a read-write lock allows for multiple clients to make requests concurrently,
as long as no single new client issues a first request. 
This approach is again very simple at the cost of not being scalable for a large number of clients.
However, for the scope of this project I believe it is good enough.
To achieve higher scalability a more sophisticated thread-safe collection should be used 
(the same holds for the job pool's internal job collection).

### Auth & TLS

Clients should be authenticated with client TLS certificates.
Certificates with different common names are treated as distinct clients.

The server should only accept TLS 1.3 connections, no support for older versions
is needed, as there are no external clients and the client can also just support TLS 1.3.
All [3 supported cipher suites](https://datatracker.ietf.org/doc/html/rfc8446#section-9.1) 
are considered secure, 
depending on hardware support ChaCha20Poly1305 or AES-GCM are faster 
(see [Go blog](https://go.dev/blog/tls-cipher-suites)).
So for the scope of this challenge I'd say to just support all of them and let the cipher
be picked based on the defaults by the used client/server http/tls libraries.
If optimizing for best performance, only TLS_CHACHA20_POLY1305_SHA256 and
TLS_AES_128_GCM_SHA256 should supported and possibly one of them only.

To enable mTLS the following has to be generated:
- root CA private key + self-signed certificate
- server private key + CA-signed certificate 
- client private key + CA-signed certificate (for each distinct client)

For the public/private key algorithm common choices are RSA and ECDSA,
with ECDSA having shorter keys offering the same level of security.
According to Mozilla's [recommended highest security configuration](https://wiki.mozilla.org/Security/Server_Side_TLS#Modern_compatibility)
ECDSA 256-bit keys (P-256 curve) and the SHA-256 digital signature algorithm should be used.

## Client

The client should allow connecting to remote machines that run the server binary 
and it should expose the API calls as CLI commands.
A client identifies itself with a TLS certificate + private key 
and it needs to know the root CA that signed the server's TLS certificate.
These should all be passed as arguments for simplicity (as opposed to configuring
some location on the local system).

Examples:
```
./rcmd certificate-directory my-server:8080 exec echo hi
$ 1
./rcmd certificate-directory my-server:8080 status 1
$ Completed, exit status: 0
./rcmd certificate-directory my-server:8080 output 1
$ hi
```
