/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use crate::eval::runtime::profile::data::ProfileDataImpl;
use crate::eval::ProfileData;
use crate::eval::ProfileMode;

#[derive(Debug, thiserror::Error)]
enum ProfileError {
    #[error("Inconsistent profile type, expected `{0}`, got `{1}`")]
    InconsistentProfileType(ProfileMode, ProfileMode),
}

pub(crate) trait ProfilerType {
    /// Result of profiling.
    type Data;

    const PROFILE_MODE: ProfileMode;

    fn data_from_generic(profile_data: &ProfileDataImpl) -> Option<&Self::Data>;
    fn data_to_generic(data: Self::Data) -> ProfileDataImpl;

    fn merge_profiles_impl(profiles: &[&Self::Data]) -> crate::Result<Self::Data>;

    // Provided methods.

    fn merge_profiles(profiles: &[&ProfileData]) -> crate::Result<ProfileData> {
        let profiles: Vec<&Self::Data> = profiles
            .iter()
            .map(|p| match Self::data_from_generic(&p.profile) {
                None => Err(crate::Error::new_other(
                    ProfileError::InconsistentProfileType(
                        Self::PROFILE_MODE,
                        p.profile.profile_mode(),
                    ),
                )),
                Some(p) => Ok(p),
            })
            .collect::<crate::Result<_>>()?;
        let merged = Self::merge_profiles_impl(&profiles)?;
        Ok(ProfileData {
            profile: Self::data_to_generic(merged),
        })
    }
}
