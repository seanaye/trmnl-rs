use crate::{
    extractor::api_key::{ApiKeyExtractor, TokenExtractor},
    tables::TableErr,
    trace_err::TraceErr,
};
use axum::{Json, extract::State, http::StatusCode};
use bincode::{Decode, Encode};
use redb::{Database, TableDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use typed_bytes::{TypedBytes, TypedTableDefinition};

#[derive(Debug, Deserialize)]
pub struct LogsPayload {
    logs: Vec<LogEntry>,
}

/// https://github.com/usetrmnl/trmnl-firmware/blob/f770cbdb87991fa4daf60097bf40b5806f0f92aa/lib/wificaptive/src/wifi-helpers.cpp#L3
#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
#[serde(rename_all = "snake_case")]
pub enum WifiStatus {
    NoShield,
    IdleStatus,
    NoSsidAvail,
    ScanCompleted,
    Connected,
    ConnectFailed,
    ConnectionLost,
    Disconnected,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode)]
pub struct LogEntry {
    id: LogId,
    message: String,
    wifi_status: WifiStatus,
    created_at: usize,
    wifi_signal: i32,
    battery_voltage: f64,
    #[bincode(with_serde)]
    firmware_version: semver::Version,
}

#[derive(Debug, Serialize, Deserialize, Encode, Decode, Clone, Copy)]
#[serde(transparent)]
struct LogId(u32);

const LOGS_TABLE: TypedTableDefinition<LogId, LogEntry> = TableDefinition::new("logs");

#[axum::debug_handler(state = Arc<Database>)]
#[tracing::instrument(ret, skip(db, key))]
pub async fn log_handler(
    key: Option<TokenExtractor>,
    State(db): State<Arc<Database>>,
    Json(content): Json<LogsPayload>,
) -> StatusCode {
    let _ = tokio::task::spawn_blocking(move || record_logs(db, content)).await;
    StatusCode::NO_CONTENT
}

#[tracing::instrument(err, skip(db))]
fn record_logs(db: Arc<Database>, content: LogsPayload) -> Result<(), TableErr> {
    let tx = db.begin_write().map_err(redb::Error::from)?;
    let mut table = tx.open_table(LOGS_TABLE).map_err(redb::Error::from)?;
    content
        .logs
        .into_iter()
        .map(|log| -> Result<(), TableErr> {
            let key = TypedBytes::new(log.id)?;
            let val = TypedBytes::new(log)?;
            let _ = table
                .insert(key, val)
                .trace_err()
                .map_err(redb::Error::from)?;
            Ok(())
        })
        .for_each(drop);
    Ok(())
}
