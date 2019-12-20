use rusoto_core::RusotoError;
use rusoto_secretsmanager::{
    SecretsManager,
    GetSecretValueRequest,
    GetSecretValueResponse,
    GetSecretValueError,
    PutSecretValueRequest,
    PutSecretValueResponse,
    PutSecretValueError,
};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::time::Duration;
use std::collections::HashMap;
use std::fmt;
use crate::{
    RotatorError,
    RotatorResult,
    SM_CLIENT,
};

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

pub fn get_secret_value(secret_id: &str, version_stage: Option<&str>, version_id: Option<&str>, timeout: Duration) -> RotatorResult<SecretValue> {
    info!("fetching secret for secret_id={} version_stage={:?} version_id={:?}", secret_id, version_stage, version_id);

    let value = get_secret_value_string(secret_id, version_stage, version_id, timeout)?;

    let secret = if let Some(string_value) = value.secret_string {
        serde_json::from_str(&string_value)
            .map_err(|err| RotatorError::SerialisationError {
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

pub fn get_secret_value_string(secret_id: &str, version_stage: Option<&str>, version_id: Option<&str>, timeout: Duration) -> RotatorResult<GetSecretValueResponse> {
    SM_CLIENT.get_secret_value(GetSecretValueRequest {
        secret_id: secret_id.to_string(),
        version_stage: version_stage.map(|s| s.to_string()),
        version_id: version_id.map(|s| s.to_string()),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|e| match e {
            RusotoError::Service(GetSecretValueError::ResourceNotFound(msg)) => {
                RotatorError::SecretValueNotFound {
                    secret_id: secret_id.to_string(),
                    version_stage: version_stage.map(|s| s.to_string()),
                    version_ids: version_id.map(|s| vec![s.to_string()]).unwrap_or(vec![]),
                    message: msg,
                }
            },
            e => RotatorError::GetSecretValue(format!("{:?}", e))
        })
}

pub fn put_secret_value(secret_id: &str, token: &str, secret: &Secret, version_stage: &str, timeout: Duration) -> RotatorResult<PutSecretValueResponse> {
    info!("putting secret for secret_id={} version_stage={:?} version_id={:?} secret={:?}", secret_id, version_stage, token, secret);

    let secret_string = serde_json::to_string(secret)
        .map_err(|err| RotatorError::SerialisationError {
            secret_id: secret_id.to_string(),
            message: format!("{:?}", err),
        })?;
    
    put_secret_value_string(secret_id, token, &secret_string, version_stage, timeout)
}

pub fn put_secret_value_string(secret_id: &str, token: &str, secret_string: &str, version_stage: &str, timeout: Duration) -> RotatorResult<PutSecretValueResponse> {
    SM_CLIENT.put_secret_value(PutSecretValueRequest {
        secret_id: secret_id.to_string(),
        client_request_token: Some(token.to_string()),
        secret_string: Some(secret_string.to_string()),
        version_stages: Some(vec![version_stage.to_string()]),
        ..Default::default()
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|e| match e {
            RusotoError::Service(PutSecretValueError::ResourceNotFound(msg)) => {
                RotatorError::SecretValueNotFound {
                    secret_id: secret_id.to_string(),
                    version_stage: Some(version_stage.to_string()),
                    version_ids: vec![token.to_string()],
                    message: msg,
                }
            },
            RusotoError::Service(PutSecretValueError::EncryptionFailure(msg)) => {
                RotatorError::EncryptionFailure {
                    secret_id: secret_id.to_string(),
                    message: msg,
                }
            },
            e => RotatorError::PutSecretValue(format!("{:?}", e))
        })
}