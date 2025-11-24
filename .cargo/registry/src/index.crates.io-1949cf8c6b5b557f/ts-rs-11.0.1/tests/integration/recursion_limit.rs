use std::any::TypeId;

use ts_rs::{TypeVisitor, TS};

#[rustfmt::skip]
#[allow(clippy::all)]
#[derive(Debug, ts_rs::TS)]
#[ts(export, export_to = "very_big_types/")]
pub enum Iso4217CurrencyCode {
    AED, AFN, ALL, AMD, ANG, AOA, ARS, AUD, AWG, AZN, BAM, BBD, BDT, BGN, BHD, BIF, BMD, BND, BOB,
    BRL, BSD, BTN, BWP, BYN, BZD, CAD, CDF, CHF, CLP, CNY, COP, CRC, CUC, CUP, CVE, CZK, DJF, DKK,
    DOP, DZD, EGP, ERN, ETB, EUR, FJD, FKP, GBP, GEL, GGP, GHS, GIP, GMD, GNF, GTQ, GYD, HKD, HNL, 
    HRK, HTG, HUF, IDR, ILS, IMP, INR, IQD, IRR, ISK, JEP, JMD, JOD, JPY, KES, KGS, KHR, KMF, KPW, 
    KRW, KWD, KYD, KZT, LAK, LBP, LKR, LRD, LSL, LYD, MAD, MDL, MGA, MKD, MMK, MNT, MOP, MRU, MUR,
    MVR, MWK, MXN, MYR, MZN, NAD, NGN, NIO, NOK, NPR, NZD, OMR, PAB, PEN, PGK, PHP, PKR, PLN, PYG,
    QAR, RON, RSD, RUB, RWF, SAR, SBD, SCR, SDG, SEK, SGD, SHP, SLL, SOS, SPL, SRD, STN, SVC, SYP,
    SZL, THB, TJS, TMT, TND, TOP, TRY, TTD, TVD, TWD, TZS, UAH, UGX, USD, UYU, UZS, VEF, VND, VUV, 
    WST, XAF, XCD, XDR, XOF, XPF, YER, ZAR, ZMW, ZWD,
}

#[rustfmt::skip]
#[derive(Debug, ts_rs::TS)]
#[ts(export, export_to = "very_big_types/")]
pub enum VeryBigEnum {
    V001(String), V002(String), V003(String), V004(String), V005(String), V006(String), V007(String),
    V008(String), V009(String), V010(String), V011(String), V012(String), V013(String), V014(String),
    V015(String), V016(String), V017(String), V018(String), V019(String), V020(String), V021(String),
    V022(String), V023(String), V024(String), V025(String), V026(String), V027(String), V028(String),
    V029(String), V030(String), V031(String), V032(String), V033(String), V034(String), V035(String),
    V036(String), V037(String), V038(String), V039(String), V040(String), V041(String), V042(String),
    V043(String), V044(String), V045(String), V046(String), V047(String), V048(String), V049(String),
    V050(String), V051(String), V052(String), V053(String), V054(String), V055(String), V056(String),
    V057(String), V058(String), V059(String), V060(String), V061(String), V062(String), V063(String),
    V064(String), V065(String), V066(String), V067(String), V068(String), V069(String), V070(String),
    V071(String), V072(String), V073(String), V074(String), V075(String), V076(String), V077(String),
    V078(String), V079(String), V080(String), V081(String), V082(String), V083(String), V084(String),
    V085(String), V086(String), V087(String), V088(String), V089(String), V090(String), V091(String),
    V092(String), V093(String), V094(String), V095(String), V096(String), V097(String), V098(String),
    V099(String), V100(String), V101(String), V102(String), V103(String), V104(String), V105(String),
    V106(String), V107(String), V108(String), V109(String), V110(String), V111(String), V112(String),
    V113(String), V114(String), V115(String), V116(String), V117(String), V118(String), V119(String),
    V120(String), V121(String), V122(String), V123(String), V124(String), V125(String), V126(String),
    V127(String), V128(String), V129(String), V130(String), V131(String), V132(String), V133(String),
    V134(String), V135(String), V136(String), V137(String), V138(String), V139(String), V140(String),
    V141(String), V142(String), V143(String), V144(String), V145(String), V146(String), V147(String),
    V148(String), V149(String), V150(String), V151(String), V152(String), V153(String), V154(String),
    V155(String), V156(String), V157(String), V158(String), V159(String), V160(String), V161(String),
    V162(String), V163(String), V164(String), V165(String), V166(String), V167(String), V168(String),
    V169(String), V170(String), V171(String), V172(String), V173(String), V174(String), V175(String),
    V176(String), V177(String), V178(String), V179(String), V180(String), V181(String), V182(String),
    V183(String), V184(String), V185(String), V186(String), V187(String), V188(String), V189(String),
    V190(String), V191(String), V192(String), V193(String), V194(String), V195(String), V196(String),
    V197(String), V198(String), V199(String), V200(String), V201(String), V202(String), V203(String),
    V204(String), V205(String), V206(String), V207(String), V208(String), V209(String), V210(String),
    V211(String), V212(String), V213(String), V214(String), V215(String), V216(String), V217(String),
    V218(String), V219(String), V220(String), V221(String), V222(String), V223(String), V224(String),
    V225(String), V226(String), V227(String), V228(String), V229(String), V230(String), V231(String),
    V232(String), V233(String), V234(String), V235(String), V236(String), V237(String), V238(String),
    V239(String), V240(String), V241(String), V242(String), V243(String), V244(String), V245(String),
    V246(String), V247(String), V248(String), V249(String), V250(String), V251(String), V252(String),
    V253(String), V254(String), V255(String), V256(String), 
}

#[test]
fn very_big_enum() {
    struct Visitor(bool);

    impl TypeVisitor for Visitor {
        fn visit<T: TS + 'static + ?Sized>(&mut self) {
            assert!(!self.0, "there must only be one dependency");
            assert_eq!(TypeId::of::<T>(), TypeId::of::<String>());
            self.0 = true;
        }
    }

    let mut visitor = Visitor(false);
    VeryBigEnum::visit_dependencies(&mut visitor);

    assert!(visitor.0, "there must be at least one dependency");
}

macro_rules! generate_types {
    ($a:ident, $b:ident $($t:tt)*) => {
        #[derive(TS)]
        #[ts(export, export_to = "very_big_types/")]
        struct $a($b);
        generate_types!($b $($t)*);
    };
    ($a:ident) => {
        #[derive(TS)]
        #[ts(export, export_to = "very_big_types/")]
        struct $a;
    }
}

// This generates
// `#[derive(TS)] struct T000(T001)`
// `#[derive(TS)] struct T001(T002)`
// ...
// `#[derive(TS)] struct T082(T083)`
// `#[derive(TS)] struct T083;`
generate_types!(
    T000, T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015,
    T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T028, T029, T030, T031,
    T032, T033, T034, T035, T036, T037, T038, T039, T040, T041, T042, T043, T044, T045, T046, T047,
    T048, T049, T050, T051, T052, T053, T054, T055, T056, T057, T058, T059, T060, T061, T062, T063,
    T064, T065, T066, T067, T068, T069, T070, T071, T072, T073, T074, T075, T076, T077, T078, T079,
    T080, T081, T082, T083
);
