/*
 * // Copyright (c) Radzivon Bartoshyk 3/2025. All rights reserved.
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
use crate::gamma::{pq_from_linearf, pq_to_linearf};
use crate::{Matrix3f, Rgb, Vector3f, Xyz};

const CROSSTALK: Matrix3f = Matrix3f {
    v: [[0.92, 0.04, 0.04], [0.04, 0.92, 0.04], [0.04, 0.04, 0.92]],
};

const HPE_LMS: Matrix3f = Matrix3f {
    v: [
        [0.4002, 0.7076, -0.0808],
        [-0.2263, 1.1653, 0.0457],
        [0f32, 0f32, 0.9182],
    ],
};

const XYZ_TO_LMS: Matrix3f = CROSSTALK.mat_mul_const(HPE_LMS);

const LMS_TO_XYZ: Matrix3f = XYZ_TO_LMS.inverse();

const L_LMS_TO_ICTCP: Matrix3f = Matrix3f {
    v: [
        [2048. / 4096., 2048. / 4096., 0.],
        [6610. / 4096., -13613. / 4096., 7003. / 4096.],
        [17933. / 4096., -17390. / 4096., -543. / 4096.],
    ],
};

const ICTCP_TO_L_LMS: Matrix3f = L_LMS_TO_ICTCP.inverse();

#[derive(Copy, Clone, Default, PartialOrd, PartialEq)]
pub struct ICtCp {
    /// Lightness
    pub i: f32,
    /// Tritan
    pub ct: f32,
    /// Protan
    pub cp: f32,
}

impl ICtCp {
    #[inline]
    pub const fn new(i: f32, ct: f32, cp: f32) -> ICtCp {
        ICtCp { i, ct, cp }
    }

    /// Converts XYZ D65 to ICtCp
    #[inline]
    pub fn from_xyz(xyz: Xyz) -> ICtCp {
        let lms = XYZ_TO_LMS.mul_vector(xyz.to_vector());
        let lin_l = pq_from_linearf(lms.v[0]);
        let lin_m = pq_from_linearf(lms.v[1]);
        let lin_s = pq_from_linearf(lms.v[2]);
        let ictcp = L_LMS_TO_ICTCP.mul_vector(Vector3f {
            v: [lin_l, lin_m, lin_s],
        });
        ICtCp {
            i: ictcp.v[0],
            ct: ictcp.v[1],
            cp: ictcp.v[2],
        }
    }

    /// Converts to [ICtCp] from linear light [Rgb]
    ///
    /// Precompute forward matrix by [ICtCp::prepare_to_lms].
    /// D65 white point is assumed.
    #[inline]
    pub fn from_linear_rgb(rgb: Rgb<f32>, matrix: Matrix3f) -> ICtCp {
        let lms = matrix.mul_vector(rgb.to_vector());
        let lin_l = pq_from_linearf(lms.v[0]);
        let lin_m = pq_from_linearf(lms.v[1]);
        let lin_s = pq_from_linearf(lms.v[2]);
        let ictcp = L_LMS_TO_ICTCP.mul_vector(Vector3f {
            v: [lin_l, lin_m, lin_s],
        });
        ICtCp {
            i: ictcp.v[0],
            ct: ictcp.v[1],
            cp: ictcp.v[2],
        }
    }

    /// Converts [ICtCp] to [Rgb]
    ///
    /// Precompute forward matrix by [ICtCp::prepare_to_lms] and then inverse it
    #[inline]
    pub fn to_linear_rgb(&self, matrix: Matrix3f) -> Rgb<f32> {
        let l_lms = ICTCP_TO_L_LMS.mul_vector(Vector3f {
            v: [self.i, self.ct, self.cp],
        });
        let gamma_l = pq_to_linearf(l_lms.v[0]);
        let gamma_m = pq_to_linearf(l_lms.v[1]);
        let gamma_s = pq_to_linearf(l_lms.v[2]);

        let lms = matrix.mul_vector(Vector3f {
            v: [gamma_l, gamma_m, gamma_s],
        });
        Rgb {
            r: lms.v[0],
            g: lms.v[1],
            b: lms.v[2],
        }
    }

    /// Converts ICtCp to XYZ D65
    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        let l_lms = ICTCP_TO_L_LMS.mul_vector(Vector3f {
            v: [self.i, self.ct, self.cp],
        });
        let gamma_l = pq_to_linearf(l_lms.v[0]);
        let gamma_m = pq_to_linearf(l_lms.v[1]);
        let gamma_s = pq_to_linearf(l_lms.v[2]);

        let lms = LMS_TO_XYZ.mul_vector(Vector3f {
            v: [gamma_l, gamma_m, gamma_s],
        });
        Xyz {
            x: lms.v[0],
            y: lms.v[1],
            z: lms.v[2],
        }
    }

    /// Prepares RGB->LMS matrix
    #[inline]
    pub const fn prepare_to_lms(rgb_to_xyz: Matrix3f) -> Matrix3f {
        XYZ_TO_LMS.mat_mul_const(rgb_to_xyz)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_roundtrip() {
        let xyz = Xyz::new(0.5, 0.4, 0.3);
        let ictcp = ICtCp::from_xyz(xyz);
        let r_xyz = ictcp.to_xyz();
        assert!((r_xyz.x - xyz.x).abs() < 1e-4);
        assert!((r_xyz.y - xyz.y).abs() < 1e-4);
        assert!((r_xyz.z - xyz.z).abs() < 1e-4);
    }

    #[test]
    fn check_roundtrip_rgb() {
        let rgb_to_xyz = Matrix3f {
            v: [
                [0.67345345, 0.165661961, 0.125096574],
                [0.27903071, 0.675341845, 0.045627553],
                [-0.00193137419, 0.0299795717, 0.797140181],
            ],
        };
        let prepared_matrix = ICtCp::prepare_to_lms(rgb_to_xyz);
        let inversed_matrix = prepared_matrix.inverse();
        let rgb = Rgb::new(0.5, 0.4, 0.3);
        let ictcp = ICtCp::from_linear_rgb(rgb, prepared_matrix);
        let r_xyz = ictcp.to_linear_rgb(inversed_matrix);
        assert!((r_xyz.r - rgb.r).abs() < 1e-4);
        assert!((r_xyz.g - rgb.g).abs() < 1e-4);
        assert!((r_xyz.b - rgb.b).abs() < 1e-4);
    }
}
