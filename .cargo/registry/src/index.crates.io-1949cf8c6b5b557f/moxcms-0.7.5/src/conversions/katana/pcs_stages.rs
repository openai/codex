/*
 * // Copyright (c) Radzivon Bartoshyk 6/2025. All rights reserved.
 * //
 * // Redistribution and use in source and binary forms, with or without modification,
 * // are permitted provided that the following conditions are met:
 * //
 * // 1.  Redistributions of source code must retain the above copyright notice, this
 * // list of conditions and the following disclaimer.
 * //
 * // 2.  Redistributions in binary form must reproduce the above copyright notice,
 * // this list of conditions and the following disclaimer in the documentation
 * // and/or other materials provided with the distribution.
 * //
 * // 3.  Neither the name of the copyright holder nor the names of its
 * // contributors may be used to endorse or promote products derived from
 * // this software without specific prior written permission.
 * //
 * // THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * // AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * // IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * // DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
 * // FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * // DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * // SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
 * // CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
 * // OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
 * // OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
 */
use crate::conversions::katana::KatanaIntermediateStage;
use crate::conversions::katana::stages::BlackholeIntermediateStage;
use crate::mlaf::mlaf;
use crate::{CmsError, ColorProfile, DataColorSpace, Matrix3f, ProfileVersion};
use std::marker::PhantomData;

pub(crate) struct KatanaMatrixStage {
    pub(crate) matrices: Vec<Matrix3f>,
}

impl KatanaMatrixStage {
    pub(crate) fn new(matrix: Matrix3f) -> Self {
        Self {
            matrices: vec![matrix],
        }
    }
}

pub(crate) type KatanaDefaultIntermediate = dyn KatanaIntermediateStage<f32> + Send + Sync;

impl KatanaIntermediateStage<f32> for KatanaMatrixStage {
    fn stage(&self, input: &mut Vec<f32>) -> Result<Vec<f32>, CmsError> {
        if input.len() % 3 != 0 {
            return Err(CmsError::LaneMultipleOfChannels);
        }

        for m in self.matrices.iter() {
            for dst in input.chunks_exact_mut(3) {
                let x = dst[0];
                let y = dst[1];
                let z = dst[2];
                dst[0] = mlaf(mlaf(x * m.v[0][0], y, m.v[0][1]), z, m.v[0][2]);
                dst[1] = mlaf(mlaf(x * m.v[1][0], y, m.v[1][1]), z, m.v[1][2]);
                dst[2] = mlaf(mlaf(x * m.v[2][0], y, m.v[2][1]), z, m.v[2][2]);
            }
        }

        Ok(std::mem::take(input))
    }
}

pub(crate) fn katana_pcs_lab_v4_to_v2(profile: &ColorProfile) -> Box<KatanaDefaultIntermediate> {
    if profile.pcs == DataColorSpace::Lab && profile.version_internal <= ProfileVersion::V4_0 {
        let v_mat = vec![Matrix3f {
            v: [
                [65280.0 / 65535.0, 0., 0.],
                [0., 65280.0 / 65535.0, 0.],
                [0., 0., 65280.0 / 65535.0],
            ],
        }];
        return Box::new(KatanaMatrixStage { matrices: v_mat });
    }
    Box::new(BlackholeIntermediateStage {
        _phantom: PhantomData,
    })
}

pub(crate) fn katana_pcs_lab_v2_to_v4(profile: &ColorProfile) -> Box<KatanaDefaultIntermediate> {
    if profile.pcs == DataColorSpace::Lab && profile.version_internal <= ProfileVersion::V4_0 {
        let v_mat = vec![Matrix3f {
            v: [
                [65535.0 / 65280.0, 0., 0.],
                [0., 65535.0 / 65280.0, 0.],
                [0., 0., 65535.0 / 65280.0],
            ],
        }];
        return Box::new(KatanaMatrixStage { matrices: v_mat });
    }
    Box::new(BlackholeIntermediateStage {
        _phantom: PhantomData,
    })
}
