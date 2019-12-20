use std::time::Duration;
use crate::{
    Resource,
    RotatorError,
    RotatorEvent,
    RotatorResult,
};
use crate::value::{get_secret_value, put_secret_value};

pub fn create_secret(e: RotatorEvent, r: Box<dyn Resource>) -> RotatorResult<()> {
    let timeout = Duration::from_secs(2);

    let current = get_secret_value(&e.secret_id, Some("AWSCURRENT"), None, timeout)?;

    match get_secret_value(&e.secret_id, Some("AWSPENDING"), Some(&e.client_request_token), timeout) {
        Ok(_) => {
            info!("createSecret: Successfully retrieved secret for {}.", e.secret_id);
            Ok(())
        },
        Err(RotatorError::SecretValueNotFound { .. }) => {
            let secret = r.create_new_password(current)?;

            put_secret_value(&e.secret_id, &e.client_request_token, &secret, "AWSPENDING", timeout)?;
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