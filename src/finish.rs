use std::time::Duration;
use rusoto_core::RusotoError;
use rusoto_secretsmanager::{
    SecretsManager,
    UpdateSecretVersionStageRequest,
    UpdateSecretVersionStageResponse,
    UpdateSecretVersionStageError,
};
use crate::{
    RotatorError,
    RotatorEvent,
    RotatorResult,
    SM_CLIENT,
};
use crate::describe_secret;
use std::collections::HashMap;

fn update_secret_version_stage(secret_id: &str, version_stage: &str, new_version: &str, old_version: Option<String>, timeout: Duration) -> RotatorResult<UpdateSecretVersionStageResponse> {
    info!("updating secret version stage secret_id={} version_stage={} new_version={} old_version={:?}", secret_id, version_stage, new_version, old_version);

    SM_CLIENT.update_secret_version_stage(UpdateSecretVersionStageRequest {
        secret_id: secret_id.to_string(),
        version_stage: "AWSCURRENT".to_string(),
        move_to_version_id: Some(new_version.to_string()),
        remove_from_version_id: old_version.clone(),
    })
        .with_timeout(timeout)
        .sync()
        .map_err(|e| match e {
            RusotoError::Service(UpdateSecretVersionStageError::ResourceNotFound(msg)) => {
                let mut version_ids = old_version.map(|s| vec![s.to_string()]).unwrap_or(vec![]);
                version_ids.push(new_version.to_string());
                RotatorError::SecretValueNotFound {
                    secret_id: secret_id.to_string(),
                    version_stage: Some(version_stage.to_string()),
                    version_ids: version_ids,
                    message: msg
                }
            },
            e => RotatorError::UpdateSecretVersionStage(format!("{:?}", e))
        })
}

pub fn finish_secret(e: RotatorEvent) -> RotatorResult<()> {
    let timeout = Duration::from_secs(2);

    let secret = describe_secret(&e.secret_id, timeout)?;
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

    update_secret_version_stage(&e.secret_id, "AWSCURRENT", &e.client_request_token, current_version, timeout)?;
    info!("finishSecret: Successfully set AWSCURRENT stage to version {:?} for secret {}.", e.client_request_token, e.secret_id);
    
    Ok(())
}