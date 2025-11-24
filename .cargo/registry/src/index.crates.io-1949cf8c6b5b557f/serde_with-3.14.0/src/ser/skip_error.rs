use super::impls::macros::foreach_map;
use crate::prelude::*;
#[cfg(feature = "hashbrown_0_14")]
use hashbrown_0_14::HashMap as HashbrownMap014;
#[cfg(feature = "hashbrown_0_15")]
use hashbrown_0_15::HashMap as HashbrownMap015;
#[cfg(feature = "indexmap_1")]
use indexmap_1::IndexMap;
#[cfg(feature = "indexmap_2")]
use indexmap_2::IndexMap as IndexMap2;

impl<T, U> SerializeAs<Vec<T>> for VecSkipError<U>
where
    U: SerializeAs<T>,
{
    fn serialize_as<S>(source: &Vec<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        Vec::<U>::serialize_as(source, serializer)
    }
}

macro_rules! map_skip_error_handling {
    ($tyorig:ident < K, V $(, $typaram:ident : $bound:ident)* >) => {
        impl<K, KAs, V, VAs $(, $typaram)*> SerializeAs<$tyorig<K, V $(, $typaram)*>> for MapSkipError<KAs, VAs>
        where
            KAs: SerializeAs<K>,
            VAs: SerializeAs<V>,
            $($typaram: ?Sized + $bound,)*
        {
            fn serialize_as<S>(value: &$tyorig<K, V $(, $typaram)*>, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                <$tyorig<KAs, VAs $(, $typaram)*>>::serialize_as(value, serializer)
            }
        }
    }
}
foreach_map!(map_skip_error_handling);
