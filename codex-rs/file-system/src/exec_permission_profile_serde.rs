//! Uses executor file URIs for sandbox permissions instead of the profile's legacy native paths.

use crate::ExecPermissionProfile;
use codex_protocol::models::PermissionProfile;
use serde::Deserialize;
use serde::Deserializer;
use serde::Serialize;
use serde::Serializer;

pub(crate) fn serialize<S>(value: &PermissionProfile, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    ExecPermissionProfile::from(value.clone()).serialize(serializer)
}

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<PermissionProfile, D::Error>
where
    D: Deserializer<'de>,
{
    ExecPermissionProfile::deserialize(deserializer).map(PermissionProfile::from)
}
