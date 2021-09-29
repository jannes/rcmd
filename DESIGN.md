# Design Document for Level 2 - Teleport Challenge

This document aims to deliver a concise technical design proposal for the Level 2 challenge.
Implementation approaches and design choices for the library, server (HTTP API) 
and client (CLI) implementations are given.

## Library

The library needs to support creation, deletion and querying status info and output of jobs.
A job is defined by a command and arguments to be executed as a process, with the output being the process's stdout/stderr.

### Job status

- state, one of:
    - running
    - error: error message
    - completed: exit status

### Job output

- stdout
- stderr

### Implementation approach

The library should support creation of a job execution pool, which serves as the single abstraction
that is used to create, delete and query jobs. 
The job pool is responsible for keeping and updating the states of the created jobs.

The following is proposed as the interface for the job pool:

- `create(command, arguments) -> job id`  
  (a job object should always be created, errors are captured by its status)  
- `delete(job id) -> optional error`  
  (job should be guaranteed to be deleted on no error response)  
- `status(job id) -> status or error`  
- `output(job id) -> output or error`  

By just returning a job id on creation the creation process is fast and every created job is guaranteed
to be available to be queried later. ....

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

### Client
