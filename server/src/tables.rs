use api_key::ApiKey;
use bincode::{Decode, Encode};
use chrono::{DateTime, Utc};
use redb::TableDefinition;
use thiserror::Error;
use typed_bytes::TypedTableDefinition;
use uuid::Uuid;

use crate::extractor::device_id::DeviceId;

#[derive(Debug, Encode, Decode)]
pub struct ApiKeyId(#[bincode(with_serde)] pub Uuid);

#[derive(Encode, Decode, Debug)]
pub struct ApiKeyDbRecord {
    #[bincode(with_serde)]
    pub key: ApiKey<DeviceId>,
    #[bincode(with_serde)]
    pub created_at: DateTime<Utc>,
}

pub const KEYS_TABLE: TypedTableDefinition<ApiKeyId, ApiKeyDbRecord> =
    TableDefinition::new("api_keys");

#[derive(Debug, Error)]
pub enum TableErr {
    #[error("{0:?}")]
    Enc(#[from] bincode::error::EncodeError),
    #[error("{0:?}")]
    Dev(#[from] bincode::error::DecodeError),
    #[error("{0:?}")]
    Redb(#[from] redb::Error),
}
