use crate::RatatuiHandle;
use axum::{
    Json,
    extract::State,
    http::{StatusCode, header::CONTENT_TYPE},
    response::IntoResponse,
};
use axum_extra::{TypedHeader, headers::Host};
use chrono::{Timelike, Utc};
use redb::Database;
use serde::Serialize;
use std::sync::Arc;
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SpecialFunction {
    Sleep,
    None,
}

#[derive(Debug, Serialize)]
pub struct DisplayResponse {
    filename: String,
    firmware_url: Option<Url>,
    image_url: Url,
    image_url_timeout: usize,
    refresh_rate: usize,
    reset_firmware: bool,
    special_function: SpecialFunction,
    update_firmware: bool,
}

#[axum::debug_handler(state = Arc<Database>)]
#[tracing::instrument(ret, err)]
pub async fn display(
    TypedHeader(host): TypedHeader<Host>,
) -> Result<Json<DisplayResponse>, StatusCode> {
    let uuid = Uuid::now_v7();
    let now = Utc::now().second();
    Ok(Json(DisplayResponse {
        filename: format!("{uuid}.bmp"),
        // firmware_url: "http://example.com".parse().unwrap(),
        firmware_url: None,
        image_url: format_url(uuid, host)?,
        image_url_timeout: 0,
        refresh_rate: (60 - now) as usize,
        reset_firmware: false,
        special_function: SpecialFunction::None,
        update_firmware: false,
    }))
}

fn format_url(name: impl std::fmt::Display, host: Host) -> Result<Url, StatusCode> {
    match host.port() {
        Some(p) => format!("http://{}:{p}/app/{name}.bmp", host.hostname(),),
        None => format!("http://{}/app/{name}.bmp", host.hostname(),),
    }
    .parse()
    .map_err(|_| StatusCode::BAD_REQUEST)
}

#[tracing::instrument(err, skip(app))]
pub async fn app(State(app): State<Arc<RatatuiHandle>>) -> Result<impl IntoResponse, StatusCode> {
    let bytes = app.get_bytes().await.unwrap();
    Ok((StatusCode::OK, [(CONTENT_TYPE, "image/bmp")], bytes))
}
