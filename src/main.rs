use serde::{Serialize, Deserialize};
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
use lambda_runtime::{Context, lambda};
use lambda_runtime::error::LambdaResultExt;
use failure::{Compat, Error};
use rusoto_core::Region;
use rusoto_iam::IamClient;
use rusoto_secretsmanager::SecretsManagerClient;
use std::time::Duration;
use std::collections::HashMap;

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

lazy_static! {
    pub static ref SM_CLIENT: SecretsManagerClient = SecretsManagerClient::new(Region::default());
    pub static ref IAM_CLIENT: IamClient = IamClient::new(Region::UsEast1);
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RotatorEvent {
    #[serde(rename="SecretId")]
    secret_id: String,
    
    #[serde(rename="ClientRequestToken")]
    client_request_token: String,

    #[serde(rename="Step")]
    step: RotatorStep,
}

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

#[derive(Clone, Debug, Serialize)]
pub struct RotatorOutput {
    message: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    lambda!(lambda_handler);

    Ok(())
}

fn lambda_handler(e: RotatorEvent, _c: Context) -> Result<RotatorOutput, Compat<Error>> {
    info!("rotator input event={:?}", e);

    rotate(e).failure_compat()?;
    
    let output = RotatorOutput {
        message: format!("ok"),
    };

    info!("rotator output={:?}", output);

    Ok(output)
}

fn rotate(e: RotatorEvent) -> RotatorResult<()> {
    let timeout = Duration::from_secs(2);

    let secret = describe_secret(&e.secret_id, timeout)?;
    
    let config = Config::new_for_secret(&e.secret_id, secret.tags)?;
    info!("rotating secret event={:?} config={:?}", e, config);

    let resource = match config.resource_type {
        ResourceType::ServiceSpecificCredential => Box::new(ServiceSpecificCredentialResource::new(&config)?) as Box<dyn Resource>,
    };

    if !secret.rotation_enabled.unwrap_or(false) {
        error!("Secret {} is not enabled for rotation", e.secret_id);
        return Err(RotatorError::RotationNotEnabled { secret_id: e.secret_id });
    }

    let version_ids_to_stages = secret.version_ids_to_stages.unwrap_or(HashMap::new());
    info!("found versions {:?}", version_ids_to_stages);

    let version = version_ids_to_stages.get(&e.client_request_token);
    if version.is_none() {
        error!("Secret version {} has no stage for rotation of secret {}.", e.client_request_token, e.secret_id);
        return Err(RotatorError::NoStageForRotation { secret_id: e.secret_id, version: e.client_request_token });
    }
    let version = version.unwrap();
    if version.contains(&"AWSCURRENT".to_string()) {
        info!("Secret version {} already set as AWSCURRENT for secret {}.", e.client_request_token, e.secret_id);
        return Ok(());
    }
    if !version.contains(&"AWSPENDING".to_string()) {
        error!("Secret version {} not set as AWSPENDING for rotation of secret {}.", e.client_request_token, e.secret_id);
        return Err(RotatorError::NotSetAsPending { secret_id: e.secret_id, version: e.client_request_token });
    }

    match e.step {
        RotatorStep::CreateSecret => create_secret(e, resource)?,
        RotatorStep::SetSecret => set_secret(e)?,
        RotatorStep::TestSecret => test_secret(e)?,
        RotatorStep::FinishSecret => finish_secret(e)?,
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
        assert_eq!(event1.step, RotatorStep::CreateSecret);
    }
}