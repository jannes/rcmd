use std::sync::Arc;

use rcmd_lib::job_pool::JobPool;
use rocket::{
    http::Status,
    mtls::{self, Certificate},
    outcome::try_outcome,
    request::{FromRequest, Outcome},
    Request,
};

use crate::state::JobPools;

pub struct Client {
    pub name: String,
}

impl Client {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[derive(Debug)]
pub enum ClientVerificationError {
    CertificateError(mtls::Error),
    MissingCommonName,
}

impl From<mtls::Error> for ClientVerificationError {
    fn from(e: mtls::Error) -> Self {
        Self::CertificateError(e)
    }
}

#[async_trait]
impl<'r> FromRequest<'r> for Client {
    type Error = ClientVerificationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let client_cert = try_outcome!(request
            .guard::<Certificate>()
            .await
            .map_failure(|(status, err)| (status, ClientVerificationError::from(err))));
        if client_cert.subject().common_name().is_none() {
            Outcome::Failure((
                Status::Unauthorized,
                ClientVerificationError::MissingCommonName,
            ))
        } else {
            Outcome::Success(Client::new(
                client_cert.subject().common_name().unwrap().to_string(),
            ))
        }
    }
}

pub struct ClientJobPool {
    pub client: Client,
    pub job_pool: Arc<JobPool>,
}

#[async_trait]
impl<'r> FromRequest<'r> for ClientJobPool {
    type Error = ClientVerificationError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let client = try_outcome!(request.guard::<Client>().await);
        // TODO: don't unrwap
        let job_pools = request.rocket().state::<JobPools>().unwrap();
        let job_pool = if job_pools.has_pool(&client.name) {
            job_pools.get_pool(&client.name).unwrap()
        } else {
            job_pools.create_pool(&client.name)
        };
        Outcome::Success(ClientJobPool { client, job_pool })
    }
}
