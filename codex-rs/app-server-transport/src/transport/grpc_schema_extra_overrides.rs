use super::*;

fn decode_boolean_network_access(
    payload: Option<proto::V2SandboxPolicyNetworkAccess>,
    field: &'static str,
) -> Result<bool, Status> {
    let Some(payload) = payload else {
        return Ok(false);
    };
    match payload.value.ok_or_else(|| missing(field))? {
        proto::v2_sandbox_policy_network_access::Value::BoolValue(value) => Ok(value),
        proto::v2_sandbox_policy_network_access::Value::StringValue(_) => {
            Err(invalid(field, "expected a boolean network access value"))
        }
    }
}

fn encode_boolean_network_access(value: bool) -> proto::V2SandboxPolicyNetworkAccess {
    proto::V2SandboxPolicyNetworkAccess {
        value: Some(proto::v2_sandbox_policy_network_access::Value::BoolValue(
            value,
        )),
    }
}

fn decode_external_network_access(
    payload: Option<proto::V2SandboxPolicyNetworkAccess>,
) -> Result<codex_app_server_protocol::NetworkAccess, Status> {
    let Some(payload) = payload else {
        return Ok(codex_app_server_protocol::NetworkAccess::Restricted);
    };
    match payload
        .value
        .ok_or_else(|| missing("SandboxPolicy.networkAccess"))?
    {
        proto::v2_sandbox_policy_network_access::Value::StringValue(value) => {
            match value.as_str() {
                "restricted" => Ok(codex_app_server_protocol::NetworkAccess::Restricted),
                "enabled" => Ok(codex_app_server_protocol::NetworkAccess::Enabled),
                value => Err(invalid(
                    "SandboxPolicy.networkAccess",
                    format!("unknown value `{value}`"),
                )),
            }
        }
        proto::v2_sandbox_policy_network_access::Value::BoolValue(_) => Err(invalid(
            "SandboxPolicy.networkAccess",
            "expected a string network access value",
        )),
    }
}

fn encode_external_network_access(
    value: codex_app_server_protocol::NetworkAccess,
) -> proto::V2SandboxPolicyNetworkAccess {
    let value = match value {
        codex_app_server_protocol::NetworkAccess::Restricted => "restricted",
        codex_app_server_protocol::NetworkAccess::Enabled => "enabled",
    };
    proto::V2SandboxPolicyNetworkAccess {
        value: Some(proto::v2_sandbox_policy_network_access::Value::StringValue(
            value.to_owned(),
        )),
    }
}

fn decode_absolute_path<T>(
    payload: proto::V2AbsolutePathBuf,
    field: &'static str,
) -> Result<T, Status>
where
    T: TryFrom<String>,
    T::Error: std::fmt::Display,
{
    decode_newtype_string(payload.value, field)
}

fn encode_absolute_path(
    value: impl AsRef<std::path::Path>,
    field: &'static str,
) -> Result<proto::V2AbsolutePathBuf, Status> {
    let value = value
        .as_ref()
        .to_str()
        .ok_or_else(|| encode_error(field, "path is not valid UTF-8"))?
        .to_owned();
    Ok(proto::V2AbsolutePathBuf { value })
}

impl DirectSchemaProto<proto::V2SandboxPolicy> for codex_app_server_protocol::SandboxPolicy {
    fn decode_schema(payload: proto::V2SandboxPolicy) -> Result<Self, Status> {
        match payload.r#type.as_str() {
            "dangerFullAccess" => Ok(Self::DangerFullAccess),
            "readOnly" => Ok(Self::ReadOnly {
                network_access: decode_boolean_network_access(
                    payload.network_access,
                    "SandboxPolicy.networkAccess",
                )?,
            }),
            "externalSandbox" => Ok(Self::ExternalSandbox {
                network_access: decode_external_network_access(payload.network_access)?,
            }),
            "workspaceWrite" => Ok(Self::WorkspaceWrite {
                writable_roots: payload
                    .writable_roots
                    .map(|roots| roots.values)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|value| decode_absolute_path(value, "SandboxPolicy.writableRoots[]"))
                    .collect::<Result<_, _>>()?,
                network_access: decode_boolean_network_access(
                    payload.network_access,
                    "SandboxPolicy.networkAccess",
                )?,
                exclude_tmpdir_env_var: payload.exclude_tmpdir_env_var.unwrap_or(false),
                exclude_slash_tmp: payload.exclude_slash_tmp.unwrap_or(false),
            }),
            value => Err(invalid(
                "SandboxPolicy.type",
                format!("unknown value `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2SandboxPolicy, Status> {
        match self {
            Self::DangerFullAccess => Ok(proto::V2SandboxPolicy {
                r#type: "dangerFullAccess".to_owned(),
                ..Default::default()
            }),
            Self::ReadOnly { network_access } => Ok(proto::V2SandboxPolicy {
                r#type: "readOnly".to_owned(),
                network_access: Some(encode_boolean_network_access(network_access)),
                ..Default::default()
            }),
            Self::ExternalSandbox { network_access } => Ok(proto::V2SandboxPolicy {
                r#type: "externalSandbox".to_owned(),
                network_access: Some(encode_external_network_access(network_access)),
                ..Default::default()
            }),
            Self::WorkspaceWrite {
                writable_roots,
                network_access,
                exclude_tmpdir_env_var,
                exclude_slash_tmp,
            } => Ok(proto::V2SandboxPolicy {
                r#type: "workspaceWrite".to_owned(),
                network_access: Some(encode_boolean_network_access(network_access)),
                exclude_slash_tmp: Some(exclude_slash_tmp),
                exclude_tmpdir_env_var: Some(exclude_tmpdir_env_var),
                writable_roots: Some(proto::V2SandboxPolicyWritableRootsList {
                    values: writable_roots
                        .into_iter()
                        .map(|value| encode_absolute_path(value, "SandboxPolicy.writableRoots[]"))
                        .collect::<Result<_, _>>()?,
                }),
            }),
        }
    }
}

fn decode_legacy_decision_literal(
    value: String,
    expected: &'static str,
    decision: codex_protocol::protocol::ReviewDecision,
) -> Result<codex_protocol::protocol::ReviewDecision, Status> {
    if value == expected {
        Ok(decision)
    } else {
        Err(invalid(
            "ReviewDecision",
            format!("expected `{expected}`, got `{value}`"),
        ))
    }
}

impl DirectSchemaProto<proto::LegacyReviewDecision> for codex_protocol::protocol::ReviewDecision {
    fn decode_schema(payload: proto::LegacyReviewDecision) -> Result<Self, Status> {
        use proto::legacy_review_decision::Value;

        match payload
            .value
            .ok_or_else(|| missing("ReviewDecision.value"))?
        {
            Value::StringValue(value) => {
                decode_legacy_decision_literal(value, "approved", Self::Approved)
            }
            Value::ApprovedExecpolicyAmendmentReviewDecision(payload) => {
                let amendment = payload
                    .approved_execpolicy_amendment
                    .ok_or_else(|| missing("ReviewDecision.approvedExecpolicyAmendment"))?;
                Ok(Self::ApprovedExecpolicyAmendment {
                    proposed_execpolicy_amendment: DirectSchemaProto::decode_schema(
                        amendment.proposed_execpolicy_amendment,
                    )?,
                })
            }
            Value::StringValue3(value) => decode_legacy_decision_literal(
                value,
                "approved_for_session",
                Self::ApprovedForSession,
            ),
            Value::NetworkPolicyAmendmentReviewDecision(payload) => {
                let wrapper = payload
                    .network_policy_amendment
                    .ok_or_else(|| missing("ReviewDecision.networkPolicyAmendment"))?;
                let amendment = wrapper
                    .network_policy_amendment
                    .ok_or_else(|| missing("ReviewDecision.networkPolicyAmendment.value"))?;
                Ok(Self::NetworkPolicyAmendment {
                    network_policy_amendment: DirectSchemaProto::decode_schema(amendment)?,
                })
            }
            Value::StringValue5(value) => {
                decode_legacy_decision_literal(value, "denied", Self::Denied)
            }
            Value::StringValue6(value) => {
                decode_legacy_decision_literal(value, "timed_out", Self::TimedOut)
            }
            Value::StringValue7(value) => {
                decode_legacy_decision_literal(value, "abort", Self::Abort)
            }
        }
    }

    fn encode_schema(self) -> Result<proto::LegacyReviewDecision, Status> {
        use proto::legacy_review_decision::Value;

        let value = match self {
            Self::Approved => Value::StringValue("approved".to_owned()),
            Self::ApprovedExecpolicyAmendment {
                proposed_execpolicy_amendment,
            } => Value::ApprovedExecpolicyAmendmentReviewDecision(
                proto::LegacyReviewDecisionApprovedExecpolicyAmendmentReviewDecision {
                    approved_execpolicy_amendment: Some(
                        proto::LegacyReviewDecisionApprovedExecpolicyAmendmentReviewDecisionApprovedExecpolicyAmendment {
                            proposed_execpolicy_amendment:
                                DirectSchemaProto::encode_schema(
                                    proposed_execpolicy_amendment,
                                )?,
                        },
                    ),
                },
            ),
            Self::ApprovedForSession => {
                Value::StringValue3("approved_for_session".to_owned())
            }
            Self::NetworkPolicyAmendment {
                network_policy_amendment,
            } => Value::NetworkPolicyAmendmentReviewDecision(
                proto::LegacyReviewDecisionNetworkPolicyAmendmentReviewDecision {
                    network_policy_amendment: Some(
                        proto::LegacyReviewDecisionNetworkPolicyAmendmentReviewDecisionNetworkPolicyAmendment {
                            network_policy_amendment: Some(
                                DirectSchemaProto::encode_schema(
                                    network_policy_amendment,
                                )?,
                            ),
                        },
                    ),
                },
            ),
            Self::Denied => Value::StringValue5("denied".to_owned()),
            Self::TimedOut => Value::StringValue6("timed_out".to_owned()),
            Self::Abort => Value::StringValue7("abort".to_owned()),
        };
        Ok(proto::LegacyReviewDecision { value: Some(value) })
    }
}

fn decode_optional_path(
    value: Option<String>,
    field: &'static str,
) -> Result<Option<std::path::PathBuf>, Status> {
    value.map(|value| decode_string(value, field)).transpose()
}

fn encode_optional_path(
    value: Option<std::path::PathBuf>,
    field: &'static str,
) -> Result<Option<String>, Status> {
    value
        .map(|value| {
            value
                .into_os_string()
                .into_string()
                .map_err(|_| encode_error(field, "path is not valid UTF-8"))
        })
        .transpose()
}

impl DirectSchemaProto<proto::V2FileSystemSpecialPath>
    for codex_app_server_protocol::FileSystemSpecialPath
{
    fn decode_schema(payload: proto::V2FileSystemSpecialPath) -> Result<Self, Status> {
        match payload.kind.as_str() {
            "root" => Ok(Self::Root),
            "minimal" => Ok(Self::Minimal),
            "project_roots" | "current_working_directory" => Ok(Self::ProjectRoots {
                subpath: decode_optional_path(payload.subpath, "FileSystemSpecialPath.subpath")?,
            }),
            "tmpdir" => Ok(Self::Tmpdir),
            "slash_tmp" => Ok(Self::SlashTmp),
            "unknown" => Ok(Self::Unknown {
                path: payload
                    .path
                    .ok_or_else(|| missing("FileSystemSpecialPath.path"))?,
                subpath: decode_optional_path(payload.subpath, "FileSystemSpecialPath.subpath")?,
            }),
            value => Err(invalid(
                "FileSystemSpecialPath.kind",
                format!("unknown value `{value}`"),
            )),
        }
    }

    fn encode_schema(self) -> Result<proto::V2FileSystemSpecialPath, Status> {
        match self {
            Self::Root => Ok(proto::V2FileSystemSpecialPath {
                kind: "root".to_owned(),
                ..Default::default()
            }),
            Self::Minimal => Ok(proto::V2FileSystemSpecialPath {
                kind: "minimal".to_owned(),
                ..Default::default()
            }),
            Self::ProjectRoots { subpath } => Ok(proto::V2FileSystemSpecialPath {
                kind: "project_roots".to_owned(),
                subpath: encode_optional_path(subpath, "FileSystemSpecialPath.subpath")?,
                ..Default::default()
            }),
            Self::Tmpdir => Ok(proto::V2FileSystemSpecialPath {
                kind: "tmpdir".to_owned(),
                ..Default::default()
            }),
            Self::SlashTmp => Ok(proto::V2FileSystemSpecialPath {
                kind: "slash_tmp".to_owned(),
                ..Default::default()
            }),
            Self::Unknown { path, subpath } => Ok(proto::V2FileSystemSpecialPath {
                kind: "unknown".to_owned(),
                path: Some(path),
                subpath: encode_optional_path(subpath, "FileSystemSpecialPath.subpath")?,
            }),
        }
    }
}

impl DirectSchemaProto<proto::V2TextElement> for codex_app_server_protocol::TextElement {
    fn decode_schema(payload: proto::V2TextElement) -> Result<Self, Status> {
        Ok(Self::new(
            DirectSchemaProto::decode_schema(
                payload
                    .byte_range
                    .ok_or_else(|| missing("TextElement.byteRange"))?,
            )?,
            payload.placeholder,
        ))
    }

    fn encode_schema(self) -> Result<proto::V2TextElement, Status> {
        let placeholder = self.placeholder().map(str::to_owned);
        Ok(proto::V2TextElement {
            byte_range: Some(DirectSchemaProto::encode_schema(self.byte_range)?),
            placeholder,
        })
    }
}
