# Design Document for Level 2 - Teleport Challenge

This document aims to deliver a concise technical design proposal for the Level 2 challenge.
Implementation approaches and design choices for the library, server and client implementations are given.

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

create(command, arguments) -> job id (a job object should always be created, errors are captured by its status)  
delete(job id) -> optional error (job should be guaranteed to be deleted on no error response)  
status(job id) -> status or error  
output(job id) -> output or error  

By just returning a job id on creation the creation process is fast and every created job is guaranteed
to be available to be queried later. ....

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

### TLS

The server should only accept TLS 1.3 connections, no support for older versions
is needed, as there are no external clients and the client can also just support TLS 1.3.
All 3 supported cipher suites are considered secure, I'm unsure if there is an advantage
to just supporting one of them (could pick the fastest):

- CHACHA20_POLY1305_SHA256,
- AES_256_GCM_SHA384,
- AES_128_GCM_SHA256

To enable mTLS the following has to be generated:
- root CA private key + certificate
- server private key + certificate
- client private key + certificate (for each distinct client)

For the public/private key algorithm common choices are RSA and ECDSA,
with ECDSA having shorter keys offering the same level of security.
For this project I propose to use ECDSA 256-bit keys (P-256 curve).
SHA-256 should be used as the digital signature algorithm, it's the most common secure choice.