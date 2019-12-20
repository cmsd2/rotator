use rusoto_core::RusotoError;
use rusoto_secretsmanager::{
    SecretsManager,
    GetRandomPasswordRequest,
    GetRandomPasswordError,
};
use std::time::Duration;
use crate::{
    RotatorError,
    RotatorResult,
    SM_CLIENT,
};

pub fn get_random_password(timeout: Duration) -> RotatorResult<String> {
    SM_CLIENT.get_random_password(GetRandomPasswordRequest {
        exclude_characters: Some(r#"/@"'\"#.to_string()),
        ..Default::default()
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|e| match e {
            RusotoError::Service(GetRandomPasswordError::InvalidParameter(msg)) => {
                RotatorError::InvalidPasswordParameter {
                    message: msg,
                }
            },
            e => RotatorError::GetRandomPassword(format!("{:?}", e))
        })
        .and_then(|response| {
            response.random_password.ok_or(RotatorError::GetRandomPassword("missing password in response".to_string()))
        })
}