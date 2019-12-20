use rusoto_core::RusotoError;
use std::time::Duration;
use std::fmt;
use rusoto_iam::{
    Iam,
    ListServiceSpecificCredentialsRequest,
    ListServiceSpecificCredentialsError,
    ServiceSpecificCredential,
    ServiceSpecificCredentialMetadata,
    CreateServiceSpecificCredentialRequest,
    CreateServiceSpecificCredentialError,
    ResetServiceSpecificCredentialRequest,
    ResetServiceSpecificCredentialError,
    UpdateServiceSpecificCredentialRequest,
    UpdateServiceSpecificCredentialError,
};
use crate::IAM_CLIENT;

#[derive(Debug, Clone, PartialEq)]
pub enum CredentialStatus {
    Active,
    Inactive,
    Unknown(String),
}

impl From<&str> for CredentialStatus {
    fn from(s: &str) -> CredentialStatus {
        match s {
            "Active" => CredentialStatus::Active,
            "Inactive" => CredentialStatus::Inactive,
            s => CredentialStatus::Unknown(s.to_string())
        }
    }
}

impl Into<String> for CredentialStatus {
    fn into(self) -> String {
        match self {
            CredentialStatus::Active => "Active".to_string(),
            CredentialStatus::Inactive => "Inactive".to_string(),
            CredentialStatus::Unknown(s) => s,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CredentialMetadata {
    pub create_date: String,
    pub service_name: String,
    pub service_specific_credential_id: String,
    pub service_user_name: String,
    pub status: CredentialStatus,
    pub user_name: String,
}

impl From<ServiceSpecificCredentialMetadata> for CredentialMetadata {
    fn from(m: ServiceSpecificCredentialMetadata) -> CredentialMetadata {
        CredentialMetadata {
            create_date: m.create_date,
            service_name: m.service_name,
            service_specific_credential_id: m.service_specific_credential_id,
            service_user_name: m.service_user_name,
            status: CredentialStatus::from(&m.status[..]),
            user_name: m.user_name,
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Credential {
    pub create_date: String,
    pub service_name: String,
    pub service_password: String,
    pub service_specific_credential_id: String,
    pub service_user_name: String,
    pub status: CredentialStatus,
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
            status: CredentialStatus::from(&m.status[..]),
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

pub fn list_service_specific_credentials(user_name: Option<&str>, service_name: Option<&str>, timeout: Duration) -> IamResult<Vec<CredentialMetadata>> {
    info!("listing service specific credentials user_name={:?} service_name={:?}", user_name, service_name);
    
    IAM_CLIENT.list_service_specific_credentials(ListServiceSpecificCredentialsRequest {
        service_name: service_name.map(|s| s.to_string()),
        user_name: user_name.map(|s| s.to_string()),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|err| match err {
            RusotoError::Service(ListServiceSpecificCredentialsError::NoSuchEntity(msg)) => {
                IamError::EntityNotFound {
                    message: msg
                }
            },
            RusotoError::Service(ListServiceSpecificCredentialsError::ServiceNotSupported(msg)) => {
                IamError::ServiceNotSupported {
                    message: msg
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

pub fn create_service_specific_credential(user_name: &str, service_name: &str, timeout: Duration) -> IamResult<Credential> {
    let cred = IAM_CLIENT.create_service_specific_credential(CreateServiceSpecificCredentialRequest {
        user_name: user_name.to_string(),
        service_name: service_name.to_string(),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|err| match err {
            RusotoError::Service(CreateServiceSpecificCredentialError::LimitExceeded(msg)) => {
                IamError::LimitExceeded {
                    message: msg
                }
            },
            RusotoError::Service(CreateServiceSpecificCredentialError::NoSuchEntity(msg)) => {
                IamError::EntityNotFound {
                    message: msg
                }
            },
            RusotoError::Service(CreateServiceSpecificCredentialError::ServiceNotSupported(msg)) => {
                IamError::ServiceNotSupported {
                    message: msg
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

pub fn reset_service_specific_credential(id: &str, user_name: Option<&str>, timeout: Duration) -> IamResult<Credential> {
    let cred = IAM_CLIENT.reset_service_specific_credential(ResetServiceSpecificCredentialRequest {
        service_specific_credential_id: id.to_string(),
        user_name: user_name.map(|s| s.to_string()),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|err| match err {
            RusotoError::Service(ResetServiceSpecificCredentialError::NoSuchEntity(msg)) => {
                IamError::EntityNotFound {
                    message: msg
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

pub fn update_service_specific_credential(id: &str, user_name: Option<&str>, status: CredentialStatus, timeout: Duration) -> IamResult<()> {
    IAM_CLIENT.update_service_specific_credential(UpdateServiceSpecificCredentialRequest {
        service_specific_credential_id: id.to_string(),
        user_name: user_name.map(|s| s.to_string()),
        status: status.into(),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|err| match err {
            RusotoError::Service(UpdateServiceSpecificCredentialError::NoSuchEntity(msg)) => {
                IamError::EntityNotFound {
                    message: msg
                }
            },
            err => IamError::RusotoError {
                message: format!("{:?}", err)
            }
        })
}