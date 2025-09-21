use std::sync::Arc;

use axum::{Json, http::StatusCode};
use axum_extra::{TypedHeader, headers::Host};
use redb::Database;
use serde::Serialize;
use url::Url;

use crate::extractor::api_key::ApiKeyExtractor;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SpecialFunction {
    Sleep,
    None,
}

#[derive(Debug, Serialize)]
pub struct DisplayResponse {
    filename: String,
    firmware_url: Url,
    image_url: Url,
    image_url_timeout: usize,
    refresh_rate: usize,
    reset_firmware: bool,
    special_function: SpecialFunction,
    update_firmware: bool,
}

#[axum::debug_handler(state = Arc<Database>)]
pub async fn display(
    key: ApiKeyExtractor,
    TypedHeader(host): TypedHeader<Host>,
) -> Result<Json<DisplayResponse>, StatusCode> {
    Ok(Json(DisplayResponse {
        filename: "sample.BMP".to_string(),
        firmware_url: "https://example.com".parse().unwrap(),
        image_url: format_url(host)?,
        image_url_timeout: 0,
        refresh_rate: 60,
        reset_firmware: false,
        special_function: SpecialFunction::Sleep,
        update_firmware: false,
    }))
}

fn format_url(host: Host) -> Result<Url, StatusCode> {
    match host.port() {
        Some(p) => format!("http://{}:{p}/sample.BMP", host.hostname()),
        None => format!("http://{}/sample.BMP", host.hostname()),
    }
    .parse()
    .map_err(|_| StatusCode::BAD_REQUEST)
}
