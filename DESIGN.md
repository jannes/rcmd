# Design Document for Level 2 - Teleport Challenge

This document aims to deliver a concise technical design proposal for the Level 2 challenge.
Specifications of interfaces, implementation approaches and design choices for 
the library, server (HTTP API) and client (CLI) components are given.

## Library

The library needs to support creation, deletion and querying status info and output of jobs.
A job is defined by a command and arguments to be executed as a (OS) process.

### Interface and semantics

The library should support creation of a job pool, which serves as the single abstraction
that is used to create, delete and query jobs and allows for easy namespacing of jobs (e.g for different clients). 
The job pool is responsible for keeping and updating the states of the created jobs.

The following is proposed as the interface for the job pool (which must be thread-safe):

- `create(command, arguments) -> job id`  
  (create job, attempt spawning process, errors must be checked for with `status`)
- `list() -> list of job ids`  
  (list all jobs that have been created and not deleted yet)
- `delete(job id) -> optional error`  
  (job is guaranteed to be deleted (and its process terminated) on non-error response)  
- `status(job id) -> status or error`  
  (for existing job, returns its status, see below for status info)
- `output(job id) -> output or error`  
  (for existing job, returns its output: tuple of current contents of stdout/stderr)

The `create` command could also have different behavior, 
e.g only return a job id if the process was created successfully, 
but with the proposed behavior the creation command is fast 
and every created job is guaranteed to be available to be queried later. 
All error cases are represented by the job's status.

A job's `status` should reflect the job's state, which is one of
- *running* (process spawned, not exited yet)
- *error*: error message  
  (e.g command doesn't exist, invalid arguments, process could not be spawned due to OS limits etc.)
- *completed*: exit status (process exited) 
- *terminated* (process terminated with e.g SIGTERM, SIGKILL etc.)

The `delete` command should terminate the process 
(using SIGKILL, as that guarantees the process' termination)
if in running state and remove the job from the job pool.
With a bigger scope I would improve the API by splitting termination of a job and
removal from the job pool into different commands, with the termination command
supporting different kinds of termination signal (e.g SIGINT, SIGTERM etc.).

### Implementation approach

There are at least two approaches for the behavior of the job pool w.r.t query operations.
- Lazy evaluation of job status/output
  (when status/output functions are called, in caller's thread of execution)
- Eager evaluation of job status/output
  (when creating a job pool, a manager thread is created that continously updates 
  running job's statuses and outputs)

The jobs' processes themselves are of course running on their own, but their status/output
data needs to be proactively queried by the process using the library.
The lazy evaluation model has lower complexity, the eager evaluation model could potentially
reduce the status/output functions' latency (depending on how the data is synchronized).
Considering the scope of this challenge, I'd opt for the lazy evaluation approach.

As all operations on the job pool must be thread-safe, the internal collection of jobs 
(mapping of job id -> job) must be synchronized. 
As a single job pool should be used by a single client and high performance is not a requirement, 
I propose to just use a single lock to protect the whole collection of jobs. 
This makes the code less complex at the cost of making concurrent calls non-parallel; 
a single job pool less scalable.

## Server

The server should expose the library's operations through a HTTPS API secured with mTLS.
Each distinct client should have access only to the created jobs by itself.

### API

#### Create
`PUT /jobs`  
request body: 
```
{
    "cmd": command_name
    "args": [arg1, arg2, ...]
}
```
response: 201, with body:  
```
{ 
    "id": <id> 
}
```

#### List
`GET /jobs`  
response: 200, with body:  
```
{ 
    "ids": [<id1>, <id2>, ...]
}
```

#### Delete
`DELETE /jobs/<id>`  
success response: 200  
error response: 404 when job doesn't exist

#### Status
`GET /jobs/<id>/status`  
success response: 200, with body:  
```
{ 
    "status": "running"/"error"/"completed/terminated"
    "error_msg": "..." (optional, set on error)
    "exit_code": ... (optional, set on completed)
}
```
error response: 404 when job doesn't exist

#### Output
`GET /jobs/<id>/output`  
success response: 200, with body:  
```
{ 
    "stdout": "..." (empty for jobs in error state)
    "stderr": "..." (empty for jobs in error state)
}
```
error response: 404 when job doesn't exist

### Implementation approach

The server makes use of the library to create job pools for each client on their
first job creation request. The collection of job pools (mapping of client -> job pool)
must be synchronized as multiple clients may be calling the API concurrently.
Since all client requests after the first one do not cause the creation of a new job pool, 
I propose to synchronize the collection of job pools with the use of a single read-write lock.
Using a read-write lock allows for multiple clients' requests to be executed concurrently,
as long as no single new client issues a first request. 
This approach is again very simple at the cost of not being scalable for a large number of clients.
To achieve higher scalability, a more sophisticated thread-safe collection would have to be used 
(the same holds for the job pool's internal job collection).

### Security - TLS/Auth

Clients should be authenticated with client TLS certificates.
Certificates with different common names are treated as distinct clients.

The server should only accept TLS 1.3 connections, no support for older versions
is needed, as there are no external clients and the client can also just support TLS 1.3.
All [3 supported cipher suites](https://datatracker.ietf.org/doc/html/rfc8446#section-9.1) 
are considered secure, depending on hardware support ChaCha20Poly1305 or AES-GCM are faster 
(see [Go blog](https://go.dev/blog/tls-cipher-suites)).
So for the scope of this challenge I'd just support all of them and let the cipher
be picked based on the defaults by the used client/server http/tls libraries.
If optimizing for best performance, only TLS_CHACHA20_POLY1305_SHA256 and
TLS_AES_128_GCM_SHA256 (the faster one of the two AWS-GCM versions) should be supported,
with the order determined by the expected hardware support of clients/servers.

To enable mTLS the following has to be generated:
- root CA private key + self-signed certificate
- server private key + CA-signed certificate 
- client private key + CA-signed certificate (for each distinct client)

For the public/private key algorithm common choices are RSA and ECDSA,
with ECDSA having shorter keys offering the same level of security.
According to Mozilla's 
[recommended highest security configuration](https://wiki.mozilla.org/Security/Server_Side_TLS#Modern_compatibility)
ECDSA 256-bit keys (P-256 curve) and the SHA-256 digital signature algorithm should be used.
The server must be configured to present its certificate and accept certificates signed by the same root CA.

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
./rcmd certificate-directory my-server:8080 delete 1
$ job 1 deleted
./rcmd certificate-directory my-server:8080 list
$ 
```
