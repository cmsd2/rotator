use std::iter::Iterator;
use std::error::Error as StdError;
use std::convert::TryFrom;
use std::result::Result;
use std::fmt;
use rusoto_secretsmanager::Tag;
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

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub secret_id: String,
    pub resource_type: ResourceType,
    pub user_name: Option<String>,
    pub service_name: Option<String>,
}

impl Config {
    pub fn new_for_secret(secret_id: &str, tags: Option<Vec<Tag>>) -> RotatorResult<Self> {
        let tags = tags.ok_or_else(|| RotatorError::MissingTags {
            secret_id: secret_id.to_string(),
        })?;

        Ok(Config {
            secret_id: secret_id.to_string(),
            resource_type: Self::get_required_tag(secret_id, tags.iter(), "rotator:resourceType")?,
            service_name: Self::get_tag(secret_id, tags.iter(), "rotator:serviceName")?,
            user_name: Self::get_tag(secret_id, tags.iter(), "rotator:userName")?,
        })
    }

    pub fn get_required_tag<'a, E: StdError, T: TryFrom<&'a str, Error=E>, I: Iterator<Item=&'a Tag>>(secret_id: &str, tags: I, name: &str) -> RotatorResult<T> {
        Self::get_tag(secret_id, tags, name)
            .and_then(|value| value.ok_or_else(|| RotatorError::MissingTag {
                secret_id: secret_id.to_string(),
                tag_name: name.to_string(),
            }))
    }

    pub fn get_tag<'a, E: StdError, T: TryFrom<&'a str, Error=E>, I: Iterator<Item=&'a Tag>>(secret_id: &str, tags: I, name: &str) -> RotatorResult<Option<T>> {
        for t in tags {
            if let Some(ref key) = t.key {
                if key == name {
                    if let Some(ref value) = t.value {
                        return T::try_from(value)
                            .map(|v| Some(v))
                            .map_err(|err: E| RotatorError::SerialisationError {
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