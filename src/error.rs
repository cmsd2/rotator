use std::fmt;
use serde::Serialize;
use lambda_runtime::error::{LambdaErrorExt};

#[derive(Clone, Debug, Serialize, LambdaErrorExt)]
pub enum RotatorError {
    DescribeSecretError(String),
    GetSecretValue(String),
    GetRandomPassword(String),
    PutSecretValue(String),
    UpdateSecretVersionStage(String),
    IamError {
        secret_id: String,
        message: String,
    },
    InvalidConfig {
        secret_id: String,
        message: String,
    },
    SecretNotFound {
        secret_id: String,
        message: String,
    },
    SecretValueNotFound {
        secret_id: String,
        version_stage: Option<String>,
        version_ids: Vec<String>,
        message: String,
    },
    RotationNotEnabled {
        secret_id: String,
    },
    NoStageForRotation {
        secret_id: String,
        version: String,
    },
    NotSetAsPending {
        secret_id: String,
        version: String,
    },
    InvalidPasswordParameter {
        message: String,
    },
    EncryptionFailure {
        secret_id: String,
        message: String,
    },
    SerialisationError {
        secret_id: String,
        message: String,
    },
    MissingTags {
        secret_id: String,
    },
    MissingTag {
        secret_id: String,
        tag_name: String,
    },
    Other {
        message: String,
    },
}

impl fmt::Display for RotatorError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for RotatorError {
}

pub type RotatorResult<R> = std::result::Result<R,RotatorError>;
