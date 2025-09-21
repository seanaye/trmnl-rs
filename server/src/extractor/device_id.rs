use axum::{
    extract::FromRequestParts,
    http::{StatusCode, header::ToStrError, request::Parts},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(#[serde(with = "remote_macaddr::RemoteMacAddr")] macaddr::MacAddr);

mod remote_macaddr {
    use super::*;

    #[derive(Serialize, Deserialize)]
    #[serde(remote = "macaddr::MacAddr")]
    #[serde(untagged)]
    pub enum RemoteMacAddr {
        V6(macaddr::MacAddr6),
        V8(macaddr::MacAddr8),
    }
}

impl AsRef<[u8]> for DeviceId {
    fn as_ref(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

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

#[derive(Debug, Error)]
pub enum DeviceIdErr {
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
