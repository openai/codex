// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::compat::private::OptionCompatLevelMut;
use crate::{
    uapi, Access, AddRuleError, AddRulesError, CompatError, CompatLevel, CompatResult, CompatState,
    Compatible, HandleAccessError, HandleAccessesError, HandledAccess, PrivateHandledAccess,
    PrivateRule, Rule, Ruleset, RulesetCreated, TailoredCompatLevel, TryCompat, ABI,
};
use enumflags2::{bitflags, BitFlags};
use std::mem::zeroed;

/// Network access right.
///
/// Each variant of `AccessNet` is an [access right](https://www.kernel.org/doc/html/latest/userspace-api/landlock.html#access-rights)
/// for the network.
/// A set of access rights can be created with [`BitFlags<AccessNet>`](BitFlags).
///
/// # Example
///
/// ```
/// use landlock::{ABI, Access, AccessNet, BitFlags, make_bitflags};
///
/// let bind = AccessNet::BindTcp;
///
/// let bind_set: BitFlags<AccessNet> = bind.into();
///
/// let bind_connect = make_bitflags!(AccessNet::{BindTcp | ConnectTcp});
///
/// let net_v4 = AccessNet::from_all(ABI::V4);
///
/// assert_eq!(bind_connect, net_v4);
/// ```
///
/// # Warning
///
/// To avoid unknown restrictions **don't use `BitFlags::<AccessNet>::all()` nor `BitFlags::ALL`**,
/// but use a version you tested and vetted instead,
/// for instance [`AccessNet::from_all(ABI::V4)`](Access::from_all).
/// Direct use of **the [`BitFlags`] API is deprecated**.
/// See [`ABI`] for the rationale and help to test it.
#[bitflags]
#[repr(u64)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AccessNet {
    /// Bind to a TCP port.
    BindTcp = uapi::LANDLOCK_ACCESS_NET_BIND_TCP as u64,
    /// Connect to a TCP port.
    ConnectTcp = uapi::LANDLOCK_ACCESS_NET_CONNECT_TCP as u64,
}

/// # Warning
///
/// If `ABI <= ABI::V3`, `AccessNet::from_all()` returns an empty `BitFlags<AccessNet>`, which
/// makes `Ruleset::handle_access(AccessNet::from_all(ABI::V3))` return an error.
impl Access for AccessNet {
    fn from_all(abi: ABI) -> BitFlags<Self> {
        match abi {
            ABI::Unsupported | ABI::V1 | ABI::V2 | ABI::V3 => BitFlags::EMPTY,
            ABI::V4 | ABI::V5 | ABI::V6 => AccessNet::BindTcp | AccessNet::ConnectTcp,
        }
    }
}

impl HandledAccess for AccessNet {}

impl PrivateHandledAccess for AccessNet {
    fn ruleset_handle_access(
        ruleset: &mut Ruleset,
        access: BitFlags<Self>,
    ) -> Result<(), HandleAccessesError> {
        // We need to record the requested accesses for PrivateRule::check_consistency().
        ruleset.requested_handled_net |= access;
        ruleset.actual_handled_net |= match access
            .try_compat(
                ruleset.compat.abi(),
                ruleset.compat.level,
                &mut ruleset.compat.state,
            )
            .map_err(HandleAccessError::Compat)?
        {
            Some(a) => a,
            None => return Ok(()),
        };
        Ok(())
    }

    fn into_add_rules_error(error: AddRuleError<Self>) -> AddRulesError {
        AddRulesError::Net(error)
    }

    fn into_handle_accesses_error(error: HandleAccessError<Self>) -> HandleAccessesError {
        HandleAccessesError::Net(error)
    }
}

/// Landlock rule for a network port.
///
/// # Example
///
/// ```
/// use landlock::{AccessNet, NetPort};
///
/// fn bind_http() -> NetPort {
///     NetPort::new(80, AccessNet::BindTcp)
/// }
/// ```
#[cfg_attr(test, derive(Debug))]
pub struct NetPort {
    attr: uapi::landlock_net_port_attr,
    // Only 16-bit port make sense for now.
    port: u16,
    allowed_access: BitFlags<AccessNet>,
    compat_level: Option<CompatLevel>,
}

// If we need support for 32 or 64 ports, we'll add a new_32() or a new_64() method returning a
// Result with a potential overflow error.
impl NetPort {
    /// Creates a new TCP port rule.
    ///
    /// As defined by the Linux ABI, `port` with a value of `0` means that TCP bindings will be
    /// allowed for a port range defined by `/proc/sys/net/ipv4/ip_local_port_range`.
    pub fn new<A>(port: u16, access: A) -> Self
    where
        A: Into<BitFlags<AccessNet>>,
    {
        NetPort {
            // Invalid access-rights until as_ptr() is called.
            attr: unsafe { zeroed() },
            port,
            allowed_access: access.into(),
            compat_level: None,
        }
    }
}

impl Rule<AccessNet> for NetPort {}

impl PrivateRule<AccessNet> for NetPort {
    const TYPE_ID: uapi::landlock_rule_type = uapi::landlock_rule_type_LANDLOCK_RULE_NET_PORT;

    fn as_ptr(&mut self) -> *const libc::c_void {
        self.attr.port = self.port as u64;
        self.attr.allowed_access = self.allowed_access.bits();
        &self.attr as *const _ as _
    }

    fn check_consistency(&self, ruleset: &RulesetCreated) -> Result<(), AddRulesError> {
        // Checks that this rule doesn't contain a superset of the access-rights handled by the
        // ruleset.  This check is about requested access-rights but not actual access-rights.
        // Indeed, we want to get a deterministic behavior, i.e. not based on the running kernel
        // (which is handled by Ruleset and RulesetCreated).
        if ruleset.requested_handled_net.contains(self.allowed_access) {
            Ok(())
        } else {
            Err(AddRuleError::UnhandledAccess {
                access: self.allowed_access,
                incompatible: self.allowed_access & !ruleset.requested_handled_net,
            }
            .into())
        }
    }
}

#[test]
fn net_port_check_consistency() {
    use crate::*;

    let bind = AccessNet::BindTcp;
    let bind_connect = bind | AccessNet::ConnectTcp;

    assert!(matches!(
        Ruleset::from(ABI::Unsupported)
            .handle_access(bind)
            .unwrap()
            .create()
            .unwrap()
            .add_rule(NetPort::new(1, bind_connect))
            .unwrap_err(),
        RulesetError::AddRules(AddRulesError::Net(AddRuleError::UnhandledAccess { access, incompatible }))
            if access == bind_connect && incompatible == AccessNet::ConnectTcp
    ));
}

impl TryCompat<AccessNet> for NetPort {
    fn try_compat_children<L>(
        mut self,
        abi: ABI,
        parent_level: L,
        compat_state: &mut CompatState,
    ) -> Result<Option<Self>, CompatError<AccessNet>>
    where
        L: Into<CompatLevel>,
    {
        // Checks with our own compatibility level, if any.
        self.allowed_access = match self.allowed_access.try_compat(
            abi,
            self.tailored_compat_level(parent_level),
            compat_state,
        )? {
            Some(a) => a,
            None => return Ok(None),
        };
        Ok(Some(self))
    }

    fn try_compat_inner(
        &mut self,
        _abi: ABI,
    ) -> Result<CompatResult<AccessNet>, CompatError<AccessNet>> {
        Ok(CompatResult::Full)
    }
}

impl OptionCompatLevelMut for NetPort {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat_level
    }
}

impl OptionCompatLevelMut for &mut NetPort {
    fn as_option_compat_level_mut(&mut self) -> &mut Option<CompatLevel> {
        &mut self.compat_level
    }
}

impl Compatible for NetPort {}

impl Compatible for &mut NetPort {}
