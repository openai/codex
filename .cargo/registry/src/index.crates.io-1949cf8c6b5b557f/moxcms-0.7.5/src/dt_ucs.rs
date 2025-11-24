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
use crate::Xyz;
use crate::mlaf::mlaf;
use pxfm::{f_atan2f, f_powf, f_sincosf};

/// Darktable UCS JCH ( Darktable Uniform Color Space )
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub struct DtUchJch {
    pub j: f32,
    pub c: f32,
    pub h: f32,
}

/// Darktable UCS HSB ( Darktable Uniform Color Space )
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub struct DtUchHsb {
    pub h: f32,
    pub s: f32,
    pub b: f32,
}

/// Darktable HCB ( Darktable Uniform Color Space )
#[derive(Copy, Clone, PartialOrd, PartialEq, Debug)]
pub struct DtUchHcb {
    pub h: f32,
    pub c: f32,
    pub b: f32,
}

const DT_UCS_L_STAR_RANGE: f32 = 2.098883786377;

#[inline]
fn y_to_dt_ucs_l_star(y: f32) -> f32 {
    let y_hat = f_powf(y, 0.631651345306265);
    DT_UCS_L_STAR_RANGE * y_hat / (y_hat + 1.12426773749357)
}

#[inline]
fn dt_ucs_l_star_to_y(x: f32) -> f32 {
    f_powf(
        1.12426773749357 * x / (DT_UCS_L_STAR_RANGE - x),
        1.5831518565279648,
    )
}

const L_WHITE: f32 = 0.98805060;

#[inline]
fn dt_ucs_luv_to_ucs_jch(
    l_star: f32,
    l_white: f32,
    u_star_prime: f32,
    v_star_prime: f32,
) -> DtUchJch {
    let m2: f32 = mlaf(u_star_prime * u_star_prime, v_star_prime, v_star_prime); // square of colorfulness M

    // should be JCH[0] = powf(L_star / L_white), cz) but we treat only the case where cz = 1
    let j = l_star / l_white;
    let c =
        15.932993652962535 * f_powf(l_star, 0.6523997524738018) * f_powf(m2, 0.6007557017508491)
            / l_white;
    let h = f_atan2f(v_star_prime, u_star_prime);
    DtUchJch::new(j, c, h)
}

#[inline]
fn dt_ucs_xy_to_uv(x: f32, y: f32) -> (f32, f32) {
    const X_C: [f32; 3] = [-0.783941002840055, 0.745273540913283, 0.318707282433486];
    const Y_C: [f32; 3] = [0.277512987809202, -0.205375866083878, 2.16743692732158];
    const BIAS: [f32; 3] = [0.153836578598858, -0.165478376301988, 0.291320554395942];

    let mut u_c = mlaf(mlaf(BIAS[0], Y_C[0], y), X_C[0], x);
    let mut v_c = mlaf(mlaf(BIAS[1], Y_C[1], y), X_C[1], x);
    let d_c = mlaf(mlaf(BIAS[2], Y_C[2], y), X_C[2], x);

    let div = if d_c >= 0.0 {
        d_c.max(f32::MIN)
    } else {
        d_c.min(-f32::MIN)
    };
    u_c /= div;
    v_c /= div;

    const STAR_C: [f32; 2] = [1.39656225667, 1.4513954287];
    const STAR_HF_C: [f32; 2] = [1.49217352929, 1.52488637914];

    let u_star = STAR_C[0] * u_c / (u_c.abs() + STAR_HF_C[0]);
    let v_star = STAR_C[1] * v_c / (v_c.abs() + STAR_HF_C[1]);

    // The following is equivalent to a 2D matrix product
    let u_star_prime = mlaf(-1.124983854323892 * u_star, -0.980483721769325, v_star);
    let v_star_prime = mlaf(1.86323315098672 * u_star, 1.971853092390862, v_star);
    (u_star_prime, v_star_prime)
}

impl DtUchJch {
    #[inline]
    pub fn new(j: f32, c: f32, h: f32) -> DtUchJch {
        DtUchJch { j, c, h }
    }

    #[inline]
    pub fn from_xyz(xyz: Xyz) -> DtUchJch {
        DtUchJch::from_xyy(xyz.to_xyy())
    }

    #[inline]
    pub fn to_xyz(&self) -> Xyz {
        let xyy = self.to_xyy();
        Xyz::from_xyy(xyy)
    }

    #[inline]
    pub fn from_xyy(xyy: [f32; 3]) -> DtUchJch {
        let l_star = y_to_dt_ucs_l_star(xyy[2]);
        // let l_white = y_to_dt_ucs_l_star(1.);

        let (u_star_prime, v_star_prime) = dt_ucs_xy_to_uv(xyy[0], xyy[1]);
        dt_ucs_luv_to_ucs_jch(l_star, L_WHITE, u_star_prime, v_star_prime)
    }

    #[inline]
    pub fn to_xyy(&self) -> [f32; 3] {
        // let l_white: f32 = y_to_dt_ucs_l_star(1.0);
        let l_star = (self.j * L_WHITE).max(0.0).min(2.09885);
        let m = if l_star != 0. {
            f_powf(
                self.c * L_WHITE / (15.932993652962535 * f_powf(l_star, 0.6523997524738018)),
                0.8322850678616855,
            )
        } else {
            0.
        };

        let sin_cos_h = f_sincosf(self.h);
        let u_star_prime = m * sin_cos_h.1;
        let v_star_prime = m * sin_cos_h.0;

        // The following is equivalent to a 2D matrix product
        let u_star = mlaf(
            -5.037522385190711 * u_star_prime,
            -2.504856328185843,
            v_star_prime,
        );
        let v_star = mlaf(
            4.760029407436461 * u_star_prime,
            2.874012963239247,
            v_star_prime,
        );

        const F: [f32; 2] = [1.39656225667, 1.4513954287];
        const HF: [f32; 2] = [1.49217352929, 1.52488637914];

        let u_c = -HF[0] * u_star / (u_star.abs() - F[0]);
        let v_c = -HF[1] * v_star / (v_star.abs() - F[1]);

        const U_C: [f32; 3] = [0.167171472114775, -0.150959086409163, 0.940254742367256];
        const V_C: [f32; 3] = [0.141299802443708, -0.155185060382272, 1.000000000000000];
        const BIAS: [f32; 3] = [
            -0.00801531300850582,
            -0.00843312433578007,
            -0.0256325967652889,
        ];

        let mut x = mlaf(mlaf(BIAS[0], V_C[0], v_c), U_C[0], u_c);
        let mut y = mlaf(mlaf(BIAS[1], V_C[1], v_c), U_C[1], u_c);
        let d = mlaf(mlaf(BIAS[2], V_C[2], v_c), U_C[2], u_c);

        let div = if d >= 0.0 {
            d.max(f32::MIN)
        } else {
            d.min(-f32::MIN)
        };
        x /= div;
        y /= div;
        let yb = dt_ucs_l_star_to_y(l_star);
        [x, y, yb]
    }
}

impl DtUchHsb {
    #[inline]
    pub fn new(h: f32, s: f32, b: f32) -> DtUchHsb {
        DtUchHsb { h, s, b }
    }

    #[inline]
    pub fn from_jch(jch: DtUchJch) -> DtUchHsb {
        let b = jch.j * (f_powf(jch.c, 1.33654221029386) + 1.);
        let s = if b > 0. { jch.c / b } else { 0. };
        let h = jch.h;
        DtUchHsb::new(h, s, b)
    }

    #[inline]
    pub fn to_jch(&self) -> DtUchJch {
        let h = self.h;
        let c = self.s * self.b;
        let j = self.b / (f_powf(c, 1.33654221029386) + 1.);
        DtUchJch::new(j, c, h)
    }
}

impl DtUchHcb {
    #[inline]
    pub fn new(h: f32, c: f32, b: f32) -> DtUchHcb {
        DtUchHcb { h, c, b }
    }

    #[inline]
    pub fn from_jch(jch: DtUchJch) -> DtUchHcb {
        let b = jch.j * (f_powf(jch.c, 1.33654221029386) + 1.);
        let c = jch.c;
        let h = jch.h;
        DtUchHcb::new(h, c, b)
    }

    #[inline]
    pub fn to_jch(&self) -> DtUchJch {
        let h = self.h;
        let c = self.c;
        let j = self.b / (f_powf(self.c, 1.33654221029386) + 1.);
        DtUchJch::new(j, c, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_darktable_ucs_jch() {
        let xyy = [0.4, 0.2, 0.5];
        let ucs = DtUchJch::from_xyy(xyy);
        let xyy_rev = ucs.to_xyy();
        assert!(
            (xyy[0] - xyy_rev[0]).abs() < 1e-5,
            "Expected {}, got {}",
            xyy[0],
            xyy_rev[0]
        );
        assert!(
            (xyy[1] - xyy_rev[1]).abs() < 1e-5,
            "Expected {}, got {}",
            xyy[1],
            xyy_rev[1]
        );
        assert!(
            (xyy[2] - xyy_rev[2]).abs() < 1e-5,
            "Expected {}, got {}",
            xyy[2],
            xyy_rev[2]
        );
    }

    #[test]
    fn test_darktable_hsb() {
        let jch = DtUchJch::new(0.3, 0.6, 0.4);
        let hsb = DtUchHsb::from_jch(jch);
        let r_jch = hsb.to_jch();

        assert!(
            (r_jch.j - jch.j).abs() < 1e-5,
            "Expected {}, got {}",
            jch.j,
            r_jch.j
        );
        assert!(
            (r_jch.c - jch.c).abs() < 1e-5,
            "Expected {}, got {}",
            jch.c,
            r_jch.c
        );
        assert!(
            (r_jch.h - jch.h).abs() < 1e-5,
            "Expected {}, got {}",
            jch.h,
            r_jch.h
        );
    }

    #[test]
    fn test_darktable_hcb() {
        let jch = DtUchJch::new(0.3, 0.6, 0.4);
        let hcb = DtUchHcb::from_jch(jch);
        let r_jch = hcb.to_jch();

        assert!(
            (r_jch.j - jch.j).abs() < 1e-5,
            "Expected {}, got {}",
            jch.j,
            r_jch.j
        );
        assert!(
            (r_jch.c - jch.c).abs() < 1e-5,
            "Expected {}, got {}",
            jch.c,
            r_jch.c
        );
        assert!(
            (r_jch.h - jch.h).abs() < 1e-5,
            "Expected {}, got {}",
            jch.h,
            r_jch.h
        );
    }

    #[test]
    fn test_darktable_ucs_jch_from_xyz() {
        let xyz = Xyz::new(0.4, 0.2, 0.5);
        let ucs = DtUchJch::from_xyz(xyz);
        let xyy_rev = ucs.to_xyz();
        assert!(
            (xyz.x - xyz.x).abs() < 1e-5,
            "Expected {}, got {}",
            xyz.x,
            xyy_rev.x
        );
        assert!(
            (xyz.y - xyz.y).abs() < 1e-5,
            "Expected {}, got {}",
            xyz.y,
            xyy_rev.y
        );
        assert!(
            (xyz.z - xyz.z).abs() < 1e-5,
            "Expected {}, got {}",
            xyz.z,
            xyy_rev.z
        );
    }
}
