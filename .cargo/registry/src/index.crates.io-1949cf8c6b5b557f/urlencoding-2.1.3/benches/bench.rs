#![feature(test)]
extern crate test;

use urlencoding::*;
use test::Bencher;

#[bench]
fn bench_enc_nop_short(b: &mut Bencher) {
    b.iter(|| {
        encode("hello")
    })
}
#[bench]
fn bench_enc_nop_long(b: &mut Bencher) {
    b.iter(|| {
        encode("Lorem-ipsum-dolor-sit-amet-consectetur-adipisicing-elit-sed-do-eiusmod-tempor-incididunt-ut-labore-et-dolore-magna-aliqua.Ut-enim-ad-minim-veniam-quis-nostrud\
            -exercitation-ullamco-laboris-nisi-ut-aliquip-ex-ea-commodo-consequat.Duis-aute-irure-dolor-in-reprehenderit-in-voluptate-velit-esse-cillum-dolore-eu-fugiat-nulla\
            -pariatur.Excepteur-sint-occaecat-cupidatat-non-proident-sunt-in-culpa-qui-officia-deserunt-mollit-anim-id-est-laborum.")
    })
}

#[bench]
fn bench_dec_nop_short(b: &mut Bencher) {
    b.iter(|| {
        decode("hello")
    })
}
#[bench]
fn bench_dec_nop_long(b: &mut Bencher) {
    b.iter(|| {
        decode("Lorem-ipsum-dolor-sit-amet-consectetur-adipisicing-elit-sed-do-eiusmod-tempor-incididunt-ut-labore-et-dolore-magna-aliqua.Ut-enim-ad-minim-veniam-quis-nostrud\
            -exercitation-ullamco-laboris-nisi-ut-aliquip-ex-ea-commodo-consequat.Duis-aute-irure-dolor-in-reprehenderit-in-voluptate-velit-esse-cillum-dolore-eu-fugiat-nulla\
            -pariatur.Excepteur-sint-occaecat-cupidatat-non-proident-sunt-in-culpa-qui-officia-deserunt-mollit-anim-id-est-laborum.")
    })
}

#[bench]
fn bench_enc_chg_short(b: &mut Bencher) {
    b.iter(|| {
        encode("he!!o")
    })
}
#[bench]
fn bench_enc_chg_long(b: &mut Bencher) {
    b.iter(|| {
        encode("Lorem ipsum dolor sit amet consectetur adipisicing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.Ut enim ad minim veniam quis nostrud\
             exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla\
             pariatur. Excepteur sint occaecat cupidatat non proident sunt in culpa qui officia deserunt mollit anim id est laborum.")
    })
}

#[bench]
fn bench_dec_chg_short(b: &mut Bencher) {
    b.iter(|| {
        decode("he%26%26o")
    })
}
#[bench]
fn bench_dec_chg_long(b: &mut Bencher) {
    b.iter(|| {
        decode("Lorem%20ipsum%20dolor%20sit%20amet%20consectetur%20adipisicing%20elit%20sed%20do%20eiusmod%20tempor%20incididunt%20ut%20labore%20et%20dolore%20magna%20aliqua.Ut%20enim%20ad%20minim%20veniam%20quis%20nostrud\
            %20exercitation%20ullamco%20laboris%20nisi%20ut%20aliquip%20ex%20ea%20commodo%20consequat.Duis%20aute%20irure%20dolor%20in%20reprehenderit%20in%20voluptate%20velit%20esse%20cillum%20dolore%20eu%20fugiat%20nulla\
            %20pariatur.Excepteur%20sint%20occaecat%20cupidatat%20non%20proident%20sunt%20in%20culpa%20qui%20officia%20deserunt%20mollit%20anim%20id%20est%20laborum.")
    })
}
