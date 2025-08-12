use std::iter::Iterator;
use std::error::Error as StdError;
use std::convert::TryFrom;
use std::result::Result;
use std::fmt;
use aws_sdk_secretsmanager::types::Tag;
use crate::error::{RotatorResult, RotatorError};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ResourceType {
    ServiceSpecificCredential
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceTypeParseError(String);

impl StdError for ResourceTypeParseError {
    fn description(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ResourceTypeParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "resource type parse error: {}", self)
    }
}

impl TryFrom<&str> for ResourceType {
    type Error = ResourceTypeParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.as_ref() {
            "ServiceSpecificCredential" => Ok(ResourceType::ServiceSpecificCredential),
            s => Err(ResourceTypeParseError(format!("invalid resource type '{}'", s)))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub sm_client: aws_sdk_secretsmanager::Client,
    pub iam_client: aws_sdk_iam::Client,
    pub secret_id: String,
    pub resource_type: Option<ResourceType>,
    pub user_name: Option<String>,
    pub service_name: Option<String>,
}

impl Config {
    pub fn new_for_secret(sm_client: aws_sdk_secretsmanager::Client, iam_client: aws_sdk_iam::Client, secret_id: &str) -> RotatorResult<Self> {
        Ok(Config {
            sm_client,
            iam_client,
            secret_id: secret_id.to_string(),
            resource_type: None,
            service_name: None,
            user_name: None,
        })
    }

    pub fn set_tags(&mut self, tags: Vec<Tag>) -> RotatorResult<()>{
        self.resource_type = Some(Self::get_required_tag(&self.secret_id, tags.iter(), "rotator:resourceType")?);
        self.service_name = Self::get_tag(&self.secret_id, tags.iter(), "rotator:serviceName")?;
        self.user_name = Self::get_tag(&self.secret_id, tags.iter(), "rotator:userName")?;
        Ok(())
    }

    pub fn get_required_tag<'a, E: StdError, T: TryFrom<&'a str, Error=E>, I: Iterator<Item=&'a Tag>>(secret_id: &str, tags: I, name: &str) -> RotatorResult<T> {
        Self::get_tag(secret_id, tags, name)
            .and_then(|value| value.ok_or_else(|| RotatorError::UnrecoverableSecretProblem {
                secret_id: secret_id.to_string(),
                message: format!("Missing tag: {}", name.to_string())
            }))
    }

    pub fn get_tag<'a, E: StdError, T: TryFrom<&'a str, Error=E>, I: Iterator<Item=&'a Tag>>(secret_id: &str, tags: I, name: &str) -> RotatorResult<Option<T>> {
        for t in tags {
            if let Some(ref key) = t.key {
                if key == name {
                    if let Some(ref value) = t.value {
                        return T::try_from(value)
                            .map(|v| Some(v))
                            .map_err(|err: E| RotatorError::UnrecoverableSecretProblem {
                                secret_id: secret_id.to_string(),
                                message: format!("{:?}", err),
                            });
                    } else {
                        return Ok(None);
                    }
                }
            }
        }

        Ok(None)
    }
}