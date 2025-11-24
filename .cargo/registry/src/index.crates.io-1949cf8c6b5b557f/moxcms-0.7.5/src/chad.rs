/*
 * // Copyright (c) Radzivon Bartoshyk 2/2025. All rights reserved.
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
use crate::matrix::{Matrix3f, Vector3f, Xyz};
use crate::{Chromaticity, Matrix3d, Vector3d, XyY};

pub(crate) const BRADFORD_D: Matrix3d = Matrix3d {
    v: [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ],
};

pub(crate) const BRADFORD_F: Matrix3f = BRADFORD_D.to_f32();

#[inline]
pub(crate) const fn compute_chromatic_adaption(
    source_white_point: Xyz,
    dest_white_point: Xyz,
    chad: Matrix3f,
) -> Matrix3f {
    let cone_source_xyz = Vector3f {
        v: [
            source_white_point.x,
            source_white_point.y,
            source_white_point.z,
        ],
    };
    let cone_source_rgb = chad.mul_vector(cone_source_xyz);

    let cone_dest_xyz = Vector3f {
        v: [dest_white_point.x, dest_white_point.y, dest_white_point.z],
    };
    let cone_dest_rgb = chad.mul_vector(cone_dest_xyz);

    let cone = Matrix3f {
        v: [
            [cone_dest_rgb.v[0] / cone_source_rgb.v[0], 0., 0.],
            [0., cone_dest_rgb.v[1] / cone_source_rgb.v[1], 0.],
            [0., 0., cone_dest_rgb.v[2] / cone_source_rgb.v[2]],
        ],
    };

    let chad_inv = chad.inverse();

    let p0 = cone.mat_mul_const(chad);
    chad_inv.mat_mul_const(p0)
}

#[inline]
pub(crate) const fn compute_chromatic_adaption_d(
    source_white_point: Xyz,
    dest_white_point: Xyz,
    chad: Matrix3d,
) -> Matrix3d {
    let cone_source_xyz = Vector3d {
        v: [
            source_white_point.x as f64,
            source_white_point.y as f64,
            source_white_point.z as f64,
        ],
    };
    let cone_source_rgb = chad.mul_vector(cone_source_xyz);

    let cone_dest_xyz = Vector3d {
        v: [
            dest_white_point.x as f64,
            dest_white_point.y as f64,
            dest_white_point.z as f64,
        ],
    };
    let cone_dest_rgb = chad.mul_vector(cone_dest_xyz);

    let cone = Matrix3d {
        v: [
            [cone_dest_rgb.v[0] / cone_source_rgb.v[0], 0., 0.],
            [0., cone_dest_rgb.v[1] / cone_source_rgb.v[1], 0.],
            [0., 0., cone_dest_rgb.v[2] / cone_source_rgb.v[2]],
        ],
    };

    let chad_inv = chad.inverse();

    let p0 = cone.mat_mul_const(chad);
    chad_inv.mat_mul_const(p0)
}

pub const fn adaption_matrix(source_illumination: Xyz, target_illumination: Xyz) -> Matrix3f {
    compute_chromatic_adaption(source_illumination, target_illumination, BRADFORD_F)
}

pub const fn adaption_matrix_d(source_illumination: Xyz, target_illumination: Xyz) -> Matrix3d {
    compute_chromatic_adaption_d(source_illumination, target_illumination, BRADFORD_D)
}

pub const fn adapt_to_d50(r: Matrix3f, source_white_pt: XyY) -> Matrix3f {
    adapt_to_illuminant(r, source_white_pt, Chromaticity::D50.to_xyz())
}

pub const fn adapt_to_d50_d(r: Matrix3d, source_white_pt: XyY) -> Matrix3d {
    adapt_to_illuminant_d(r, source_white_pt, Chromaticity::D50.to_xyz())
}

pub const fn adapt_to_illuminant(
    r: Matrix3f,
    source_white_pt: XyY,
    illuminant_xyz: Xyz,
) -> Matrix3f {
    let bradford = adaption_matrix(source_white_pt.to_xyz(), illuminant_xyz);
    bradford.mat_mul_const(r)
}

pub const fn adapt_to_illuminant_d(
    r: Matrix3d,
    source_white_pt: XyY,
    illuminant_xyz: Xyz,
) -> Matrix3d {
    let bradford = adaption_matrix_d(source_white_pt.to_xyz(), illuminant_xyz);
    bradford.mat_mul_const(r)
}

pub const fn adapt_to_illuminant_xyz(
    r: Matrix3f,
    source_white_pt: Xyz,
    illuminant_xyz: Xyz,
) -> Matrix3f {
    if source_white_pt.y == 0.0 {
        return r;
    }

    let bradford = adaption_matrix(source_white_pt, illuminant_xyz);
    bradford.mat_mul_const(r)
}

pub const fn adapt_to_illuminant_xyz_d(
    r: Matrix3d,
    source_white_pt: Xyz,
    illuminant_xyz: Xyz,
) -> Matrix3d {
    if source_white_pt.y == 0.0 {
        return r;
    }

    let bradford = adaption_matrix_d(source_white_pt, illuminant_xyz);
    bradford.mat_mul_const(r)
}
