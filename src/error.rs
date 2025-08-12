use lambda_runtime::Diagnostic;
use serde::Serialize;

#[derive(Clone, Debug, Serialize, thiserror::Error)]
pub enum RotatorError {
    #[error("Invalid configuration: {message}")]
    InvalidConfig {
        message: String,
    },
    #[error("Unrecoverable error rotating secret: {secret_id}: {message}")]
    UnrecoverableSecretProblem {
        secret_id: String,
        message: String,
    },
    #[error("Error rotating secret: {secret_id}: {message}")]
    SecretProblem {
        secret_id: String,
        message: String,
    },
    #[error("Secret value not found for secret: {secret_id}")]
    SecretValueNotFound {
        secret_id: String,
        version_stage: Option<String>,
        version_id: Option<String>,
    },
    #[error("Unexpected error: {message}")]
    Other {
        message: String,
    },
}

pub type RotatorResult<R> = std::result::Result<R,RotatorError>;

impl From<RotatorError> for Diagnostic {
    fn from(value: RotatorError) -> Diagnostic {
        let (error_type, error_message) = match value {
            ref err @ RotatorError::InvalidConfig { message: ref _message } => ("NonRetryable", format!("{}", err)),
            ref err @ RotatorError::UnrecoverableSecretProblem { secret_id: ref _secret_id, message: ref _message } => ("NonRetryable", err.to_string()),
            ref err @ RotatorError::SecretProblem { secret_id: ref _secret_id, message: ref _message } => ("Retryable", err.to_string()),
            RotatorError::SecretValueNotFound { ref secret_id, version_stage, version_id } => ("Retryable", format!("Secret value not found for secret_id {} with version_stage {:?} and version_id {:?}", secret_id, version_stage, version_id)),
            ref err @ RotatorError::Other { message: ref _message } => ("NonRetryable", err.to_string()),};
        Diagnostic {
            error_type: error_type.into(),
            error_message,
        }
    }
}
