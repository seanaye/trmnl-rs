use crate::{
    extractor::device_id::DeviceId,
    tables::{ApiKeyDbRecord, ApiKeyId, KEYS_TABLE},
};
use api_key::{ApiKeyPair, TokenString};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use chrono::Utc;
use redb::Database;
use serde::Serialize;
use std::sync::Arc;
use thiserror::Error;
use typed_bytes::TypedBytes;
use url::Url;

#[derive(Debug, Serialize)]
pub struct SetupResponse {
    api_key: api_key::TokenString<'static>,
    friendly_id: String,
    image_url: Url,
    message: String,
}

#[derive(Debug, Error)]
pub enum SetupErr {
    #[error("{0:?}")]
    Redb(#[from] redb::Error),
    #[error("{0:?}")]
    Encode(#[from] bincode::error::EncodeError),
    #[error("{0:?}")]
    JoinErr(#[from] tokio::task::JoinError),
}

impl IntoResponse for SetupErr {
    fn into_response(self) -> axum::response::Response {
        match self {
            SetupErr::Redb(_) | SetupErr::Encode(_) | SetupErr::JoinErr(_) => {
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

#[axum::debug_handler(state = Arc<Database>)]
pub async fn setup_handler(
    State(db): State<Arc<Database>>,
    device: DeviceId,
) -> Result<Json<SetupResponse>, SetupErr> {
    let token = tokio::task::spawn_blocking(move || create_and_write_key(db, device)).await??;
    Ok(Json(SetupResponse {
        api_key: token,
        friendly_id: "my_termnl".to_string(),
        image_url: "http://mira:2443/assets/setup.bmp".parse().unwrap(),
        message: "Hello world".to_string(),
    }))
}

fn create_and_write_key(
    db: Arc<Database>,
    device: DeviceId,
) -> Result<TokenString<'static>, SetupErr> {
    let ApiKeyPair { api_key, token, .. } = api_key::ApiKeyPair::new(device);

    let tx = db.begin_write().map_err(redb::Error::from)?;
    let mut table = tx.open_table(KEYS_TABLE).map_err(redb::Error::from)?;

    let _res = table.insert(
        TypedBytes::new(ApiKeyId(api_key.id))?,
        TypedBytes::new(ApiKeyDbRecord {
            key: api_key,
            created_at: Utc::now(),
        })?,
    );

    Ok(token)
}
