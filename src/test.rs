use crate::{
    RotatorEvent,
    RotatorResult,
};
use crate::value::get_secret_value;
use crate::config::Config;

pub async fn test_secret(config: &Config, e: RotatorEvent) -> RotatorResult<()> {
    let _value = match get_secret_value(config, &e.secret_id, Some("AWSPENDING"), Some(&e.client_request_token)).await {
        Ok(value) => {
            info!("testSecret: Successfully retrieved secret for {}.", e.secret_id);
            Ok(value)
        },
        Err(err) => {
            error!("testSecret: Error retrieving secret for ARN {} and version {}: {:?}.", e.secret_id, e.client_request_token, err);
            Err(err)
        }
    }?;

    // todo: test secret on resource

    
    Ok(())
}