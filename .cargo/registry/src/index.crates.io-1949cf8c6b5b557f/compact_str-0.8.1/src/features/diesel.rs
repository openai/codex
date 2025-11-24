#![cfg_attr(docsrs, doc(cfg(feature = "diesel")))]

// Copied and adapted from
// <https://github.com/diesel-rs/diesel/blob/ab70dd5ed1f96926a3e8d98ab42636eaac1e1594/diesel/src/type_impls/primitives.rs>

use diesel::{
    backend,
    deserialize,
    expression,
    serialize,
    sql_types,
};

use crate::CompactString;

#[derive(expression::AsExpression, deserialize::FromSqlRow)]
#[diesel(foreign_derive)]
#[diesel(sql_type = sql_types::Text)]
#[allow(dead_code)]
struct CompactStringProxy(CompactString);

impl<ST, DB> deserialize::FromSql<ST, DB> for CompactString
where
    DB: backend::Backend,
    *const str: deserialize::FromSql<ST, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> deserialize::Result<Self> {
        let str_ptr = <*const str as deserialize::FromSql<ST, DB>>::from_sql(bytes)?;
        if !str_ptr.is_null() {
            // SAFETY: We just checked that `str_ptr` is not null, and `from_sql()` should return
            // a valid pointer to an `str`.
            let string = unsafe { &*str_ptr };
            Ok(string.into())
        } else {
            Ok(CompactString::new(""))
        }
    }
}

impl<DB> serialize::ToSql<sql_types::Text, DB> for CompactString
where
    DB: backend::Backend,
    str: serialize::ToSql<sql_types::Text, DB>,
{
    fn to_sql<'b>(&'b self, out: &mut serialize::Output<'b, '_, DB>) -> serialize::Result {
        self.as_str().to_sql(out)
    }
}
