use serde::{Serialize, Deserialize};
#[macro_use]
extern crate log;
use aws_lambda_events::secretsmanager::SecretsManagerSecretRotationEvent;
use lambda_runtime::{run, service_fn, tracing, Error, LambdaEvent};
use aws_config::BehaviorVersion;
use std::collections::HashMap;
use std::fmt;

mod describe;
mod create;
mod passwd;
mod set;
mod value;
mod test;
mod finish;
mod resource;
mod error;
mod config;
mod iam;

use describe::describe_secret;
use create::create_secret;
use set::set_secret;
use test::test_secret;
use finish::finish_secret;
use resource::{ServiceSpecificCredentialResource, Resource};
pub use error::*;
use config::{Config, ResourceType};

pub type RotatorEvent = SecretsManagerSecretRotationEvent;

#[derive(Copy, Clone, Debug, PartialEq, Deserialize)]
pub enum RotatorStep {
    #[serde(rename="createSecret")]
    CreateSecret,
    
    #[serde(rename="setSecret")]
    SetSecret,
    
    #[serde(rename="testSecret")]
    TestSecret,
    
    #[serde(rename="finishSecret")]
    FinishSecret,
}

impl fmt::Display for RotatorStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RotatorStep::CreateSecret => write!(f, "createSecret"),
            RotatorStep::SetSecret => write!(f, "setSecret"),
            RotatorStep::TestSecret => write!(f, "testSecret"),
            RotatorStep::FinishSecret => write!(f, "finishSecret"),
        }
    }
}

impl TryFrom<&str> for RotatorStep {
    type Error = RotatorError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "createSecret" => Ok(RotatorStep::CreateSecret),
            "setSecret" => Ok(RotatorStep::SetSecret),
            "testSecret" => Ok(RotatorStep::TestSecret),
            "finishSecret" => Ok(RotatorStep::FinishSecret),
            _ => Err(RotatorError::Other {
                message: format!("Unknown secret rotation step: {}", value),
            }),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RotatorOutput {
    message: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    tracing::init_default_subscriber();

    run(service_fn(lambda_handler)).await
}

async fn lambda_handler(e: LambdaEvent<SecretsManagerSecretRotationEvent>) -> Result<RotatorOutput, RotatorError> {
    info!("rotator input event={:?}", e);

    rotate(e).await?;
    
    let output = RotatorOutput {
        message: format!("ok"),
    };

    info!("rotator output={:?}", output);

    Ok(output)
}

async fn rotate(e: LambdaEvent<SecretsManagerSecretRotationEvent>) -> RotatorResult<()> {
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let sm_client = aws_sdk_secretsmanager::Client::new(&config);
    let iam_client = aws_sdk_iam::Client::new(&config);

    let mut config = Config::new_for_secret(sm_client, iam_client, &e.payload.secret_id)?;

    let secret = describe_secret(&config, &e.payload.secret_id).await?;
    
    let tags = secret.tags.ok_or_else(|| RotatorError::UnrecoverableSecretProblem { 
            secret_id: e.payload.secret_id.clone(), message: format!("Missing tags") }
    )?;
    config.set_tags(tags)?;
    
    info!("rotating secret event={:?} config={:?}", e, config);

    let resource = match config.resource_type {
        Some(ResourceType::ServiceSpecificCredential) => Box::new(ServiceSpecificCredentialResource::new(&config)?) as Box<dyn Resource>,
        _ => return Err(RotatorError::InvalidConfig {
            message: format!("Unsupported resource type for secret {}: {:?}", e.payload.secret_id, config.resource_type),
        }),
    };

    if !secret.rotation_enabled.unwrap_or(false) {
        error!("Secret {} is not enabled for rotation", e.payload.secret_id);
        return Err(RotatorError::UnrecoverableSecretProblem {
            secret_id: e.payload.secret_id,
            message: format!("Secret is not enabled for rotation")
        });
    }

    let version_ids_to_stages = secret.version_ids_to_stages.unwrap_or(HashMap::new());
    info!("found versions {:?}", version_ids_to_stages);

    let version = version_ids_to_stages.get(&e.payload.client_request_token);
    if version.is_none() {
        error!("Secret version {} has no stage for rotation of secret {}.", e.payload.client_request_token, e.payload.secret_id);
        return Err(RotatorError::UnrecoverableSecretProblem { 
            secret_id: e.payload.secret_id,
            message: format!("Secret version {} has no stage for rotation of secret", e.payload.client_request_token)
        });
    }
    let version = version.unwrap();
    if version.contains(&"AWSCURRENT".to_string()) {
        info!("Secret version {} already set as AWSCURRENT for secret {}.", e.payload.client_request_token, e.payload.secret_id);
        return Ok(());
    }
    if !version.contains(&"AWSPENDING".to_string()) {
        error!("Secret version {} not set as AWSPENDING for rotation of secret {}.", e.payload.client_request_token, e.payload.secret_id);
        return Err(RotatorError::UnrecoverableSecretProblem {
            secret_id: e.payload.secret_id,
            message: format!("Secret version {} not set as AWSPENDING for rotation of secret", e.payload.client_request_token)
        });
    }

    match RotatorStep::try_from(&e.payload.step[..])? {
        RotatorStep::CreateSecret => create_secret(&config, e.payload, resource).await?,
        RotatorStep::SetSecret => set_secret(&config, e.payload).await?,
        RotatorStep::TestSecret => test_secret(&config, e.payload).await?,
        RotatorStep::FinishSecret => finish_secret(&config, e.payload).await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_parse_input() {
        let input1 = r#"
        {
            "SecretId": "secret",
            "ClientRequestToken": "token",
            "Step": "createSecret"
        }
        "#;

        let event1: RotatorEvent = serde_json::from_str(input1).expect("json parse error");

        assert_eq!(event1.secret_id, "secret".to_string());
        assert_eq!(event1.client_request_token, "token".to_string());
        assert_eq!(event1.step, format!("{}", RotatorStep::CreateSecret));
    }
}