#![feature(test)]

extern crate test;

use semver::{Prerelease, Version, VersionReq};
use test::{black_box, Bencher};

#[bench]
fn parse_prerelease(b: &mut Bencher) {
    let text = "x.7.z.92";
    b.iter(|| black_box(text).parse::<Prerelease>().unwrap());
}

#[bench]
fn parse_version(b: &mut Bencher) {
    let text = "1.0.2021-beta+exp.sha.5114f85";
    b.iter(|| black_box(text).parse::<Version>().unwrap());
}

#[bench]
fn parse_version_req(b: &mut Bencher) {
    let text = ">=1.2.3, <2.0.0";
    b.iter(|| black_box(text).parse::<VersionReq>().unwrap());
}
