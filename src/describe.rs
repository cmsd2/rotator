use std::time::Duration;
use rusoto_core::RusotoError;
use rusoto_secretsmanager::{
    SecretsManager,
    DescribeSecretRequest,
    DescribeSecretResponse,
    DescribeSecretError,
};
use crate::error::{RotatorResult, RotatorError};
use crate::SM_CLIENT;

pub fn describe_secret(secret_id: &str, timeout: Duration) -> RotatorResult<DescribeSecretResponse> {
    info!("desribing secret {}", secret_id);

    SM_CLIENT.describe_secret(DescribeSecretRequest {
        secret_id: secret_id.to_string()
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|e| match e {
            RusotoError::Service(DescribeSecretError::ResourceNotFound(msg)) => {
                RotatorError::SecretNotFound {
                    secret_id: secret_id.to_string(),
                    message: msg,
                }
            },
            e => RotatorError::DescribeSecretError(format!("{:?}", e))
        })  
}