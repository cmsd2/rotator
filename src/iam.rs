use std::fmt;
use aws_sdk_iam::operation::create_service_specific_credential::CreateServiceSpecificCredentialError;
use aws_sdk_iam::operation::list_service_specific_credentials::ListServiceSpecificCredentialsError;
use aws_sdk_iam::operation::reset_service_specific_credential::ResetServiceSpecificCredentialError;
use aws_sdk_iam::operation::update_service_specific_credential::UpdateServiceSpecificCredentialError;
use aws_sdk_iam::types::{StatusType, ServiceSpecificCredential, ServiceSpecificCredentialMetadata};
use aws_sdk_iam::primitives::DateTime;
use crate::config::Config;

#[derive(Debug, Clone, PartialEq)]
pub enum CredentialStatus {
    Active,
    Inactive,
    Expired,
    Unknown(String),
}

impl From<&str> for CredentialStatus {
    fn from(s: &str) -> CredentialStatus {
        match s {
            "Active" => CredentialStatus::Active,
            "Inactive" => CredentialStatus::Inactive,
            "Expired" => CredentialStatus::Expired,
            s => CredentialStatus::Unknown(s.to_string())
        }
    }
}

impl Into<String> for CredentialStatus {
    fn into(self) -> String {
        match self {
            CredentialStatus::Active => "Active".to_string(),
            CredentialStatus::Inactive => "Inactive".to_string(),
            CredentialStatus::Expired => "Expired".to_string(),
            CredentialStatus::Unknown(s) => s,
        }
    }
}

impl Into<aws_sdk_iam::types::StatusType> for CredentialStatus {
    fn into(self) -> aws_sdk_iam::types::StatusType {
        match self {
            CredentialStatus::Active => aws_sdk_iam::types::StatusType::Active,
            CredentialStatus::Inactive => aws_sdk_iam::types::StatusType::Inactive,
            CredentialStatus::Expired => aws_sdk_iam::types::StatusType::Expired,
            CredentialStatus::Unknown(s) => (&s[..]).into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CredentialMetadata {
    pub create_date: DateTime,
    pub service_name: String,
    pub service_specific_credential_id: String,
    pub service_user_name: String,
    pub status: StatusType,
    pub user_name: String,
}

impl From<ServiceSpecificCredentialMetadata> for CredentialMetadata {
    fn from(m: ServiceSpecificCredentialMetadata) -> CredentialMetadata {
        CredentialMetadata {
            create_date: m.create_date,
            service_name: m.service_name,
            service_specific_credential_id: m.service_specific_credential_id,
            service_user_name: m.service_user_name,
            status: m.status,
            user_name: m.user_name,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Credential {
    pub create_date: DateTime,
    pub service_name: String,
    pub service_password: String,
    pub service_specific_credential_id: String,
    pub service_user_name: String,
    pub status: StatusType,
    pub user_name: String,
}

impl From<ServiceSpecificCredential> for Credential {
    fn from(m: ServiceSpecificCredential) -> Credential {
        Credential {
            create_date: m.create_date,
            service_name: m.service_name,
            service_password: m.service_password,
            service_specific_credential_id: m.service_specific_credential_id,
            service_user_name: m.service_user_name,
            status: m.status,
            user_name: m.user_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum IamError {
    EntityNotFound {
        message: String,
    },
    ServiceNotSupported {
        message: String,
    },
    LimitExceeded {
        message: String,
    },
    RusotoError {
        message: String,
    },
    MissingCredentialInResponse,
}

impl fmt::Display for IamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "iam error: {:?}", self)
    }
}

impl std::error::Error for IamError {
}

pub type IamResult<T> = std::result::Result<T, IamError>;

pub async fn list_service_specific_credentials(config: &Config, user_name: Option<&str>, service_name: Option<&str>) -> IamResult<Vec<CredentialMetadata>> {
    info!("listing service specific credentials user_name={:?} service_name={:?}", user_name, service_name);
    
    let mut builder = config.iam_client.list_service_specific_credentials();

    if let Some(service_name) = service_name {
        builder = builder.service_name(service_name.to_string());
    }

    if let Some(user_name) = user_name {
        builder = builder.user_name(user_name.to_string());
    }

    builder
        .send()
        .await
        .map_err(|err| match err.into_service_error() {
            ListServiceSpecificCredentialsError::NoSuchEntityException(_) => {
                IamError::EntityNotFound {
                    message: format!("Service specific credentials not found for user {:?} or service_name {:?}", user_name, service_name)
                }
            },
            ListServiceSpecificCredentialsError::ServiceNotSupportedException(_) => {
                IamError::ServiceNotSupported {
                    message: format!("Service specific credentials not supported for service_name {:?}", service_name)
                }
            },
            err => IamError::RusotoError {
                message: format!("{:?}", err)
            }
        })
        .map(|creds| {
            creds.service_specific_credentials.unwrap_or(vec![])
                .into_iter()
                .map(|metadata| CredentialMetadata::from(metadata))
                .collect()
        })
}

pub async fn create_service_specific_credential(config: &Config, user_name: &str, service_name: &str) -> IamResult<Credential> {
    let cred = config.iam_client.create_service_specific_credential()
        .user_name(user_name.to_string())
        .service_name(service_name.to_string())
        .send()
        .await
        .map_err(|err| match err.into_service_error() {
            CreateServiceSpecificCredentialError::LimitExceededException(_) => {
                IamError::LimitExceeded {
                    message: format!("Limit exceeded for service specific credentials for user {} and service_name {}", user_name, service_name)
                }
            },
            CreateServiceSpecificCredentialError::NoSuchEntityException(_) => {
                IamError::EntityNotFound {
                    message: format!("Entity not found for user {} and service_name {}", user_name, service_name)
                }
            },
            CreateServiceSpecificCredentialError::ServiceNotSupportedException(_) => {
                IamError::ServiceNotSupported {
                    message: format!("Service not supported for service_name {}", service_name)
                }
            },
            err => IamError::RusotoError {
                message: format!("{:?}", err)
            }
        })?;
    
    cred.service_specific_credential
        .ok_or_else(|| IamError::MissingCredentialInResponse)
        .map(|cred| Credential::from(cred))
}

pub async fn reset_service_specific_credential(config: &Config, id: &str, user_name: Option<&str>) -> IamResult<Credential> {
    let mut builder = config.iam_client.reset_service_specific_credential()
        .service_specific_credential_id(id.to_string());

    if let Some(user_name) = user_name {
        builder = builder.user_name(user_name.to_string());
    }

    let cred = builder
        .send()
        .await
        .map_err(|err| match err.into_service_error() {
            ResetServiceSpecificCredentialError::NoSuchEntityException(_) => {
                IamError::EntityNotFound {
                    message: format!("Service specific credential not found for id {} and user_name {:?}", id, user_name)
                }
            },
            err => IamError::RusotoError {
                message: format!("{:?}", err)
            }
        })?;

    cred.service_specific_credential
        .ok_or_else(|| IamError::MissingCredentialInResponse)
        .map(|cred| Credential::from(cred))
}

pub async fn update_service_specific_credential(config: &Config, id: &str, user_name: Option<&str>, status: CredentialStatus) -> IamResult<()> {
    let mut builder = config.iam_client.update_service_specific_credential()
        .service_specific_credential_id(id.to_string());

    if let Some(user_name) = user_name {
        builder = builder.user_name(user_name.to_string());
    }

    builder.status(status.into())
        .send()
        .await
        .map_err(|err| match err.into_service_error() {
            UpdateServiceSpecificCredentialError::NoSuchEntityException(_) => {
                IamError::EntityNotFound {
                    message: format!("Service specific credential not found for id {} and user_name {:?}", id, user_name)
                }
            },
            err => IamError::RusotoError {
                message: format!("{:?}", err)
            }
        })?;

    Ok(())
}