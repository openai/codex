use sqlx::database::HasValueRef;
use sqlx::error::BoxDynError;
#[cfg(any(
    feature = "sqlx-mysql",
    feature = "sqlx-postgres",
    feature = "sqlx-sqlite"
))]
use sqlx::{
    database::HasArguments,
    encode::IsNull,
    Encode,
};
use sqlx::{
    Database,
    Decode,
    Type,
    Value,
    ValueRef,
};

use crate::{
    CompactString,
    ToCompactString,
};

#[cfg_attr(docsrs, doc(cfg(feature = "sqlx")))]
impl<DB> Type<DB> for CompactString
where
    DB: Database,
    for<'x> &'x str: Type<DB>,
{
    #[inline]
    fn type_info() -> <DB as Database>::TypeInfo {
        <&str as Type<DB>>::type_info()
    }
}

#[cfg_attr(docsrs, doc(cfg(feature = "sqlx")))]
impl<'r, DB> Decode<'r, DB> for CompactString
where
    DB: Database,
    for<'x> &'x str: Decode<'x, DB> + Type<DB>,
{
    fn decode(value: <DB as HasValueRef<'r>>::ValueRef) -> Result<Self, BoxDynError> {
        let value = value.to_owned();
        let value: &str = value.try_decode()?;
        Ok(value.try_to_compact_string()?)
    }
}

#[cfg(feature = "sqlx-mysql")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-mysql")))]
impl<'q> Encode<'q, sqlx::MySql> for CompactString {
    fn encode_by_ref(&self, buf: &mut <sqlx::MySql as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
        Encode::<'_, sqlx::MySql>::encode_by_ref(&self.as_str(), buf)
    }

    #[inline]
    fn produces(&self) -> Option<<sqlx::MySql as Database>::TypeInfo> {
        <&str as Encode<'_, sqlx::MySql>>::produces(&self.as_str())
    }

    #[inline]
    fn size_hint(&self) -> usize {
        <&str as Encode<'_, sqlx::MySql>>::size_hint(&self.as_str())
    }
}

#[cfg(feature = "sqlx-postgres")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-postgres")))]
impl<'q> Encode<'q, sqlx::Postgres> for CompactString {
    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Postgres as HasArguments<'q>>::ArgumentBuffer,
    ) -> IsNull {
        Encode::<'_, sqlx::Postgres>::encode_by_ref(&self.as_str(), buf)
    }

    #[inline]
    fn produces(&self) -> Option<<sqlx::Postgres as Database>::TypeInfo> {
        <&str as Encode<'_, sqlx::Postgres>>::produces(&self.as_str())
    }

    #[inline]
    fn size_hint(&self) -> usize {
        <&str as Encode<'_, sqlx::Postgres>>::size_hint(&self.as_str())
    }
}

#[cfg(feature = "sqlx-sqlite")]
#[cfg_attr(docsrs, doc(cfg(feature = "sqlx-sqlite")))]
impl<'q> Encode<'q, sqlx::Sqlite> for CompactString {
    fn encode(self, buf: &mut <sqlx::Sqlite as HasArguments<'q>>::ArgumentBuffer) -> IsNull {
        Encode::<'_, sqlx::Sqlite>::encode(self.into_string(), buf)
    }

    fn encode_by_ref(
        &self,
        buf: &mut <sqlx::Sqlite as HasArguments<'q>>::ArgumentBuffer,
    ) -> IsNull {
        Encode::<'_, sqlx::Sqlite>::encode(alloc::string::String::from(self.as_str()), buf)
    }

    #[inline]
    fn produces(&self) -> Option<<sqlx::Sqlite as Database>::TypeInfo> {
        <&str as Encode<'_, sqlx::Sqlite>>::produces(&self.as_str())
    }

    #[inline]
    fn size_hint(&self) -> usize {
        <&str as Encode<'_, sqlx::Sqlite>>::size_hint(&self.as_str())
    }
}
