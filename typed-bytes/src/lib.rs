use std::{borrow::Cow, marker::PhantomData};

use bincode::{Decode, Encode};
use redb::{Key, TableDefinition, Value};

#[derive(Debug)]
pub struct TypedBytes<'a, T> {
    inner: Cow<'a, [u8]>,
    _type: PhantomData<T>,
}

impl<T> AsRef<[u8]> for TypedBytes<'_, T> {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl<T> TypedBytes<'_, T>
where
    T: Encode + std::fmt::Debug,
{
    #[cfg_attr(feature = "tracing", tracing::instrument(err))]
    pub fn new(val: T) -> Result<TypedBytes<'static, T>, bincode::error::EncodeError> {
        let inner = bincode::encode_to_vec(val, bincode::config::standard())?;
        Ok(TypedBytes {
            inner: Cow::Owned(inner),
            _type: PhantomData,
        })
    }
}

impl<T> TypedBytes<'_, T>
where
    T: Decode<()>,
{
    pub fn decode(&self) -> Result<(T, usize), bincode::error::DecodeError> {
        bincode::decode_from_slice::<T, _>(&self.inner, bincode::config::standard())
    }
}

impl<T> Value for TypedBytes<'_, T>
where
    T: std::fmt::Debug,
{
    type SelfType<'a>
        = TypedBytes<'a, T>
    where
        Self: 'a;

    type AsBytes<'a>
        = &'a [u8]
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        TypedBytes {
            inner: Cow::Borrowed(data),
            _type: PhantomData,
        }
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'b,
    {
        value.inner.as_ref()
    }

    fn type_name() -> redb::TypeName {
        redb::TypeName::new(std::any::type_name::<Self>())
    }
}

impl<T> Key for TypedBytes<'_, T>
where
    T: std::fmt::Debug,
{
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        data1.cmp(data2)
    }
}

pub type TypedTableDefinition<'a, K, V> =
    TableDefinition<'a, TypedBytes<'static, K>, TypedBytes<'static, V>>;
