use super::SpinnerKind;
use super::SpinnerSet;
use super::SpinnerTheme;

mod animation1;
mod animation2;
pub(crate) mod animation3;
pub(crate) mod animation4;
mod default;

pub(super) fn theme(set: SpinnerSet, kind: SpinnerKind) -> SpinnerTheme {
    match set {
        SpinnerSet::Default => default::theme(kind),
        SpinnerSet::Animation1 => animation1::theme(kind),
        SpinnerSet::Animation2 => animation2::theme(kind),
        SpinnerSet::Animation3 => animation3::theme(kind),
        SpinnerSet::Animation4 => animation4::theme(kind),
    }
}
