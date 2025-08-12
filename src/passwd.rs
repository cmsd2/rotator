use crate::{
    RotatorError,
    RotatorResult
};
use crate::config::Config;
use aws_sdk_secretsmanager::operation::get_random_password::GetRandomPasswordError;

pub async fn get_random_password(config: &Config) -> RotatorResult<String> {
    config.sm_client.get_random_password()
        .exclude_characters(r#"/@"'\"#.to_string())
        .send()
        .await
        .map_err(|e| match e.into_service_error() {
            GetRandomPasswordError::InvalidParameterException(_) => {
                RotatorError::Other {
                    message: format!("Invalid parameter when requesting random password"),
                }
            },
            e => RotatorError::Other {
                message: format!("{:?}", e)
            }
        })
        .and_then(|response| {
            response.random_password.ok_or(RotatorError::Other {
                message: format!("missing password in response")
            })
        })
}