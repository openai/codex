use codex_utils_absolute_path::AbsolutePathBuf;

use crate::models::ActivePermissionProfile;
use crate::models::PermissionProfile;

/// Trusted snapshot of a resolved permission profile.
///
/// Keeps the concrete permissions, optional active profile identity, and
/// profile-defined workspace roots together for atomic installation. Callers
/// handling user-selected profile ids must resolve those ids before
/// constructing a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionProfileSnapshot {
    permission_profile: PermissionProfile,
    active_permission_profile: Option<ActivePermissionProfile>,
    profile_workspace_roots: Vec<AbsolutePathBuf>,
}

impl PermissionProfileSnapshot {
    /// Create a snapshot with no active profile identity.
    ///
    /// Use this only for legacy data or local overrides that genuinely do not
    /// have a named or built-in profile identity.
    pub fn legacy(permission_profile: PermissionProfile) -> Self {
        Self {
            permission_profile,
            active_permission_profile: None,
            profile_workspace_roots: Vec::new(),
        }
    }

    /// Create a snapshot for an already-resolved active profile.
    pub fn active(
        permission_profile: PermissionProfile,
        active_permission_profile: ActivePermissionProfile,
    ) -> Self {
        Self::active_with_profile_workspace_roots(
            permission_profile,
            active_permission_profile,
            Vec::new(),
        )
    }

    /// Create a snapshot for an active profile and its declared roots.
    ///
    /// Profile roots remain distinct from turn-scoped runtime workspace roots.
    pub fn active_with_profile_workspace_roots(
        permission_profile: PermissionProfile,
        active_permission_profile: ActivePermissionProfile,
        profile_workspace_roots: Vec<AbsolutePathBuf>,
    ) -> Self {
        Self {
            permission_profile,
            active_permission_profile: Some(active_permission_profile),
            profile_workspace_roots,
        }
    }

    /// Reconstruct a trusted snapshot from already-resolved session state.
    pub fn from_session_snapshot(
        permission_profile: PermissionProfile,
        active_permission_profile: Option<ActivePermissionProfile>,
    ) -> Self {
        match active_permission_profile {
            Some(active_permission_profile) => {
                Self::active(permission_profile, active_permission_profile)
            }
            None => Self::legacy(permission_profile),
        }
    }

    /// Borrow the concrete permissions captured in this snapshot.
    pub fn permission_profile(&self) -> &PermissionProfile {
        &self.permission_profile
    }

    /// Return the active profile identity captured in this snapshot, if any.
    pub fn active_permission_profile(&self) -> Option<ActivePermissionProfile> {
        self.active_permission_profile.clone()
    }

    /// Borrow profile-declared workspace roots captured in this snapshot.
    pub fn profile_workspace_roots(&self) -> &[AbsolutePathBuf] {
        &self.profile_workspace_roots
    }
}
