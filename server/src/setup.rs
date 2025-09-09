use std::sync::Arc;

use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, header::ToStrError, request::Parts},
    response::IntoResponse,
};
use redb::Database;
use serde::Serialize;
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
enum DeviceIdErr {
    #[error("No header value was found for ID")]
    MissingHeader,
    #[error("The header value was not ascii {0:?}")]
    HeaderValue(#[from] ToStrError),
    #[error("The header value could not be parsed to mac address {0:?}")]
    ParseErr(#[from] macaddr::ParseError),
}

impl IntoResponse for DeviceIdErr {
    fn into_response(self) -> axum::response::Response {
        StatusCode::BAD_REQUEST.into_response()
    }
}

struct DeviceId(macaddr::MacAddr);

impl<S> FromRequestParts<S> for DeviceId
where
    S: Send + Sync,
{
    type Rejection = DeviceIdErr;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(val) = parts.headers.get("ID") else {
            return Err(DeviceIdErr::MissingHeader);
        };
        let s = val.to_str()?;
        let addr: macaddr::MacAddr = s.parse()?;
        Ok(DeviceId(addr))
    }
}

#[derive(Debug, Serialize)]
pub struct SetupResponse {
    api_key: String,
    friendly_id: String,
    image_url: Url,
    message: String,
}

#[axum::debug_handler(state = Arc<Database>)]
pub async fn setup_handler(State(db): State<Arc<Database>>) -> Json<SetupResponse> {
    Json(SetupResponse {
        api_key: "123456789".to_string(),
        friendly_id: "my_termnl".to_string(),
        image_url: "http://mira:2443/assets/setup.bmp".parse().unwrap(),
        message: "Hello world".to_string(),
    })
}
