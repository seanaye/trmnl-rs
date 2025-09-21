use nom::{Parser, bytes::complete::tag, character::complete::char, combinator::eof};
use rand::RngCore;
use sha3::Digest;
use std::{borrow::Cow, ops::Deref};
use thiserror::Error;
use uuid::Uuid;

use crate::parse::{decimal_u8, secret_parse, uuid_parse};

#[non_exhaustive]
#[derive(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ApiKey<Owner> {
    pub id: Uuid,
    pub hashed_secret: SecretHash,
    pub owner: Owner,
    pub version: u8,
}

const SECRET_LENGTH: usize = 32;
const HASH_LENGTH: usize = 64;

#[non_exhaustive]
pub struct ApiKeyPair<Owner> {
    pub api_key: ApiKey<Owner>,
    pub token: TokenString<'static>,
}

impl<Owner> ApiKeyPair<Owner>
where
    Owner: AsRef<[u8]>,
{
    pub fn new(owner: Owner) -> ApiKeyPair<Owner> {
        let id = Uuid::now_v7();
        let version = 1u8;
        let mut token_data = [0u8; 16 + SECRET_LENGTH];
        token_data[..16].copy_from_slice(id.as_bytes());
        rand::rng().fill_bytes(&mut token_data[16..]);

        let secret_hash = KeyMaterial {
            key_id: &id,
            version,
            owner: &owner,
            secret: &token_data[16..],
        }
        .create_hash();

        let token = TokenStringParams::new().build_token(&token_data);

        ApiKeyPair {
            api_key: ApiKey {
                id,
                hashed_secret: secret_hash,
                owner,
                version,
            },
            token,
        }
    }
}

#[derive(Clone)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(transparent)
)]
pub struct TokenString<'a>(Cow<'a, str>);

impl<'a> TokenString<'a> {
    pub fn new_from_string(s: String) -> TokenString<'static> {
        TokenString(Cow::Owned(s))
    }

    pub fn new_from_str(s: &'a str) -> Self {
        Self(Cow::Borrowed(s))
    }
}

impl std::fmt::Debug for TokenString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TokenString").field(&"SENSITIVE").finish()
    }
}

impl Deref for TokenString<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

struct TokenStringParams {
    prefix: &'static str,
    version: u8,
}

impl TokenStringParams {
    fn new() -> Self {
        TokenStringParams {
            prefix: "TRM",
            version: 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum ParseErr {
    #[error("Recevied an invalid version. Expected {expected} got {received}")]
    InvalidVersion { received: u8, expected: u8 },
    #[error("{0:?}")]
    NomStr(#[from] nom::Err<nom::error::Error<String>>),
    #[error("No uuid or secret bytes")]
    NoBytes,
    #[error("Invalid bytes {0}")]
    NomBytes(#[from] nom::Err<nom::error::Error<Vec<u8>>>),
}

pub struct Secret([u8; SECRET_LENGTH]);

#[non_exhaustive]
pub struct ParsedToken {
    version: u8,
    uuid: Uuid,
    secret: Secret,
}

impl ParsedToken {
    pub fn parse(token: &TokenString) -> Result<ParsedToken, ParseErr> {
        TokenStringParams::new().parse_token(token)
    }

    pub fn version(&self) -> u8 {
        self.version
    }

    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }
}

const ALPHABET: base32::Alphabet = base32::Alphabet::Rfc4648Lower { padding: false };

impl TokenStringParams {
    fn build_token(&self, token_data: &[u8]) -> TokenString<'static> {
        let mut token = base32::encode(ALPHABET, token_data);
        token.insert_str(0, &format!("V{}", self.version));
        token.insert_str(0, self.prefix);
        TokenString(Cow::Owned(token))
    }

    fn parse_token(&self, token: &TokenString) -> Result<ParsedToken, ParseErr> {
        let (rest, (_, _, version)) = (tag(self.prefix), char('V'), decimal_u8)
            .parse(token)
            .map_err(|e| e.map(|e| e.cloned()))?;
        if version != self.version {
            return Err(ParseErr::InvalidVersion {
                received: version,
                expected: self.version,
            });
        }
        let bytes = base32::decode(ALPHABET, rest).ok_or(ParseErr::NoBytes)?;
        let (_, (uuid, secret, _)) = (uuid_parse, secret_parse, eof)
            .parse(bytes.as_slice())
            .map_err(|e| e.map(|e| e.cloned()))?;

        Ok(ParsedToken {
            version,
            uuid,
            secret,
        })
    }
}

mod parse {
    use nom::{
        IResult, Parser,
        bytes::{complete::take_while_m_n, take},
        combinator::map_res,
    };
    use uuid::Uuid;

    use crate::{SECRET_LENGTH, Secret};

    fn from_decimal(s: &str) -> Result<u8, std::num::ParseIntError> {
        s.parse::<u8>()
    }

    pub fn decimal_u8(s: &str) -> IResult<&str, u8> {
        map_res(
            take_while_m_n(1, 3, |c: char| c.is_ascii_digit()),
            from_decimal,
        )
        .parse(s)
    }

    pub fn uuid_parse(s: &[u8]) -> IResult<&[u8], Uuid> {
        map_res(take(16usize), <[u8; 16]>::try_from)
            .map(Uuid::from_bytes)
            .parse(s)
    }

    pub fn secret_parse(s: &[u8]) -> IResult<&[u8], Secret> {
        map_res(take(SECRET_LENGTH), <[u8; SECRET_LENGTH]>::try_from)
            .map(Secret)
            .parse(s)
    }
}

struct KeyMaterial<'a, Owner> {
    key_id: &'a Uuid,
    version: u8,
    owner: &'a Owner,
    secret: &'a [u8],
}

#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SecretHash(
    #[cfg_attr(feature = "serde", serde(with = "serde_big_array::BigArray"))] [u8; HASH_LENGTH],
);

#[derive(Debug, Error)]
#[error("The input hash did not match the expected hash")]
pub struct InvalidHash;

impl SecretHash {
    pub fn compare_hash<Owner>(&self, token: &ParsedToken, owner: &Owner) -> Result<(), InvalidHash>
    where
        Owner: AsRef<[u8]>,
    {
        let hash = KeyMaterial {
            key_id: &token.uuid,
            version: token.version,
            owner,
            secret: &token.secret.0,
        }
        .create_hash();

        constant_time_eq::constant_time_eq(&hash.0, &self.0)
            .then_some(())
            .ok_or(InvalidHash)
    }
}

impl std::fmt::Debug for SecretHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SecretHash").field(&"REDACTED").finish()
    }
}

impl Deref for SecretHash {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, Owner> KeyMaterial<'a, Owner>
where
    Owner: AsRef<[u8]>,
{
    fn create_hash(self) -> SecretHash {
        let hasher = sha3::Sha3_512::new();

        let mut output = [0; HASH_LENGTH];

        output.copy_from_slice(
            &hasher
                .chain_update(self.key_id.as_bytes())
                .chain_update([self.version])
                .chain_update(self.owner)
                .chain_update(self.secret)
                .finalize(),
        );

        SecretHash(output)
    }
}
