use api_key::{ApiKey, ParsedToken, TokenString};
use axum::{
    extract::{FromRef, FromRequestParts, OptionalFromRequestParts},
    http::{StatusCode, request::Parts},
};
use redb::{Database, ReadableDatabase};
use std::sync::Arc;
use typed_bytes::TypedBytes;

use crate::{
    extractor::device_id::DeviceId,
    tables::{ApiKeyDbRecord, ApiKeyId, KEYS_TABLE, TableErr},
};

#[derive(Debug)]
pub struct TokenExtractor(ParsedToken);

const ACCESS_TOKEN_HEADER: &str = "access-token";
impl<S> FromRequestParts<S> for TokenExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    #[tracing::instrument(err, skip(parts, _state))]
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let Some(val) = parts.headers.get(ACCESS_TOKEN_HEADER) else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        let Ok(s) = val.to_str() else {
            return Err(StatusCode::BAD_REQUEST);
        };

        let token = TokenString::new_from_str(s);
        let Ok(parsed) = ParsedToken::parse(&token) else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        Ok(TokenExtractor(parsed))
    }
}

impl<S> OptionalFromRequestParts<S> for TokenExtractor
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    #[tracing::instrument(err, skip(parts, state))]
    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        let res: Result<TokenExtractor, StatusCode> =
            <TokenExtractor as FromRequestParts<S>>::from_request_parts(parts, state).await;
        Ok(res.ok())
    }
}

#[must_use]
#[derive(Debug)]
pub struct ApiKeyExtractor {
    pub parsed_token: ParsedToken,
    pub api_key: ApiKey<DeviceId>,
}

impl<S> FromRequestParts<S> for ApiKeyExtractor
where
    Arc<Database>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = StatusCode;
    #[tracing::instrument(ret, err, skip(parts, state))]
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let token =
            <TokenExtractor as FromRequestParts<S>>::from_request_parts(parts, state).await?;
        let device_id = DeviceId::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let db = <Arc<Database>>::from_ref(state);
        let key_id = ApiKeyId(*token.0.uuid());
        let Ok(Ok(res)) =
            tokio::task::spawn_blocking(move || get_api_key_by_uuid(db, key_id)).await
        else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };
        let Some(record) = res else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        let Ok(()) = record.key.hashed_secret.compare_hash(&token.0, &device_id) else {
            return Err(StatusCode::UNAUTHORIZED);
        };

        Ok(ApiKeyExtractor {
            parsed_token: token.0,
            api_key: record.key,
        })
    }
}

fn get_api_key_by_uuid(
    db: Arc<Database>,
    key_id: ApiKeyId,
) -> Result<Option<ApiKeyDbRecord>, TableErr> {
    let read = db.begin_read().map_err(redb::Error::from)?;
    let table = read.open_table(KEYS_TABLE).map_err(redb::Error::from)?;
    let Some(res) = table
        .get(&TypedBytes::new(key_id)?)
        .map_err(redb::Error::from)?
    else {
        return Ok(None);
    };
    let (val, _) = res.value().decode()?;
    Ok(Some(val))
}
