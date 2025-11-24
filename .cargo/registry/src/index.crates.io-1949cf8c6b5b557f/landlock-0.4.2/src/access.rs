// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{
    private, AccessError, AddRuleError, AddRulesError, BitFlags, CompatError, CompatResult,
    HandleAccessError, HandleAccessesError, Ruleset, TailoredCompatLevel, TryCompat, ABI,
};
use enumflags2::BitFlag;

#[cfg(test)]
use crate::{make_bitflags, AccessFs, CompatLevel, CompatState, Compatibility};

pub trait Access: BitFlag + private::Sealed {
    /// Gets the access rights defined by a specific [`ABI`].
    fn from_all(abi: ABI) -> BitFlags<Self>;
}

// This HandledAccess trait is useful to document the API.
pub trait HandledAccess: Access {}

pub trait PrivateHandledAccess: HandledAccess {
    fn ruleset_handle_access(
        ruleset: &mut Ruleset,
        access: BitFlags<Self>,
    ) -> Result<(), HandleAccessesError>
    where
        Self: Access;

    fn into_add_rules_error(error: AddRuleError<Self>) -> AddRulesError
    where
        Self: Access;

    fn into_handle_accesses_error(error: HandleAccessError<Self>) -> HandleAccessesError
    where
        Self: Access;
}

// Creates an illegal/overflowed BitFlags<T> with all its bits toggled, including undefined ones.
fn full_negation<T>(flags: BitFlags<T>) -> BitFlags<T>
where
    T: Access,
{
    unsafe { BitFlags::<T>::from_bits_unchecked(!flags.bits()) }
}

#[test]
fn bit_flags_full_negation() {
    let scoped_negation = !BitFlags::<AccessFs>::all();
    assert_eq!(scoped_negation, BitFlags::<AccessFs>::empty());
    // !BitFlags::<AccessFs>::all() could be equal to full_negation(BitFlags::<AccessFs>::all()))
    // if all the 64-bits would be used, which is not currently the case.
    assert_ne!(scoped_negation, full_negation(BitFlags::<AccessFs>::all()));
}

impl<A> TailoredCompatLevel for BitFlags<A> where A: Access {}

impl<A> TryCompat<A> for BitFlags<A>
where
    A: Access,
{
    fn try_compat_inner(&mut self, abi: ABI) -> Result<CompatResult<A>, CompatError<A>> {
        if self.is_empty() {
            // Empty access-rights would result to a runtime error.
            Err(AccessError::Empty.into())
        } else if !Self::all().contains(*self) {
            // Unknown access-rights (at build time) would result to a runtime error.
            // This can only be reached by using the unsafe BitFlags::from_bits_unchecked().
            Err(AccessError::Unknown {
                access: *self,
                unknown: *self & full_negation(Self::all()),
            }
            .into())
        } else {
            let compat = *self & A::from_all(abi);
            let ret = if compat.is_empty() {
                Ok(CompatResult::No(
                    AccessError::Incompatible { access: *self }.into(),
                ))
            } else if compat != *self {
                let error = AccessError::PartiallyCompatible {
                    access: *self,
                    incompatible: *self & full_negation(compat),
                }
                .into();
                Ok(CompatResult::Partial(error))
            } else {
                Ok(CompatResult::Full)
            };
            *self = compat;
            ret
        }
    }
}

#[test]
fn compat_bit_flags() {
    use crate::ABI;

    let mut compat: Compatibility = ABI::V1.into();
    assert!(compat.state == CompatState::Init);

    let ro_access = make_bitflags!(AccessFs::{Execute | ReadFile | ReadDir});
    assert_eq!(
        ro_access,
        ro_access
            .try_compat(compat.abi(), compat.level, &mut compat.state)
            .unwrap()
            .unwrap()
    );
    assert!(compat.state == CompatState::Full);

    let empty_access = BitFlags::<AccessFs>::empty();
    assert!(matches!(
        empty_access
            .try_compat(compat.abi(), compat.level, &mut compat.state)
            .unwrap_err(),
        CompatError::Access(AccessError::Empty)
    ));

    let all_unknown_access = unsafe { BitFlags::<AccessFs>::from_bits_unchecked(1 << 63) };
    assert!(matches!(
        all_unknown_access.try_compat(compat.abi(), compat.level, &mut compat.state).unwrap_err(),
        CompatError::Access(AccessError::Unknown { access, unknown }) if access == all_unknown_access && unknown == all_unknown_access
    ));
    // An error makes the state final.
    assert!(compat.state == CompatState::Dummy);

    let some_unknown_access = unsafe { BitFlags::<AccessFs>::from_bits_unchecked(1 << 63 | 1) };
    assert!(matches!(
        some_unknown_access.try_compat(compat.abi(), compat.level, &mut compat.state).unwrap_err(),
        CompatError::Access(AccessError::Unknown { access, unknown }) if access == some_unknown_access && unknown == all_unknown_access
    ));
    assert!(compat.state == CompatState::Dummy);

    compat = ABI::Unsupported.into();

    // Tests that the ruleset is marked as unsupported.
    assert!(compat.state == CompatState::Init);

    // Access-rights are valid (but ignored) when they are not required for the current ABI.
    assert_eq!(
        None,
        ro_access
            .try_compat(compat.abi(), compat.level, &mut compat.state)
            .unwrap()
    );

    assert!(compat.state == CompatState::No);

    // Access-rights are not valid when they are required for the current ABI.
    compat.level = Some(CompatLevel::HardRequirement);
    assert!(matches!(
        ro_access.try_compat(compat.abi(), compat.level, &mut compat.state).unwrap_err(),
        CompatError::Access(AccessError::Incompatible { access }) if access == ro_access
    ));

    compat = ABI::V1.into();

    // Tests that the ruleset is marked as the unknown compatibility state.
    assert!(compat.state == CompatState::Init);

    // Access-rights are valid (but ignored) when they are not required for the current ABI.
    assert_eq!(
        ro_access,
        ro_access
            .try_compat(compat.abi(), compat.level, &mut compat.state)
            .unwrap()
            .unwrap()
    );

    // Tests that the ruleset is in an unsupported state, which is important to be able to still
    // enforce no_new_privs.
    assert!(compat.state == CompatState::Full);

    let v2_access = ro_access | AccessFs::Refer;

    // Access-rights are not valid when they are required for the current ABI.
    compat.level = Some(CompatLevel::HardRequirement);
    assert!(matches!(
        v2_access.try_compat(compat.abi(), compat.level, &mut compat.state).unwrap_err(),
        CompatError::Access(AccessError::PartiallyCompatible { access, incompatible })
            if access == v2_access && incompatible == AccessFs::Refer
    ));
}
