use crate::{
    RotatorEvent,
    RotatorResult,
};
use crate::value::get_secret_value;
use crate::config::Config;

pub async fn set_secret(config: &Config, e: RotatorEvent) -> RotatorResult<()> {
    let _value = match get_secret_value(config, &e.secret_id, Some("AWSPENDING"), Some(&e.client_request_token)).await {
        Ok(value) => {
            info!("setSecret: Successfully retrieved secret for {}.", e.secret_id);
            Ok(value)
        },
        Err(err) => {
            error!("setSecret: Error retrieving secret for ARN {} and version {}: {:?}.", e.secret_id, e.client_request_token, err);
            Err(err)
        }
    }?;

    // todo: set secret on resource

    
    Ok(())
}