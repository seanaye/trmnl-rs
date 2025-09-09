use rand::RngCore;
use sha3::Digest;
use std::ops::Deref;
use uuid::Uuid;

#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ApiKey<Owner> {
    pub id: Uuid,
    pub secret_hash: SecretHash,
    pub owner: Owner,
    pub version: u8,
}

const SECRET_LENGTH: usize = 32;
const HASH_LENGTH: usize = 64;

#[non_exhaustive]
pub struct ApiKeyPair<Owner> {
    pub api_key: ApiKey<Owner>,
    pub token: TokenString,
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

        let token = TokenStringBuilder {
            prefix: "TRM",
            version: 1,
        }
        .build_token(&token_data);

        ApiKeyPair {
            api_key: ApiKey {
                id,
                secret_hash,
                owner,
                version,
            },
            token,
        }
    }
}

pub struct TokenString(String);

impl Deref for TokenString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

struct TokenStringBuilder {
    prefix: &'static str,
    version: u8,
}

impl TokenStringBuilder {
    fn build_token(&self, token_data: &[u8]) -> TokenString {
        let mut token = base32::encode(
            base32::Alphabet::Rfc4648Lower { padding: false },
            token_data,
        );
        token.insert_str(0, &format!("V{}", self.version));
        token.insert_str(0, self.prefix);
        TokenString(token)
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
