use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use aws_sdk_secretsmanager::operation::get_secret_value::{GetSecretValueOutput, GetSecretValueError};
use aws_sdk_secretsmanager::operation::put_secret_value::{PutSecretValueError, PutSecretValueOutput};
use crate::{
    RotatorError,
    RotatorResult,
};
use crate::config::Config;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Secret {
    pub username: Option<String>,
    pub password: Option<String>,

    // for service specific credentials:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_specific_credential_id: Option<String>,

    // capture unknown fields for future proofing and interoperability
    #[serde(flatten)]
    pub attributes: HashMap<String, Value>,
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut s = self.clone();

        if s.password.is_some() {
            s.password = Some("******".to_string());
        }

        write!(f, "{}", serde_json::to_string(&s).map_err(|err| {
            error!("error displaying secret details: {:?}", err);
            fmt::Error
        })?)
    }
}

#[derive(Clone)]
pub struct SecretValue {
    pub secret: Option<Secret>,
}

pub async fn get_secret_value(config: &Config, secret_id: &str, version_stage: Option<&str>, version_id: Option<&str>) -> RotatorResult<SecretValue> {
    info!("fetching secret for secret_id={} version_stage={:?} version_id={:?}", secret_id, version_stage, version_id);

    let value = get_secret_value_string(config, secret_id, version_stage, version_id).await?;

    let secret = if let Some(ref string_value) = value.secret_string {
        serde_json::from_str(&string_value)
            .map_err(|err| RotatorError::UnrecoverableSecretProblem {
                secret_id: secret_id.to_string(),
                message: format!("secret string value deserialisation error: {:?}", err),
            })?
    } else {
        None
    };

    info!("found secret {:?}", secret);

    Ok(SecretValue {
        secret: secret,
    })
}

pub async fn get_secret_value_string(config: &Config, secret_id: &str, version_stage: Option<&str>, version_id: Option<&str>) -> RotatorResult<GetSecretValueOutput> {
    let mut builder = config.sm_client.get_secret_value()
        .secret_id(secret_id.to_string());

    if let Some(stage) = version_stage {
        builder = builder.version_stage(stage.to_string());
    }

    if let Some(version_id) = version_id {
        builder = builder.version_id(version_id.to_string());
    }

    builder
        .send()
        .await
        .map_err(|e| match e.into_service_error() {
            GetSecretValueError::ResourceNotFoundException(_) => {
                RotatorError::SecretValueNotFound {
                    secret_id: secret_id.to_string(),
                    version_stage: version_stage.map(|s| s.to_string()),
                    version_id: version_id.map(|s| s.to_string()),
                }
            },
            e => RotatorError::Other {
                message: format!("{:?}", e)
            }
        })
}

pub async fn put_secret_value(config: &Config, secret_id: &str, token: &str, secret: &Secret, version_stage: &str) -> RotatorResult<PutSecretValueOutput> {
    info!("putting secret for secret_id={} version_stage={:?} version_id={:?} secret={:?}", secret_id, version_stage, token, secret);

    let secret_string = serde_json::to_string(secret)
        .map_err(|err| RotatorError::UnrecoverableSecretProblem {
            secret_id: secret_id.to_string(),
            message: format!("{:?}", err),
        })?;
    
    put_secret_value_string(config, secret_id, token, &secret_string, version_stage).await
}

pub async fn put_secret_value_string(config: &Config, secret_id: &str, token: &str, secret_string: &str, version_stage: &str) -> RotatorResult<PutSecretValueOutput> {
    config.sm_client.put_secret_value()
        .secret_id(secret_id)
        .client_request_token(token)
        .secret_string(secret_string)
        .version_stages(version_stage)
        .send()
        .await
        .map_err(|e| match e.into_service_error() {
            PutSecretValueError::ResourceNotFoundException(_) => {
                RotatorError::UnrecoverableSecretProblem {
                    secret_id: secret_id.to_string(),
                    message: format!("Secret not found with version_stage {:?} and version_ids {:?}", version_stage, token),
                }
            },
            PutSecretValueError::EncryptionFailure(_) => {
                RotatorError::UnrecoverableSecretProblem {
                    secret_id: secret_id.to_string(),
                    message: format!("Encryption failure")
                }
            },
            e => RotatorError::Other {
                message: format!("{:?}", e)
            }
        })
}