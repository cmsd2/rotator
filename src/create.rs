use crate::{
    Resource,
    RotatorError,
    RotatorEvent,
    RotatorResult,
};
use crate::config::Config;
use crate::value::{get_secret_value, put_secret_value};

pub async fn create_secret(config: &Config, e: RotatorEvent, r: Box<dyn Resource>) -> RotatorResult<()> {
    let current = get_secret_value(config, &e.secret_id, Some("AWSCURRENT"), None).await?;

    match get_secret_value(config, &e.secret_id, Some("AWSPENDING"), Some(&e.client_request_token)).await {
        Ok(_) => {
            info!("createSecret: Successfully retrieved secret for {}.", e.secret_id);
            Ok(())
        },
        Err(RotatorError::SecretValueNotFound { .. }) => {
            let secret = r.create_new_password(current).await?;

            put_secret_value(config, &e.secret_id, &e.client_request_token, &secret, "AWSPENDING").await?;
            info!("createSecret: Successfully put secret for ARN {} and version {}.", e.secret_id, e.client_request_token);

            Ok(())
        },
        Err(err) => {
            error!("createSecret: Error retrieving secret for ARN {} and version {}: {:?}.", e.secret_id, e.client_request_token, err);
            Err(err)
        }
    }?;
    
    Ok(())
}