macro_rules! impl_str_basic {
    ($type:ty) => {
        impl zvariant::Basic for $type {
            const SIGNATURE_CHAR: char = <zvariant::Str<'_>>::SIGNATURE_CHAR;
            const SIGNATURE_STR: &'static str = <zvariant::Str<'_>>::SIGNATURE_STR;

            fn alignment(format: zvariant::serialized::Format) -> usize {
                <zvariant::Str<'_>>::alignment(format)
            }
        }
    };
}

macro_rules! impl_try_from {
    (ty: $type:ty, owned_ty: $owned_type:ty, validate_fn: $validate_fn:ident, try_from: [$($from:ty),*],) => {
        $(
            impl<'s> TryFrom<$from> for $type {
                type Error = Error;

                fn try_from(value: $from) -> Result<Self> {
                    let value = Str::from(value);
                    $validate_fn(value.as_str())?;
                    Ok(Self(value))
                }
            }

            impl<'s> TryFrom<$from> for $owned_type {
                type Error = Error;

                fn try_from(value: $from) -> Result<Self> {
                    Ok(Self::from(<$type>::try_from(value)?))
                }
            }
        )*
    };
}

pub(crate) use impl_str_basic;
pub(crate) use impl_try_from;
