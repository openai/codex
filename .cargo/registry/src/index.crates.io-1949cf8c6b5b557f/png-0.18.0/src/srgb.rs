use crate::{ScaledFloat, SourceChromaticities};

/// Get the gamma that should be substituted for images conforming to the sRGB color space.
pub fn substitute_gamma() -> ScaledFloat {
    // Value taken from https://www.w3.org/TR/2003/REC-PNG-20031110/#11sRGB
    ScaledFloat::from_scaled(45455)
}

/// Get the chromaticities that should be substituted for images conforming to the sRGB color space.
pub fn substitute_chromaticities() -> SourceChromaticities {
    // Values taken from https://www.w3.org/TR/2003/REC-PNG-20031110/#11sRGB
    SourceChromaticities {
        white: (
            ScaledFloat::from_scaled(31270),
            ScaledFloat::from_scaled(32900),
        ),
        red: (
            ScaledFloat::from_scaled(64000),
            ScaledFloat::from_scaled(33000),
        ),
        green: (
            ScaledFloat::from_scaled(30000),
            ScaledFloat::from_scaled(60000),
        ),
        blue: (
            ScaledFloat::from_scaled(15000),
            ScaledFloat::from_scaled(6000),
        ),
    }
}
