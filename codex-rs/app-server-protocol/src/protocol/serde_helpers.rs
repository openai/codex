use std::path::PathBuf;

use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

#[allow(dead_code)]
pub fn deserialize_empty_path_as_none<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    let path = Option::<PathBuf>::deserialize(deserializer)?;
    Ok(path.filter(|path| !path.as_os_str().is_empty()))
}

#[allow(dead_code)]
pub fn deserialize_double_option<'de, T, D>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    serde_with::rust::double_option::deserialize(deserializer)
}

#[allow(dead_code)]
pub fn serialize_double_option<T, S>(
    value: &Option<Option<T>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: Serialize,
    S: Serializer,
{
    serde_with::rust::double_option::serialize(value, serializer)
}
