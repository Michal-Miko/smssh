use aws_config::BehaviorVersion;
use color_eyre::{eyre::eyre, Result};

pub fn get_key_blocking(secret_arn: &str) -> Result<String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let key = runtime.block_on(get_key(secret_arn))?;
    Ok(key)
}

pub async fn get_key(secret_arn: &str) -> Result<String> {
    let secret_manager = aws_sdk_secretsmanager::Client::new(
        &aws_config::load_defaults(BehaviorVersion::latest()).await,
    );
    let response = secret_manager
        .get_secret_value()
        .secret_id(secret_arn)
        .send()
        .await?;
    let secret_value = response
        .secret_string()
        .ok_or(eyre!("The secret '{secret_arn}' does not contain a key"))?;
    Ok(secret_value.to_string())
}
