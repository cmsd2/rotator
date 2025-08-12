use aws_sdk_secretsmanager::operation::describe_secret::{DescribeSecretError, DescribeSecretOutput};
use crate::error::{RotatorResult, RotatorError};
use crate::config::Config;

pub async fn describe_secret(config: &Config, secret_id: &str) -> RotatorResult<DescribeSecretOutput> {
    info!("desribing secret {}", secret_id);

    config.sm_client.describe_secret()
        .secret_id(secret_id.to_owned())
        .send()
        .await
        .map_err(|err| match err.into_service_error() {
            DescribeSecretError::ResourceNotFoundException(_) => {
                RotatorError::UnrecoverableSecretProblem {
                    secret_id: secret_id.to_owned(),
                    message: format!("Secret not found: {}", secret_id),
                }
            },
            DescribeSecretError::InvalidParameterException(_) => {
                RotatorError::UnrecoverableSecretProblem {
                    secret_id: secret_id.to_owned(),
                    message: format!("Invalid parameter for secret: {}", secret_id),
                }
            },
            DescribeSecretError::InternalServiceError(_) => {
                RotatorError::UnrecoverableSecretProblem {
                    secret_id: secret_id.to_owned(),
                    message: format!("Internal service error for secret: {}", secret_id),
                }
            },
            err @ _ => RotatorError::Other {
                message: format!("Unexpected error describing secret {}: {:?}", secret_id, err),
            },
        })
}