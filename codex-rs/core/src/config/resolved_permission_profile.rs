use codex_config::Constrained;
use codex_config::ConstraintResult;
use codex_protocol::models::ActivePermissionProfile;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_READ_ONLY;
use codex_protocol::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
use codex_protocol::models::PermissionProfile;
use codex_utils_absolute_path::AbsolutePathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BuiltInPermissionProfileId {
    ReadOnly,
    Workspace,
    DangerFullAccess,
}

impl BuiltInPermissionProfileId {
    fn from_str(id: &str) -> Option<Self> {
        match id {
            BUILT_IN_PERMISSION_PROFILE_READ_ONLY => Some(Self::ReadOnly),
            BUILT_IN_PERMISSION_PROFILE_WORKSPACE => Some(Self::Workspace),
            BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS => Some(Self::DangerFullAccess),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => BUILT_IN_PERMISSION_PROFILE_READ_ONLY,
            Self::Workspace => BUILT_IN_PERMISSION_PROFILE_WORKSPACE,
            Self::DangerFullAccess => BUILT_IN_PERMISSION_PROFILE_DANGER_FULL_ACCESS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvedPermissionProfile {
    Legacy(LegacyPermissionProfile),
    BuiltIn(BuiltInPermissionProfile),
    Named(NamedPermissionProfile),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyPermissionProfile {
    permission_profile: PermissionProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuiltInPermissionProfile {
    id: BuiltInPermissionProfileId,
    extends: Option<String>,
    permission_profile: PermissionProfile,
    profile_workspace_roots: Vec<AbsolutePathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NamedPermissionProfile {
    id: String,
    extends: Option<String>,
    permission_profile: PermissionProfile,
    profile_workspace_roots: Vec<AbsolutePathBuf>,
}

impl ResolvedPermissionProfile {
    pub(crate) fn from_active_profile(
        permission_profile: PermissionProfile,
        active_permission_profile: Option<ActivePermissionProfile>,
        profile_workspace_roots: Vec<AbsolutePathBuf>,
    ) -> Self {
        let Some(active_permission_profile) = active_permission_profile else {
            return Self::legacy(permission_profile);
        };

        let ActivePermissionProfile { id, extends } = active_permission_profile;
        if let Some(built_in_id) = BuiltInPermissionProfileId::from_str(&id) {
            Self::BuiltIn(BuiltInPermissionProfile {
                id: built_in_id,
                extends,
                permission_profile,
                profile_workspace_roots,
            })
        } else {
            Self::Named(NamedPermissionProfile {
                id,
                extends,
                permission_profile,
                profile_workspace_roots,
            })
        }
    }

    pub(crate) fn legacy(permission_profile: PermissionProfile) -> Self {
        Self::Legacy(LegacyPermissionProfile { permission_profile })
    }

    pub(crate) fn permission_profile(&self) -> &PermissionProfile {
        match self {
            Self::Legacy(profile) => &profile.permission_profile,
            Self::BuiltIn(profile) => &profile.permission_profile,
            Self::Named(profile) => &profile.permission_profile,
        }
    }

    pub(crate) fn active_permission_profile(&self) -> Option<ActivePermissionProfile> {
        match self {
            Self::Legacy(_) => None,
            Self::BuiltIn(profile) => Some(ActivePermissionProfile {
                id: profile.id.as_str().to_string(),
                extends: profile.extends.clone(),
            }),
            Self::Named(profile) => Some(ActivePermissionProfile {
                id: profile.id.clone(),
                extends: profile.extends.clone(),
            }),
        }
    }

    pub(crate) fn profile_workspace_roots(&self) -> &[AbsolutePathBuf] {
        match self {
            Self::Legacy(_) => &[],
            Self::BuiltIn(profile) => &profile.profile_workspace_roots,
            Self::Named(profile) => &profile.profile_workspace_roots,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PermissionProfileState {
    resolved_permission_profile: Constrained<ResolvedPermissionProfile>,
}

impl PermissionProfileState {
    pub(crate) fn from_constrained_legacy(
        constrained_permission_profile: Constrained<PermissionProfile>,
    ) -> ConstraintResult<Self> {
        let resolved =
            ResolvedPermissionProfile::legacy(constrained_permission_profile.get().clone());
        Self::from_constrained_resolved(constrained_permission_profile, resolved)
    }

    pub(crate) fn from_constrained_active_profile(
        constrained_permission_profile: Constrained<PermissionProfile>,
        active_permission_profile: Option<ActivePermissionProfile>,
        profile_workspace_roots: Vec<AbsolutePathBuf>,
    ) -> ConstraintResult<Self> {
        let resolved = ResolvedPermissionProfile::from_active_profile(
            constrained_permission_profile.get().clone(),
            active_permission_profile,
            profile_workspace_roots,
        );
        Self::from_constrained_resolved(constrained_permission_profile, resolved)
    }

    pub(crate) fn from_constrained_resolved(
        constrained_permission_profile: Constrained<PermissionProfile>,
        resolved_permission_profile: ResolvedPermissionProfile,
    ) -> ConstraintResult<Self> {
        let permission_profile_constraint = constrained_permission_profile;
        let resolved_permission_profile = Constrained::new(
            resolved_permission_profile,
            move |candidate: &ResolvedPermissionProfile| {
                permission_profile_constraint.can_set(candidate.permission_profile())
            },
        )?;
        Ok(Self {
            resolved_permission_profile,
        })
    }

    pub(crate) fn permission_profile(&self) -> &PermissionProfile {
        self.resolved_permission_profile.get().permission_profile()
    }

    pub(crate) fn active_permission_profile(&self) -> Option<ActivePermissionProfile> {
        self.resolved_permission_profile
            .get()
            .active_permission_profile()
    }

    pub(crate) fn profile_workspace_roots(&self) -> &[AbsolutePathBuf] {
        self.resolved_permission_profile
            .get()
            .profile_workspace_roots()
    }

    pub(crate) fn can_set_legacy_permission_profile(
        &self,
        permission_profile: &PermissionProfile,
    ) -> ConstraintResult<()> {
        let candidate = ResolvedPermissionProfile::legacy(permission_profile.clone());
        self.resolved_permission_profile.can_set(&candidate)
    }

    pub(crate) fn set_legacy_permission_profile(
        &mut self,
        permission_profile: PermissionProfile,
    ) -> ConstraintResult<()> {
        self.resolved_permission_profile
            .set(ResolvedPermissionProfile::legacy(permission_profile))
    }

    pub(crate) fn set_active_permission_profile(
        &mut self,
        permission_profile: PermissionProfile,
        active_permission_profile: Option<ActivePermissionProfile>,
        profile_workspace_roots: Vec<AbsolutePathBuf>,
    ) -> ConstraintResult<()> {
        let candidate = ResolvedPermissionProfile::from_active_profile(
            permission_profile,
            active_permission_profile,
            profile_workspace_roots,
        );
        self.resolved_permission_profile.set(candidate)
    }
}
