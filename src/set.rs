use std::time::Duration;
use crate::{
    RotatorEvent,
    RotatorResult,
};
use crate::value::get_secret_value;

pub fn set_secret(e: RotatorEvent) -> RotatorResult<()> {
    let timeout = Duration::from_secs(2);

    let _value = match get_secret_value(&e.secret_id, Some("AWSPENDING"), Some(&e.client_request_token), timeout) {
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