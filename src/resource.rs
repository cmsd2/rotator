use std::time::Duration;
use crate::error::{RotatorResult, RotatorError};
use crate::passwd::get_random_password;
use crate::config::Config;
use crate::iam::{
    CredentialStatus,
    list_service_specific_credentials,
    create_service_specific_credential,
    reset_service_specific_credential,
    update_service_specific_credential,
};
use crate::value::{
    Secret,
    SecretValue,
};

const MAX_SERVICE_SPECIFIC_CREDENTIALS: usize = 2;

pub trait Resource {
    fn create_new_password(&self, current_value: SecretValue) -> RotatorResult<Secret>;
    fn set_password(&self, s: Secret) -> RotatorResult<()>;
    fn test_password(&self, s: Secret) -> RotatorResult<()>;
}

pub struct GenericResource {
}

impl GenericResource {
    #[allow(dead_code)]
    pub fn new() -> Self {
        GenericResource {}
    }
}

impl Resource for GenericResource {

    fn create_new_password(&self, _current_value: SecretValue) -> RotatorResult<Secret> {
        let timeout = Duration::from_secs(2);
        let password = get_random_password(timeout)?;
        
        Ok(Secret {
            password: Some(password),
            ..Default::default()
        })
    }

    fn set_password(&self, _s: Secret) -> RotatorResult<()> {
        unimplemented!()
    }

    fn test_password(&self, _s: Secret) -> RotatorResult<()> {
        unimplemented!()
    }
}

#[derive(Debug, Clone)]
pub struct ServiceSpecificCredentialResource {
    secret_id: String,
    user_name: String,
    service_name: String,
}

impl ServiceSpecificCredentialResource {
    pub fn new(cfg: &Config) -> RotatorResult<Self> {
        let service_name = cfg.service_name.as_ref().ok_or_else(|| RotatorError::InvalidConfig {
            secret_id: cfg.secret_id.to_string(),
            message: format!("missing service name for service specific credential"),
        })?;

        let user_name = cfg.user_name.as_ref().ok_or_else(|| RotatorError::InvalidConfig {
            secret_id: cfg.secret_id.to_string(),
            message: format!("missing resource arn for service specific credential"),
        })?;
        
        Ok(ServiceSpecificCredentialResource {
            secret_id: cfg.secret_id.to_string(),
            user_name: user_name.to_string(),
            service_name: service_name.to_string(),
        })      
    }
}

impl Resource for ServiceSpecificCredentialResource {
    fn create_new_password(&self, current_value: SecretValue) -> RotatorResult<Secret> {
        let timeout = Duration::from_secs(2);
        
        let creds = list_service_specific_credentials(Some(&self.user_name), Some(&self.service_name), timeout)
            .map_err(|err| RotatorError::IamError {
                secret_id: self.secret_id.to_string(),
                message: format!("list service specific credential error: {:?}", err),
            })?;
        info!("found {} service specific credentials: {:?}", creds.len(), creds);
        
        let mut cred;
        
        if creds.len() < MAX_SERVICE_SPECIFIC_CREDENTIALS {
            info!("creating service specific credential user_name={} service_name={}", self.user_name, self.service_name);

            cred = create_service_specific_credential(&self.user_name, &self.service_name, timeout)
                .map_err(|err| RotatorError::IamError {
                    secret_id: self.secret_id.to_string(),
                    message: format!("create service specific credential error: {:?}", err)
                })?;
        } else {
            let cred_to_reset = if let Some(credential_id) = current_value.secret.as_ref()
                .and_then(|secret| secret.service_specific_credential_id.as_ref()) {

                creds.iter().find(|cred| &cred.service_specific_credential_id != credential_id)
            } else {
                creds.get(0)
            };

            if let Some(cred_to_reset) = cred_to_reset {
                info!("reseting service specific credential id={} user_name={}", cred_to_reset.service_specific_credential_id, cred_to_reset.service_user_name);

                cred = reset_service_specific_credential(&cred_to_reset.service_specific_credential_id, Some(&self.user_name), timeout)
                    .map_err(|err| RotatorError::IamError {
                        secret_id: self.secret_id.to_string(),
                        message: format!("reset service specific credential error: {:?}", err)
                    })?;
                
                if cred.status == CredentialStatus::Inactive {
                    info!("activating service specific credential id={} user_name={}", cred.service_specific_credential_id, cred.service_user_name);

                    update_service_specific_credential(&cred.service_specific_credential_id, Some(&self.user_name), CredentialStatus::Active, timeout)
                        .map_err(|err| RotatorError::IamError {
                            secret_id: self.secret_id.to_string(),
                            message: format!("update service specific credential error: {:?}", err)
                        })?;
                    
                    cred.status = CredentialStatus::Active;
                }
            } else {
                // shouldn't get here. there should be at least one credential that isn't current
                return Err(RotatorError::Other {
                    message: format!("no empty credential slots to rotate")
                });
            }
        }

        let mut secret = current_value.secret.clone().unwrap_or(Secret::default());

        secret.username = Some(cred.service_user_name);
        secret.password = Some(cred.service_password);
        secret.service_specific_credential_id = Some(cred.service_specific_credential_id);

        Ok(secret)
    }

    fn set_password(&self, _s: Secret) -> RotatorResult<()> {
        info!("nothing to do to set password");
        Ok(())
    }

    fn test_password(&self, _s: Secret) -> RotatorResult<()> {
        info!("test_password unimplemented");
        Ok(())
    }
}