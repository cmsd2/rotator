use aws_sdk_secretsmanager::operation::update_secret_version_stage::{UpdateSecretVersionStageError, UpdateSecretVersionStageOutput};
use crate::{
    RotatorError,
    RotatorEvent,
    RotatorResult,
};
use crate::describe_secret;
use crate::config::Config;
use std::collections::HashMap;

async fn update_secret_version_stage(config: &Config, secret_id: &str, version_stage: &str, new_version: &str, old_version: Option<String>) -> RotatorResult<UpdateSecretVersionStageOutput> {
    info!("updating secret version stage secret_id={} version_stage={} new_version={} old_version={:?}", secret_id, version_stage, new_version, old_version);

    let mut builder = config.sm_client.update_secret_version_stage()
        .secret_id(secret_id.to_string())
        .version_stage("AWSCURRENT".to_string())
        .move_to_version_id(new_version.to_string());

    if let Some(ref old_version) = old_version {
        builder = builder.remove_from_version_id(old_version.to_owned());
    }
    
    builder
        .send()
        .await
        .map_err(|e| match e.into_service_error() {
            UpdateSecretVersionStageError::ResourceNotFoundException(_) => {
                let mut version_ids = old_version.map(|s| vec![s.to_string()]).unwrap_or(vec![]);
                version_ids.push(new_version.to_string());
                RotatorError::UnrecoverableSecretProblem { 
                    secret_id: secret_id.to_string(),
                    message: format!("Version stage {} not found. ids: {:?}", version_stage, version_ids) 
                }
            },
            e @ _ => RotatorError::Other {
                message: format!("Unexpected error updating secret version stage for secret {}: {:?}", secret_id, e),
            },
        })
}

pub async fn finish_secret(config: &Config, e: RotatorEvent) -> RotatorResult<()> {
    let secret = describe_secret(config, &e.secret_id).await?;
    let mut current_version = None;
    let version_ids_to_stages = secret.version_ids_to_stages.unwrap_or(HashMap::new());
    for (version, stages) in version_ids_to_stages.iter() {
        if stages.contains(&"AWSCURRENT".to_string()) {
            if version == &e.client_request_token {
                info!("finishSecret: Version {:?} already marked as AWSCURRENT for {}", version, e.secret_id);
                return Ok(());
            }
            current_version = Some(version.to_string());
        }
    }

    update_secret_version_stage(config, &e.secret_id, "AWSCURRENT", &e.client_request_token, current_version).await?;
    info!("finishSecret: Successfully set AWSCURRENT stage to version {:?} for secret {}.", e.client_request_token, e.secret_id);
    
    Ok(())
}