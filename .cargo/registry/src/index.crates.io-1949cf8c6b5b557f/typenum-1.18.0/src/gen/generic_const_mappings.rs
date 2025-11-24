// THIS IS GENERATED CODE
//! Module with some `const`-generics-friendly definitions, to help bridge the gap
//! between those and `typenum` types.
//!
//!   - It requires the `const-generics` crate feature to be enabled.
//!
//! The main type to use here is [`U`], although [`Const`] and [`ToUInt`] may be needed
//! in a generic context.

use crate::*;

/// The main mapping from a generic `const: usize` to a [`UInt`]: [`U<N>`] is expected to work like [`UN`].
///
///   - It requires the `const-generics` crate feature to be enabled.
///
/// [`U<N>`]: `U`
/// [`UN`]: `U42`
///
/// # Example
///
/// ```rust
/// use typenum::*;
///
/// assert_type_eq!(U<42>, U42);
/// ```
///
/// This can even be used in a generic `const N: usize` context, provided the
/// genericity is guarded by a `where` clause:
///
/// ```rust
/// use typenum::*;
///
/// struct MyStruct<const N: usize>;
///
/// trait MyTrait { type AssocType; }
///
/// impl<const N: usize> MyTrait
///     for MyStruct<N>
/// where
///     Const<N> : ToUInt,
/// {
///     type AssocType = U<N>;
/// }
///
/// assert_type_eq!(<MyStruct<42> as MyTrait>::AssocType, U42);
/// ```
pub type U<const N: usize> = <Const<N> as ToUInt>::Output;

/// Used to allow the usage of [`U`] in a generic context.
pub struct Const<const N: usize>;

/// Used to allow the usage of [`U`] in a generic context.
pub trait ToUInt {
    /// The [`UN`][`crate::U42`] type corresponding to `Self = Const<N>`.
    type Output;
}

impl ToUInt for Const<0> {
    type Output = U0;
}

impl ToUInt for Const<1> {
    type Output = U1;
}

impl ToUInt for Const<2> {
    type Output = U2;
}

impl ToUInt for Const<3> {
    type Output = U3;
}

impl ToUInt for Const<4> {
    type Output = U4;
}

impl ToUInt for Const<5> {
    type Output = U5;
}

impl ToUInt for Const<6> {
    type Output = U6;
}

impl ToUInt for Const<7> {
    type Output = U7;
}

impl ToUInt for Const<8> {
    type Output = U8;
}

impl ToUInt for Const<9> {
    type Output = U9;
}

impl ToUInt for Const<10> {
    type Output = U10;
}

impl ToUInt for Const<11> {
    type Output = U11;
}

impl ToUInt for Const<12> {
    type Output = U12;
}

impl ToUInt for Const<13> {
    type Output = U13;
}

impl ToUInt for Const<14> {
    type Output = U14;
}

impl ToUInt for Const<15> {
    type Output = U15;
}

impl ToUInt for Const<16> {
    type Output = U16;
}

impl ToUInt for Const<17> {
    type Output = U17;
}

impl ToUInt for Const<18> {
    type Output = U18;
}

impl ToUInt for Const<19> {
    type Output = U19;
}

impl ToUInt for Const<20> {
    type Output = U20;
}

impl ToUInt for Const<21> {
    type Output = U21;
}

impl ToUInt for Const<22> {
    type Output = U22;
}

impl ToUInt for Const<23> {
    type Output = U23;
}

impl ToUInt for Const<24> {
    type Output = U24;
}

impl ToUInt for Const<25> {
    type Output = U25;
}

impl ToUInt for Const<26> {
    type Output = U26;
}

impl ToUInt for Const<27> {
    type Output = U27;
}

impl ToUInt for Const<28> {
    type Output = U28;
}

impl ToUInt for Const<29> {
    type Output = U29;
}

impl ToUInt for Const<30> {
    type Output = U30;
}

impl ToUInt for Const<31> {
    type Output = U31;
}

impl ToUInt for Const<32> {
    type Output = U32;
}

impl ToUInt for Const<33> {
    type Output = U33;
}

impl ToUInt for Const<34> {
    type Output = U34;
}

impl ToUInt for Const<35> {
    type Output = U35;
}

impl ToUInt for Const<36> {
    type Output = U36;
}

impl ToUInt for Const<37> {
    type Output = U37;
}

impl ToUInt for Const<38> {
    type Output = U38;
}

impl ToUInt for Const<39> {
    type Output = U39;
}

impl ToUInt for Const<40> {
    type Output = U40;
}

impl ToUInt for Const<41> {
    type Output = U41;
}

impl ToUInt for Const<42> {
    type Output = U42;
}

impl ToUInt for Const<43> {
    type Output = U43;
}

impl ToUInt for Const<44> {
    type Output = U44;
}

impl ToUInt for Const<45> {
    type Output = U45;
}

impl ToUInt for Const<46> {
    type Output = U46;
}

impl ToUInt for Const<47> {
    type Output = U47;
}

impl ToUInt for Const<48> {
    type Output = U48;
}

impl ToUInt for Const<49> {
    type Output = U49;
}

impl ToUInt for Const<50> {
    type Output = U50;
}

impl ToUInt for Const<51> {
    type Output = U51;
}

impl ToUInt for Const<52> {
    type Output = U52;
}

impl ToUInt for Const<53> {
    type Output = U53;
}

impl ToUInt for Const<54> {
    type Output = U54;
}

impl ToUInt for Const<55> {
    type Output = U55;
}

impl ToUInt for Const<56> {
    type Output = U56;
}

impl ToUInt for Const<57> {
    type Output = U57;
}

impl ToUInt for Const<58> {
    type Output = U58;
}

impl ToUInt for Const<59> {
    type Output = U59;
}

impl ToUInt for Const<60> {
    type Output = U60;
}

impl ToUInt for Const<61> {
    type Output = U61;
}

impl ToUInt for Const<62> {
    type Output = U62;
}

impl ToUInt for Const<63> {
    type Output = U63;
}

impl ToUInt for Const<64> {
    type Output = U64;
}

impl ToUInt for Const<65> {
    type Output = U65;
}

impl ToUInt for Const<66> {
    type Output = U66;
}

impl ToUInt for Const<67> {
    type Output = U67;
}

impl ToUInt for Const<68> {
    type Output = U68;
}

impl ToUInt for Const<69> {
    type Output = U69;
}

impl ToUInt for Const<70> {
    type Output = U70;
}

impl ToUInt for Const<71> {
    type Output = U71;
}

impl ToUInt for Const<72> {
    type Output = U72;
}

impl ToUInt for Const<73> {
    type Output = U73;
}

impl ToUInt for Const<74> {
    type Output = U74;
}

impl ToUInt for Const<75> {
    type Output = U75;
}

impl ToUInt for Const<76> {
    type Output = U76;
}

impl ToUInt for Const<77> {
    type Output = U77;
}

impl ToUInt for Const<78> {
    type Output = U78;
}

impl ToUInt for Const<79> {
    type Output = U79;
}

impl ToUInt for Const<80> {
    type Output = U80;
}

impl ToUInt for Const<81> {
    type Output = U81;
}

impl ToUInt for Const<82> {
    type Output = U82;
}

impl ToUInt for Const<83> {
    type Output = U83;
}

impl ToUInt for Const<84> {
    type Output = U84;
}

impl ToUInt for Const<85> {
    type Output = U85;
}

impl ToUInt for Const<86> {
    type Output = U86;
}

impl ToUInt for Const<87> {
    type Output = U87;
}

impl ToUInt for Const<88> {
    type Output = U88;
}

impl ToUInt for Const<89> {
    type Output = U89;
}

impl ToUInt for Const<90> {
    type Output = U90;
}

impl ToUInt for Const<91> {
    type Output = U91;
}

impl ToUInt for Const<92> {
    type Output = U92;
}

impl ToUInt for Const<93> {
    type Output = U93;
}

impl ToUInt for Const<94> {
    type Output = U94;
}

impl ToUInt for Const<95> {
    type Output = U95;
}

impl ToUInt for Const<96> {
    type Output = U96;
}

impl ToUInt for Const<97> {
    type Output = U97;
}

impl ToUInt for Const<98> {
    type Output = U98;
}

impl ToUInt for Const<99> {
    type Output = U99;
}

impl ToUInt for Const<100> {
    type Output = U100;
}

impl ToUInt for Const<101> {
    type Output = U101;
}

impl ToUInt for Const<102> {
    type Output = U102;
}

impl ToUInt for Const<103> {
    type Output = U103;
}

impl ToUInt for Const<104> {
    type Output = U104;
}

impl ToUInt for Const<105> {
    type Output = U105;
}

impl ToUInt for Const<106> {
    type Output = U106;
}

impl ToUInt for Const<107> {
    type Output = U107;
}

impl ToUInt for Const<108> {
    type Output = U108;
}

impl ToUInt for Const<109> {
    type Output = U109;
}

impl ToUInt for Const<110> {
    type Output = U110;
}

impl ToUInt for Const<111> {
    type Output = U111;
}

impl ToUInt for Const<112> {
    type Output = U112;
}

impl ToUInt for Const<113> {
    type Output = U113;
}

impl ToUInt for Const<114> {
    type Output = U114;
}

impl ToUInt for Const<115> {
    type Output = U115;
}

impl ToUInt for Const<116> {
    type Output = U116;
}

impl ToUInt for Const<117> {
    type Output = U117;
}

impl ToUInt for Const<118> {
    type Output = U118;
}

impl ToUInt for Const<119> {
    type Output = U119;
}

impl ToUInt for Const<120> {
    type Output = U120;
}

impl ToUInt for Const<121> {
    type Output = U121;
}

impl ToUInt for Const<122> {
    type Output = U122;
}

impl ToUInt for Const<123> {
    type Output = U123;
}

impl ToUInt for Const<124> {
    type Output = U124;
}

impl ToUInt for Const<125> {
    type Output = U125;
}

impl ToUInt for Const<126> {
    type Output = U126;
}

impl ToUInt for Const<127> {
    type Output = U127;
}

impl ToUInt for Const<128> {
    type Output = U128;
}

impl ToUInt for Const<129> {
    type Output = U129;
}

impl ToUInt for Const<130> {
    type Output = U130;
}

impl ToUInt for Const<131> {
    type Output = U131;
}

impl ToUInt for Const<132> {
    type Output = U132;
}

impl ToUInt for Const<133> {
    type Output = U133;
}

impl ToUInt for Const<134> {
    type Output = U134;
}

impl ToUInt for Const<135> {
    type Output = U135;
}

impl ToUInt for Const<136> {
    type Output = U136;
}

impl ToUInt for Const<137> {
    type Output = U137;
}

impl ToUInt for Const<138> {
    type Output = U138;
}

impl ToUInt for Const<139> {
    type Output = U139;
}

impl ToUInt for Const<140> {
    type Output = U140;
}

impl ToUInt for Const<141> {
    type Output = U141;
}

impl ToUInt for Const<142> {
    type Output = U142;
}

impl ToUInt for Const<143> {
    type Output = U143;
}

impl ToUInt for Const<144> {
    type Output = U144;
}

impl ToUInt for Const<145> {
    type Output = U145;
}

impl ToUInt for Const<146> {
    type Output = U146;
}

impl ToUInt for Const<147> {
    type Output = U147;
}

impl ToUInt for Const<148> {
    type Output = U148;
}

impl ToUInt for Const<149> {
    type Output = U149;
}

impl ToUInt for Const<150> {
    type Output = U150;
}

impl ToUInt for Const<151> {
    type Output = U151;
}

impl ToUInt for Const<152> {
    type Output = U152;
}

impl ToUInt for Const<153> {
    type Output = U153;
}

impl ToUInt for Const<154> {
    type Output = U154;
}

impl ToUInt for Const<155> {
    type Output = U155;
}

impl ToUInt for Const<156> {
    type Output = U156;
}

impl ToUInt for Const<157> {
    type Output = U157;
}

impl ToUInt for Const<158> {
    type Output = U158;
}

impl ToUInt for Const<159> {
    type Output = U159;
}

impl ToUInt for Const<160> {
    type Output = U160;
}

impl ToUInt for Const<161> {
    type Output = U161;
}

impl ToUInt for Const<162> {
    type Output = U162;
}

impl ToUInt for Const<163> {
    type Output = U163;
}

impl ToUInt for Const<164> {
    type Output = U164;
}

impl ToUInt for Const<165> {
    type Output = U165;
}

impl ToUInt for Const<166> {
    type Output = U166;
}

impl ToUInt for Const<167> {
    type Output = U167;
}

impl ToUInt for Const<168> {
    type Output = U168;
}

impl ToUInt for Const<169> {
    type Output = U169;
}

impl ToUInt for Const<170> {
    type Output = U170;
}

impl ToUInt for Const<171> {
    type Output = U171;
}

impl ToUInt for Const<172> {
    type Output = U172;
}

impl ToUInt for Const<173> {
    type Output = U173;
}

impl ToUInt for Const<174> {
    type Output = U174;
}

impl ToUInt for Const<175> {
    type Output = U175;
}

impl ToUInt for Const<176> {
    type Output = U176;
}

impl ToUInt for Const<177> {
    type Output = U177;
}

impl ToUInt for Const<178> {
    type Output = U178;
}

impl ToUInt for Const<179> {
    type Output = U179;
}

impl ToUInt for Const<180> {
    type Output = U180;
}

impl ToUInt for Const<181> {
    type Output = U181;
}

impl ToUInt for Const<182> {
    type Output = U182;
}

impl ToUInt for Const<183> {
    type Output = U183;
}

impl ToUInt for Const<184> {
    type Output = U184;
}

impl ToUInt for Const<185> {
    type Output = U185;
}

impl ToUInt for Const<186> {
    type Output = U186;
}

impl ToUInt for Const<187> {
    type Output = U187;
}

impl ToUInt for Const<188> {
    type Output = U188;
}

impl ToUInt for Const<189> {
    type Output = U189;
}

impl ToUInt for Const<190> {
    type Output = U190;
}

impl ToUInt for Const<191> {
    type Output = U191;
}

impl ToUInt for Const<192> {
    type Output = U192;
}

impl ToUInt for Const<193> {
    type Output = U193;
}

impl ToUInt for Const<194> {
    type Output = U194;
}

impl ToUInt for Const<195> {
    type Output = U195;
}

impl ToUInt for Const<196> {
    type Output = U196;
}

impl ToUInt for Const<197> {
    type Output = U197;
}

impl ToUInt for Const<198> {
    type Output = U198;
}

impl ToUInt for Const<199> {
    type Output = U199;
}

impl ToUInt for Const<200> {
    type Output = U200;
}

impl ToUInt for Const<201> {
    type Output = U201;
}

impl ToUInt for Const<202> {
    type Output = U202;
}

impl ToUInt for Const<203> {
    type Output = U203;
}

impl ToUInt for Const<204> {
    type Output = U204;
}

impl ToUInt for Const<205> {
    type Output = U205;
}

impl ToUInt for Const<206> {
    type Output = U206;
}

impl ToUInt for Const<207> {
    type Output = U207;
}

impl ToUInt for Const<208> {
    type Output = U208;
}

impl ToUInt for Const<209> {
    type Output = U209;
}

impl ToUInt for Const<210> {
    type Output = U210;
}

impl ToUInt for Const<211> {
    type Output = U211;
}

impl ToUInt for Const<212> {
    type Output = U212;
}

impl ToUInt for Const<213> {
    type Output = U213;
}

impl ToUInt for Const<214> {
    type Output = U214;
}

impl ToUInt for Const<215> {
    type Output = U215;
}

impl ToUInt for Const<216> {
    type Output = U216;
}

impl ToUInt for Const<217> {
    type Output = U217;
}

impl ToUInt for Const<218> {
    type Output = U218;
}

impl ToUInt for Const<219> {
    type Output = U219;
}

impl ToUInt for Const<220> {
    type Output = U220;
}

impl ToUInt for Const<221> {
    type Output = U221;
}

impl ToUInt for Const<222> {
    type Output = U222;
}

impl ToUInt for Const<223> {
    type Output = U223;
}

impl ToUInt for Const<224> {
    type Output = U224;
}

impl ToUInt for Const<225> {
    type Output = U225;
}

impl ToUInt for Const<226> {
    type Output = U226;
}

impl ToUInt for Const<227> {
    type Output = U227;
}

impl ToUInt for Const<228> {
    type Output = U228;
}

impl ToUInt for Const<229> {
    type Output = U229;
}

impl ToUInt for Const<230> {
    type Output = U230;
}

impl ToUInt for Const<231> {
    type Output = U231;
}

impl ToUInt for Const<232> {
    type Output = U232;
}

impl ToUInt for Const<233> {
    type Output = U233;
}

impl ToUInt for Const<234> {
    type Output = U234;
}

impl ToUInt for Const<235> {
    type Output = U235;
}

impl ToUInt for Const<236> {
    type Output = U236;
}

impl ToUInt for Const<237> {
    type Output = U237;
}

impl ToUInt for Const<238> {
    type Output = U238;
}

impl ToUInt for Const<239> {
    type Output = U239;
}

impl ToUInt for Const<240> {
    type Output = U240;
}

impl ToUInt for Const<241> {
    type Output = U241;
}

impl ToUInt for Const<242> {
    type Output = U242;
}

impl ToUInt for Const<243> {
    type Output = U243;
}

impl ToUInt for Const<244> {
    type Output = U244;
}

impl ToUInt for Const<245> {
    type Output = U245;
}

impl ToUInt for Const<246> {
    type Output = U246;
}

impl ToUInt for Const<247> {
    type Output = U247;
}

impl ToUInt for Const<248> {
    type Output = U248;
}

impl ToUInt for Const<249> {
    type Output = U249;
}

impl ToUInt for Const<250> {
    type Output = U250;
}

impl ToUInt for Const<251> {
    type Output = U251;
}

impl ToUInt for Const<252> {
    type Output = U252;
}

impl ToUInt for Const<253> {
    type Output = U253;
}

impl ToUInt for Const<254> {
    type Output = U254;
}

impl ToUInt for Const<255> {
    type Output = U255;
}

impl ToUInt for Const<256> {
    type Output = U256;
}

impl ToUInt for Const<257> {
    type Output = U257;
}

impl ToUInt for Const<258> {
    type Output = U258;
}

impl ToUInt for Const<259> {
    type Output = U259;
}

impl ToUInt for Const<260> {
    type Output = U260;
}

impl ToUInt for Const<261> {
    type Output = U261;
}

impl ToUInt for Const<262> {
    type Output = U262;
}

impl ToUInt for Const<263> {
    type Output = U263;
}

impl ToUInt for Const<264> {
    type Output = U264;
}

impl ToUInt for Const<265> {
    type Output = U265;
}

impl ToUInt for Const<266> {
    type Output = U266;
}

impl ToUInt for Const<267> {
    type Output = U267;
}

impl ToUInt for Const<268> {
    type Output = U268;
}

impl ToUInt for Const<269> {
    type Output = U269;
}

impl ToUInt for Const<270> {
    type Output = U270;
}

impl ToUInt for Const<271> {
    type Output = U271;
}

impl ToUInt for Const<272> {
    type Output = U272;
}

impl ToUInt for Const<273> {
    type Output = U273;
}

impl ToUInt for Const<274> {
    type Output = U274;
}

impl ToUInt for Const<275> {
    type Output = U275;
}

impl ToUInt for Const<276> {
    type Output = U276;
}

impl ToUInt for Const<277> {
    type Output = U277;
}

impl ToUInt for Const<278> {
    type Output = U278;
}

impl ToUInt for Const<279> {
    type Output = U279;
}

impl ToUInt for Const<280> {
    type Output = U280;
}

impl ToUInt for Const<281> {
    type Output = U281;
}

impl ToUInt for Const<282> {
    type Output = U282;
}

impl ToUInt for Const<283> {
    type Output = U283;
}

impl ToUInt for Const<284> {
    type Output = U284;
}

impl ToUInt for Const<285> {
    type Output = U285;
}

impl ToUInt for Const<286> {
    type Output = U286;
}

impl ToUInt for Const<287> {
    type Output = U287;
}

impl ToUInt for Const<288> {
    type Output = U288;
}

impl ToUInt for Const<289> {
    type Output = U289;
}

impl ToUInt for Const<290> {
    type Output = U290;
}

impl ToUInt for Const<291> {
    type Output = U291;
}

impl ToUInt for Const<292> {
    type Output = U292;
}

impl ToUInt for Const<293> {
    type Output = U293;
}

impl ToUInt for Const<294> {
    type Output = U294;
}

impl ToUInt for Const<295> {
    type Output = U295;
}

impl ToUInt for Const<296> {
    type Output = U296;
}

impl ToUInt for Const<297> {
    type Output = U297;
}

impl ToUInt for Const<298> {
    type Output = U298;
}

impl ToUInt for Const<299> {
    type Output = U299;
}

impl ToUInt for Const<300> {
    type Output = U300;
}

impl ToUInt for Const<301> {
    type Output = U301;
}

impl ToUInt for Const<302> {
    type Output = U302;
}

impl ToUInt for Const<303> {
    type Output = U303;
}

impl ToUInt for Const<304> {
    type Output = U304;
}

impl ToUInt for Const<305> {
    type Output = U305;
}

impl ToUInt for Const<306> {
    type Output = U306;
}

impl ToUInt for Const<307> {
    type Output = U307;
}

impl ToUInt for Const<308> {
    type Output = U308;
}

impl ToUInt for Const<309> {
    type Output = U309;
}

impl ToUInt for Const<310> {
    type Output = U310;
}

impl ToUInt for Const<311> {
    type Output = U311;
}

impl ToUInt for Const<312> {
    type Output = U312;
}

impl ToUInt for Const<313> {
    type Output = U313;
}

impl ToUInt for Const<314> {
    type Output = U314;
}

impl ToUInt for Const<315> {
    type Output = U315;
}

impl ToUInt for Const<316> {
    type Output = U316;
}

impl ToUInt for Const<317> {
    type Output = U317;
}

impl ToUInt for Const<318> {
    type Output = U318;
}

impl ToUInt for Const<319> {
    type Output = U319;
}

impl ToUInt for Const<320> {
    type Output = U320;
}

impl ToUInt for Const<321> {
    type Output = U321;
}

impl ToUInt for Const<322> {
    type Output = U322;
}

impl ToUInt for Const<323> {
    type Output = U323;
}

impl ToUInt for Const<324> {
    type Output = U324;
}

impl ToUInt for Const<325> {
    type Output = U325;
}

impl ToUInt for Const<326> {
    type Output = U326;
}

impl ToUInt for Const<327> {
    type Output = U327;
}

impl ToUInt for Const<328> {
    type Output = U328;
}

impl ToUInt for Const<329> {
    type Output = U329;
}

impl ToUInt for Const<330> {
    type Output = U330;
}

impl ToUInt for Const<331> {
    type Output = U331;
}

impl ToUInt for Const<332> {
    type Output = U332;
}

impl ToUInt for Const<333> {
    type Output = U333;
}

impl ToUInt for Const<334> {
    type Output = U334;
}

impl ToUInt for Const<335> {
    type Output = U335;
}

impl ToUInt for Const<336> {
    type Output = U336;
}

impl ToUInt for Const<337> {
    type Output = U337;
}

impl ToUInt for Const<338> {
    type Output = U338;
}

impl ToUInt for Const<339> {
    type Output = U339;
}

impl ToUInt for Const<340> {
    type Output = U340;
}

impl ToUInt for Const<341> {
    type Output = U341;
}

impl ToUInt for Const<342> {
    type Output = U342;
}

impl ToUInt for Const<343> {
    type Output = U343;
}

impl ToUInt for Const<344> {
    type Output = U344;
}

impl ToUInt for Const<345> {
    type Output = U345;
}

impl ToUInt for Const<346> {
    type Output = U346;
}

impl ToUInt for Const<347> {
    type Output = U347;
}

impl ToUInt for Const<348> {
    type Output = U348;
}

impl ToUInt for Const<349> {
    type Output = U349;
}

impl ToUInt for Const<350> {
    type Output = U350;
}

impl ToUInt for Const<351> {
    type Output = U351;
}

impl ToUInt for Const<352> {
    type Output = U352;
}

impl ToUInt for Const<353> {
    type Output = U353;
}

impl ToUInt for Const<354> {
    type Output = U354;
}

impl ToUInt for Const<355> {
    type Output = U355;
}

impl ToUInt for Const<356> {
    type Output = U356;
}

impl ToUInt for Const<357> {
    type Output = U357;
}

impl ToUInt for Const<358> {
    type Output = U358;
}

impl ToUInt for Const<359> {
    type Output = U359;
}

impl ToUInt for Const<360> {
    type Output = U360;
}

impl ToUInt for Const<361> {
    type Output = U361;
}

impl ToUInt for Const<362> {
    type Output = U362;
}

impl ToUInt for Const<363> {
    type Output = U363;
}

impl ToUInt for Const<364> {
    type Output = U364;
}

impl ToUInt for Const<365> {
    type Output = U365;
}

impl ToUInt for Const<366> {
    type Output = U366;
}

impl ToUInt for Const<367> {
    type Output = U367;
}

impl ToUInt for Const<368> {
    type Output = U368;
}

impl ToUInt for Const<369> {
    type Output = U369;
}

impl ToUInt for Const<370> {
    type Output = U370;
}

impl ToUInt for Const<371> {
    type Output = U371;
}

impl ToUInt for Const<372> {
    type Output = U372;
}

impl ToUInt for Const<373> {
    type Output = U373;
}

impl ToUInt for Const<374> {
    type Output = U374;
}

impl ToUInt for Const<375> {
    type Output = U375;
}

impl ToUInt for Const<376> {
    type Output = U376;
}

impl ToUInt for Const<377> {
    type Output = U377;
}

impl ToUInt for Const<378> {
    type Output = U378;
}

impl ToUInt for Const<379> {
    type Output = U379;
}

impl ToUInt for Const<380> {
    type Output = U380;
}

impl ToUInt for Const<381> {
    type Output = U381;
}

impl ToUInt for Const<382> {
    type Output = U382;
}

impl ToUInt for Const<383> {
    type Output = U383;
}

impl ToUInt for Const<384> {
    type Output = U384;
}

impl ToUInt for Const<385> {
    type Output = U385;
}

impl ToUInt for Const<386> {
    type Output = U386;
}

impl ToUInt for Const<387> {
    type Output = U387;
}

impl ToUInt for Const<388> {
    type Output = U388;
}

impl ToUInt for Const<389> {
    type Output = U389;
}

impl ToUInt for Const<390> {
    type Output = U390;
}

impl ToUInt for Const<391> {
    type Output = U391;
}

impl ToUInt for Const<392> {
    type Output = U392;
}

impl ToUInt for Const<393> {
    type Output = U393;
}

impl ToUInt for Const<394> {
    type Output = U394;
}

impl ToUInt for Const<395> {
    type Output = U395;
}

impl ToUInt for Const<396> {
    type Output = U396;
}

impl ToUInt for Const<397> {
    type Output = U397;
}

impl ToUInt for Const<398> {
    type Output = U398;
}

impl ToUInt for Const<399> {
    type Output = U399;
}

impl ToUInt for Const<400> {
    type Output = U400;
}

impl ToUInt for Const<401> {
    type Output = U401;
}

impl ToUInt for Const<402> {
    type Output = U402;
}

impl ToUInt for Const<403> {
    type Output = U403;
}

impl ToUInt for Const<404> {
    type Output = U404;
}

impl ToUInt for Const<405> {
    type Output = U405;
}

impl ToUInt for Const<406> {
    type Output = U406;
}

impl ToUInt for Const<407> {
    type Output = U407;
}

impl ToUInt for Const<408> {
    type Output = U408;
}

impl ToUInt for Const<409> {
    type Output = U409;
}

impl ToUInt for Const<410> {
    type Output = U410;
}

impl ToUInt for Const<411> {
    type Output = U411;
}

impl ToUInt for Const<412> {
    type Output = U412;
}

impl ToUInt for Const<413> {
    type Output = U413;
}

impl ToUInt for Const<414> {
    type Output = U414;
}

impl ToUInt for Const<415> {
    type Output = U415;
}

impl ToUInt for Const<416> {
    type Output = U416;
}

impl ToUInt for Const<417> {
    type Output = U417;
}

impl ToUInt for Const<418> {
    type Output = U418;
}

impl ToUInt for Const<419> {
    type Output = U419;
}

impl ToUInt for Const<420> {
    type Output = U420;
}

impl ToUInt for Const<421> {
    type Output = U421;
}

impl ToUInt for Const<422> {
    type Output = U422;
}

impl ToUInt for Const<423> {
    type Output = U423;
}

impl ToUInt for Const<424> {
    type Output = U424;
}

impl ToUInt for Const<425> {
    type Output = U425;
}

impl ToUInt for Const<426> {
    type Output = U426;
}

impl ToUInt for Const<427> {
    type Output = U427;
}

impl ToUInt for Const<428> {
    type Output = U428;
}

impl ToUInt for Const<429> {
    type Output = U429;
}

impl ToUInt for Const<430> {
    type Output = U430;
}

impl ToUInt for Const<431> {
    type Output = U431;
}

impl ToUInt for Const<432> {
    type Output = U432;
}

impl ToUInt for Const<433> {
    type Output = U433;
}

impl ToUInt for Const<434> {
    type Output = U434;
}

impl ToUInt for Const<435> {
    type Output = U435;
}

impl ToUInt for Const<436> {
    type Output = U436;
}

impl ToUInt for Const<437> {
    type Output = U437;
}

impl ToUInt for Const<438> {
    type Output = U438;
}

impl ToUInt for Const<439> {
    type Output = U439;
}

impl ToUInt for Const<440> {
    type Output = U440;
}

impl ToUInt for Const<441> {
    type Output = U441;
}

impl ToUInt for Const<442> {
    type Output = U442;
}

impl ToUInt for Const<443> {
    type Output = U443;
}

impl ToUInt for Const<444> {
    type Output = U444;
}

impl ToUInt for Const<445> {
    type Output = U445;
}

impl ToUInt for Const<446> {
    type Output = U446;
}

impl ToUInt for Const<447> {
    type Output = U447;
}

impl ToUInt for Const<448> {
    type Output = U448;
}

impl ToUInt for Const<449> {
    type Output = U449;
}

impl ToUInt for Const<450> {
    type Output = U450;
}

impl ToUInt for Const<451> {
    type Output = U451;
}

impl ToUInt for Const<452> {
    type Output = U452;
}

impl ToUInt for Const<453> {
    type Output = U453;
}

impl ToUInt for Const<454> {
    type Output = U454;
}

impl ToUInt for Const<455> {
    type Output = U455;
}

impl ToUInt for Const<456> {
    type Output = U456;
}

impl ToUInt for Const<457> {
    type Output = U457;
}

impl ToUInt for Const<458> {
    type Output = U458;
}

impl ToUInt for Const<459> {
    type Output = U459;
}

impl ToUInt for Const<460> {
    type Output = U460;
}

impl ToUInt for Const<461> {
    type Output = U461;
}

impl ToUInt for Const<462> {
    type Output = U462;
}

impl ToUInt for Const<463> {
    type Output = U463;
}

impl ToUInt for Const<464> {
    type Output = U464;
}

impl ToUInt for Const<465> {
    type Output = U465;
}

impl ToUInt for Const<466> {
    type Output = U466;
}

impl ToUInt for Const<467> {
    type Output = U467;
}

impl ToUInt for Const<468> {
    type Output = U468;
}

impl ToUInt for Const<469> {
    type Output = U469;
}

impl ToUInt for Const<470> {
    type Output = U470;
}

impl ToUInt for Const<471> {
    type Output = U471;
}

impl ToUInt for Const<472> {
    type Output = U472;
}

impl ToUInt for Const<473> {
    type Output = U473;
}

impl ToUInt for Const<474> {
    type Output = U474;
}

impl ToUInt for Const<475> {
    type Output = U475;
}

impl ToUInt for Const<476> {
    type Output = U476;
}

impl ToUInt for Const<477> {
    type Output = U477;
}

impl ToUInt for Const<478> {
    type Output = U478;
}

impl ToUInt for Const<479> {
    type Output = U479;
}

impl ToUInt for Const<480> {
    type Output = U480;
}

impl ToUInt for Const<481> {
    type Output = U481;
}

impl ToUInt for Const<482> {
    type Output = U482;
}

impl ToUInt for Const<483> {
    type Output = U483;
}

impl ToUInt for Const<484> {
    type Output = U484;
}

impl ToUInt for Const<485> {
    type Output = U485;
}

impl ToUInt for Const<486> {
    type Output = U486;
}

impl ToUInt for Const<487> {
    type Output = U487;
}

impl ToUInt for Const<488> {
    type Output = U488;
}

impl ToUInt for Const<489> {
    type Output = U489;
}

impl ToUInt for Const<490> {
    type Output = U490;
}

impl ToUInt for Const<491> {
    type Output = U491;
}

impl ToUInt for Const<492> {
    type Output = U492;
}

impl ToUInt for Const<493> {
    type Output = U493;
}

impl ToUInt for Const<494> {
    type Output = U494;
}

impl ToUInt for Const<495> {
    type Output = U495;
}

impl ToUInt for Const<496> {
    type Output = U496;
}

impl ToUInt for Const<497> {
    type Output = U497;
}

impl ToUInt for Const<498> {
    type Output = U498;
}

impl ToUInt for Const<499> {
    type Output = U499;
}

impl ToUInt for Const<500> {
    type Output = U500;
}

impl ToUInt for Const<501> {
    type Output = U501;
}

impl ToUInt for Const<502> {
    type Output = U502;
}

impl ToUInt for Const<503> {
    type Output = U503;
}

impl ToUInt for Const<504> {
    type Output = U504;
}

impl ToUInt for Const<505> {
    type Output = U505;
}

impl ToUInt for Const<506> {
    type Output = U506;
}

impl ToUInt for Const<507> {
    type Output = U507;
}

impl ToUInt for Const<508> {
    type Output = U508;
}

impl ToUInt for Const<509> {
    type Output = U509;
}

impl ToUInt for Const<510> {
    type Output = U510;
}

impl ToUInt for Const<511> {
    type Output = U511;
}

impl ToUInt for Const<512> {
    type Output = U512;
}

impl ToUInt for Const<513> {
    type Output = U513;
}

impl ToUInt for Const<514> {
    type Output = U514;
}

impl ToUInt for Const<515> {
    type Output = U515;
}

impl ToUInt for Const<516> {
    type Output = U516;
}

impl ToUInt for Const<517> {
    type Output = U517;
}

impl ToUInt for Const<518> {
    type Output = U518;
}

impl ToUInt for Const<519> {
    type Output = U519;
}

impl ToUInt for Const<520> {
    type Output = U520;
}

impl ToUInt for Const<521> {
    type Output = U521;
}

impl ToUInt for Const<522> {
    type Output = U522;
}

impl ToUInt for Const<523> {
    type Output = U523;
}

impl ToUInt for Const<524> {
    type Output = U524;
}

impl ToUInt for Const<525> {
    type Output = U525;
}

impl ToUInt for Const<526> {
    type Output = U526;
}

impl ToUInt for Const<527> {
    type Output = U527;
}

impl ToUInt for Const<528> {
    type Output = U528;
}

impl ToUInt for Const<529> {
    type Output = U529;
}

impl ToUInt for Const<530> {
    type Output = U530;
}

impl ToUInt for Const<531> {
    type Output = U531;
}

impl ToUInt for Const<532> {
    type Output = U532;
}

impl ToUInt for Const<533> {
    type Output = U533;
}

impl ToUInt for Const<534> {
    type Output = U534;
}

impl ToUInt for Const<535> {
    type Output = U535;
}

impl ToUInt for Const<536> {
    type Output = U536;
}

impl ToUInt for Const<537> {
    type Output = U537;
}

impl ToUInt for Const<538> {
    type Output = U538;
}

impl ToUInt for Const<539> {
    type Output = U539;
}

impl ToUInt for Const<540> {
    type Output = U540;
}

impl ToUInt for Const<541> {
    type Output = U541;
}

impl ToUInt for Const<542> {
    type Output = U542;
}

impl ToUInt for Const<543> {
    type Output = U543;
}

impl ToUInt for Const<544> {
    type Output = U544;
}

impl ToUInt for Const<545> {
    type Output = U545;
}

impl ToUInt for Const<546> {
    type Output = U546;
}

impl ToUInt for Const<547> {
    type Output = U547;
}

impl ToUInt for Const<548> {
    type Output = U548;
}

impl ToUInt for Const<549> {
    type Output = U549;
}

impl ToUInt for Const<550> {
    type Output = U550;
}

impl ToUInt for Const<551> {
    type Output = U551;
}

impl ToUInt for Const<552> {
    type Output = U552;
}

impl ToUInt for Const<553> {
    type Output = U553;
}

impl ToUInt for Const<554> {
    type Output = U554;
}

impl ToUInt for Const<555> {
    type Output = U555;
}

impl ToUInt for Const<556> {
    type Output = U556;
}

impl ToUInt for Const<557> {
    type Output = U557;
}

impl ToUInt for Const<558> {
    type Output = U558;
}

impl ToUInt for Const<559> {
    type Output = U559;
}

impl ToUInt for Const<560> {
    type Output = U560;
}

impl ToUInt for Const<561> {
    type Output = U561;
}

impl ToUInt for Const<562> {
    type Output = U562;
}

impl ToUInt for Const<563> {
    type Output = U563;
}

impl ToUInt for Const<564> {
    type Output = U564;
}

impl ToUInt for Const<565> {
    type Output = U565;
}

impl ToUInt for Const<566> {
    type Output = U566;
}

impl ToUInt for Const<567> {
    type Output = U567;
}

impl ToUInt for Const<568> {
    type Output = U568;
}

impl ToUInt for Const<569> {
    type Output = U569;
}

impl ToUInt for Const<570> {
    type Output = U570;
}

impl ToUInt for Const<571> {
    type Output = U571;
}

impl ToUInt for Const<572> {
    type Output = U572;
}

impl ToUInt for Const<573> {
    type Output = U573;
}

impl ToUInt for Const<574> {
    type Output = U574;
}

impl ToUInt for Const<575> {
    type Output = U575;
}

impl ToUInt for Const<576> {
    type Output = U576;
}

impl ToUInt for Const<577> {
    type Output = U577;
}

impl ToUInt for Const<578> {
    type Output = U578;
}

impl ToUInt for Const<579> {
    type Output = U579;
}

impl ToUInt for Const<580> {
    type Output = U580;
}

impl ToUInt for Const<581> {
    type Output = U581;
}

impl ToUInt for Const<582> {
    type Output = U582;
}

impl ToUInt for Const<583> {
    type Output = U583;
}

impl ToUInt for Const<584> {
    type Output = U584;
}

impl ToUInt for Const<585> {
    type Output = U585;
}

impl ToUInt for Const<586> {
    type Output = U586;
}

impl ToUInt for Const<587> {
    type Output = U587;
}

impl ToUInt for Const<588> {
    type Output = U588;
}

impl ToUInt for Const<589> {
    type Output = U589;
}

impl ToUInt for Const<590> {
    type Output = U590;
}

impl ToUInt for Const<591> {
    type Output = U591;
}

impl ToUInt for Const<592> {
    type Output = U592;
}

impl ToUInt for Const<593> {
    type Output = U593;
}

impl ToUInt for Const<594> {
    type Output = U594;
}

impl ToUInt for Const<595> {
    type Output = U595;
}

impl ToUInt for Const<596> {
    type Output = U596;
}

impl ToUInt for Const<597> {
    type Output = U597;
}

impl ToUInt for Const<598> {
    type Output = U598;
}

impl ToUInt for Const<599> {
    type Output = U599;
}

impl ToUInt for Const<600> {
    type Output = U600;
}

impl ToUInt for Const<601> {
    type Output = U601;
}

impl ToUInt for Const<602> {
    type Output = U602;
}

impl ToUInt for Const<603> {
    type Output = U603;
}

impl ToUInt for Const<604> {
    type Output = U604;
}

impl ToUInt for Const<605> {
    type Output = U605;
}

impl ToUInt for Const<606> {
    type Output = U606;
}

impl ToUInt for Const<607> {
    type Output = U607;
}

impl ToUInt for Const<608> {
    type Output = U608;
}

impl ToUInt for Const<609> {
    type Output = U609;
}

impl ToUInt for Const<610> {
    type Output = U610;
}

impl ToUInt for Const<611> {
    type Output = U611;
}

impl ToUInt for Const<612> {
    type Output = U612;
}

impl ToUInt for Const<613> {
    type Output = U613;
}

impl ToUInt for Const<614> {
    type Output = U614;
}

impl ToUInt for Const<615> {
    type Output = U615;
}

impl ToUInt for Const<616> {
    type Output = U616;
}

impl ToUInt for Const<617> {
    type Output = U617;
}

impl ToUInt for Const<618> {
    type Output = U618;
}

impl ToUInt for Const<619> {
    type Output = U619;
}

impl ToUInt for Const<620> {
    type Output = U620;
}

impl ToUInt for Const<621> {
    type Output = U621;
}

impl ToUInt for Const<622> {
    type Output = U622;
}

impl ToUInt for Const<623> {
    type Output = U623;
}

impl ToUInt for Const<624> {
    type Output = U624;
}

impl ToUInt for Const<625> {
    type Output = U625;
}

impl ToUInt for Const<626> {
    type Output = U626;
}

impl ToUInt for Const<627> {
    type Output = U627;
}

impl ToUInt for Const<628> {
    type Output = U628;
}

impl ToUInt for Const<629> {
    type Output = U629;
}

impl ToUInt for Const<630> {
    type Output = U630;
}

impl ToUInt for Const<631> {
    type Output = U631;
}

impl ToUInt for Const<632> {
    type Output = U632;
}

impl ToUInt for Const<633> {
    type Output = U633;
}

impl ToUInt for Const<634> {
    type Output = U634;
}

impl ToUInt for Const<635> {
    type Output = U635;
}

impl ToUInt for Const<636> {
    type Output = U636;
}

impl ToUInt for Const<637> {
    type Output = U637;
}

impl ToUInt for Const<638> {
    type Output = U638;
}

impl ToUInt for Const<639> {
    type Output = U639;
}

impl ToUInt for Const<640> {
    type Output = U640;
}

impl ToUInt for Const<641> {
    type Output = U641;
}

impl ToUInt for Const<642> {
    type Output = U642;
}

impl ToUInt for Const<643> {
    type Output = U643;
}

impl ToUInt for Const<644> {
    type Output = U644;
}

impl ToUInt for Const<645> {
    type Output = U645;
}

impl ToUInt for Const<646> {
    type Output = U646;
}

impl ToUInt for Const<647> {
    type Output = U647;
}

impl ToUInt for Const<648> {
    type Output = U648;
}

impl ToUInt for Const<649> {
    type Output = U649;
}

impl ToUInt for Const<650> {
    type Output = U650;
}

impl ToUInt for Const<651> {
    type Output = U651;
}

impl ToUInt for Const<652> {
    type Output = U652;
}

impl ToUInt for Const<653> {
    type Output = U653;
}

impl ToUInt for Const<654> {
    type Output = U654;
}

impl ToUInt for Const<655> {
    type Output = U655;
}

impl ToUInt for Const<656> {
    type Output = U656;
}

impl ToUInt for Const<657> {
    type Output = U657;
}

impl ToUInt for Const<658> {
    type Output = U658;
}

impl ToUInt for Const<659> {
    type Output = U659;
}

impl ToUInt for Const<660> {
    type Output = U660;
}

impl ToUInt for Const<661> {
    type Output = U661;
}

impl ToUInt for Const<662> {
    type Output = U662;
}

impl ToUInt for Const<663> {
    type Output = U663;
}

impl ToUInt for Const<664> {
    type Output = U664;
}

impl ToUInt for Const<665> {
    type Output = U665;
}

impl ToUInt for Const<666> {
    type Output = U666;
}

impl ToUInt for Const<667> {
    type Output = U667;
}

impl ToUInt for Const<668> {
    type Output = U668;
}

impl ToUInt for Const<669> {
    type Output = U669;
}

impl ToUInt for Const<670> {
    type Output = U670;
}

impl ToUInt for Const<671> {
    type Output = U671;
}

impl ToUInt for Const<672> {
    type Output = U672;
}

impl ToUInt for Const<673> {
    type Output = U673;
}

impl ToUInt for Const<674> {
    type Output = U674;
}

impl ToUInt for Const<675> {
    type Output = U675;
}

impl ToUInt for Const<676> {
    type Output = U676;
}

impl ToUInt for Const<677> {
    type Output = U677;
}

impl ToUInt for Const<678> {
    type Output = U678;
}

impl ToUInt for Const<679> {
    type Output = U679;
}

impl ToUInt for Const<680> {
    type Output = U680;
}

impl ToUInt for Const<681> {
    type Output = U681;
}

impl ToUInt for Const<682> {
    type Output = U682;
}

impl ToUInt for Const<683> {
    type Output = U683;
}

impl ToUInt for Const<684> {
    type Output = U684;
}

impl ToUInt for Const<685> {
    type Output = U685;
}

impl ToUInt for Const<686> {
    type Output = U686;
}

impl ToUInt for Const<687> {
    type Output = U687;
}

impl ToUInt for Const<688> {
    type Output = U688;
}

impl ToUInt for Const<689> {
    type Output = U689;
}

impl ToUInt for Const<690> {
    type Output = U690;
}

impl ToUInt for Const<691> {
    type Output = U691;
}

impl ToUInt for Const<692> {
    type Output = U692;
}

impl ToUInt for Const<693> {
    type Output = U693;
}

impl ToUInt for Const<694> {
    type Output = U694;
}

impl ToUInt for Const<695> {
    type Output = U695;
}

impl ToUInt for Const<696> {
    type Output = U696;
}

impl ToUInt for Const<697> {
    type Output = U697;
}

impl ToUInt for Const<698> {
    type Output = U698;
}

impl ToUInt for Const<699> {
    type Output = U699;
}

impl ToUInt for Const<700> {
    type Output = U700;
}

impl ToUInt for Const<701> {
    type Output = U701;
}

impl ToUInt for Const<702> {
    type Output = U702;
}

impl ToUInt for Const<703> {
    type Output = U703;
}

impl ToUInt for Const<704> {
    type Output = U704;
}

impl ToUInt for Const<705> {
    type Output = U705;
}

impl ToUInt for Const<706> {
    type Output = U706;
}

impl ToUInt for Const<707> {
    type Output = U707;
}

impl ToUInt for Const<708> {
    type Output = U708;
}

impl ToUInt for Const<709> {
    type Output = U709;
}

impl ToUInt for Const<710> {
    type Output = U710;
}

impl ToUInt for Const<711> {
    type Output = U711;
}

impl ToUInt for Const<712> {
    type Output = U712;
}

impl ToUInt for Const<713> {
    type Output = U713;
}

impl ToUInt for Const<714> {
    type Output = U714;
}

impl ToUInt for Const<715> {
    type Output = U715;
}

impl ToUInt for Const<716> {
    type Output = U716;
}

impl ToUInt for Const<717> {
    type Output = U717;
}

impl ToUInt for Const<718> {
    type Output = U718;
}

impl ToUInt for Const<719> {
    type Output = U719;
}

impl ToUInt for Const<720> {
    type Output = U720;
}

impl ToUInt for Const<721> {
    type Output = U721;
}

impl ToUInt for Const<722> {
    type Output = U722;
}

impl ToUInt for Const<723> {
    type Output = U723;
}

impl ToUInt for Const<724> {
    type Output = U724;
}

impl ToUInt for Const<725> {
    type Output = U725;
}

impl ToUInt for Const<726> {
    type Output = U726;
}

impl ToUInt for Const<727> {
    type Output = U727;
}

impl ToUInt for Const<728> {
    type Output = U728;
}

impl ToUInt for Const<729> {
    type Output = U729;
}

impl ToUInt for Const<730> {
    type Output = U730;
}

impl ToUInt for Const<731> {
    type Output = U731;
}

impl ToUInt for Const<732> {
    type Output = U732;
}

impl ToUInt for Const<733> {
    type Output = U733;
}

impl ToUInt for Const<734> {
    type Output = U734;
}

impl ToUInt for Const<735> {
    type Output = U735;
}

impl ToUInt for Const<736> {
    type Output = U736;
}

impl ToUInt for Const<737> {
    type Output = U737;
}

impl ToUInt for Const<738> {
    type Output = U738;
}

impl ToUInt for Const<739> {
    type Output = U739;
}

impl ToUInt for Const<740> {
    type Output = U740;
}

impl ToUInt for Const<741> {
    type Output = U741;
}

impl ToUInt for Const<742> {
    type Output = U742;
}

impl ToUInt for Const<743> {
    type Output = U743;
}

impl ToUInt for Const<744> {
    type Output = U744;
}

impl ToUInt for Const<745> {
    type Output = U745;
}

impl ToUInt for Const<746> {
    type Output = U746;
}

impl ToUInt for Const<747> {
    type Output = U747;
}

impl ToUInt for Const<748> {
    type Output = U748;
}

impl ToUInt for Const<749> {
    type Output = U749;
}

impl ToUInt for Const<750> {
    type Output = U750;
}

impl ToUInt for Const<751> {
    type Output = U751;
}

impl ToUInt for Const<752> {
    type Output = U752;
}

impl ToUInt for Const<753> {
    type Output = U753;
}

impl ToUInt for Const<754> {
    type Output = U754;
}

impl ToUInt for Const<755> {
    type Output = U755;
}

impl ToUInt for Const<756> {
    type Output = U756;
}

impl ToUInt for Const<757> {
    type Output = U757;
}

impl ToUInt for Const<758> {
    type Output = U758;
}

impl ToUInt for Const<759> {
    type Output = U759;
}

impl ToUInt for Const<760> {
    type Output = U760;
}

impl ToUInt for Const<761> {
    type Output = U761;
}

impl ToUInt for Const<762> {
    type Output = U762;
}

impl ToUInt for Const<763> {
    type Output = U763;
}

impl ToUInt for Const<764> {
    type Output = U764;
}

impl ToUInt for Const<765> {
    type Output = U765;
}

impl ToUInt for Const<766> {
    type Output = U766;
}

impl ToUInt for Const<767> {
    type Output = U767;
}

impl ToUInt for Const<768> {
    type Output = U768;
}

impl ToUInt for Const<769> {
    type Output = U769;
}

impl ToUInt for Const<770> {
    type Output = U770;
}

impl ToUInt for Const<771> {
    type Output = U771;
}

impl ToUInt for Const<772> {
    type Output = U772;
}

impl ToUInt for Const<773> {
    type Output = U773;
}

impl ToUInt for Const<774> {
    type Output = U774;
}

impl ToUInt for Const<775> {
    type Output = U775;
}

impl ToUInt for Const<776> {
    type Output = U776;
}

impl ToUInt for Const<777> {
    type Output = U777;
}

impl ToUInt for Const<778> {
    type Output = U778;
}

impl ToUInt for Const<779> {
    type Output = U779;
}

impl ToUInt for Const<780> {
    type Output = U780;
}

impl ToUInt for Const<781> {
    type Output = U781;
}

impl ToUInt for Const<782> {
    type Output = U782;
}

impl ToUInt for Const<783> {
    type Output = U783;
}

impl ToUInt for Const<784> {
    type Output = U784;
}

impl ToUInt for Const<785> {
    type Output = U785;
}

impl ToUInt for Const<786> {
    type Output = U786;
}

impl ToUInt for Const<787> {
    type Output = U787;
}

impl ToUInt for Const<788> {
    type Output = U788;
}

impl ToUInt for Const<789> {
    type Output = U789;
}

impl ToUInt for Const<790> {
    type Output = U790;
}

impl ToUInt for Const<791> {
    type Output = U791;
}

impl ToUInt for Const<792> {
    type Output = U792;
}

impl ToUInt for Const<793> {
    type Output = U793;
}

impl ToUInt for Const<794> {
    type Output = U794;
}

impl ToUInt for Const<795> {
    type Output = U795;
}

impl ToUInt for Const<796> {
    type Output = U796;
}

impl ToUInt for Const<797> {
    type Output = U797;
}

impl ToUInt for Const<798> {
    type Output = U798;
}

impl ToUInt for Const<799> {
    type Output = U799;
}

impl ToUInt for Const<800> {
    type Output = U800;
}

impl ToUInt for Const<801> {
    type Output = U801;
}

impl ToUInt for Const<802> {
    type Output = U802;
}

impl ToUInt for Const<803> {
    type Output = U803;
}

impl ToUInt for Const<804> {
    type Output = U804;
}

impl ToUInt for Const<805> {
    type Output = U805;
}

impl ToUInt for Const<806> {
    type Output = U806;
}

impl ToUInt for Const<807> {
    type Output = U807;
}

impl ToUInt for Const<808> {
    type Output = U808;
}

impl ToUInt for Const<809> {
    type Output = U809;
}

impl ToUInt for Const<810> {
    type Output = U810;
}

impl ToUInt for Const<811> {
    type Output = U811;
}

impl ToUInt for Const<812> {
    type Output = U812;
}

impl ToUInt for Const<813> {
    type Output = U813;
}

impl ToUInt for Const<814> {
    type Output = U814;
}

impl ToUInt for Const<815> {
    type Output = U815;
}

impl ToUInt for Const<816> {
    type Output = U816;
}

impl ToUInt for Const<817> {
    type Output = U817;
}

impl ToUInt for Const<818> {
    type Output = U818;
}

impl ToUInt for Const<819> {
    type Output = U819;
}

impl ToUInt for Const<820> {
    type Output = U820;
}

impl ToUInt for Const<821> {
    type Output = U821;
}

impl ToUInt for Const<822> {
    type Output = U822;
}

impl ToUInt for Const<823> {
    type Output = U823;
}

impl ToUInt for Const<824> {
    type Output = U824;
}

impl ToUInt for Const<825> {
    type Output = U825;
}

impl ToUInt for Const<826> {
    type Output = U826;
}

impl ToUInt for Const<827> {
    type Output = U827;
}

impl ToUInt for Const<828> {
    type Output = U828;
}

impl ToUInt for Const<829> {
    type Output = U829;
}

impl ToUInt for Const<830> {
    type Output = U830;
}

impl ToUInt for Const<831> {
    type Output = U831;
}

impl ToUInt for Const<832> {
    type Output = U832;
}

impl ToUInt for Const<833> {
    type Output = U833;
}

impl ToUInt for Const<834> {
    type Output = U834;
}

impl ToUInt for Const<835> {
    type Output = U835;
}

impl ToUInt for Const<836> {
    type Output = U836;
}

impl ToUInt for Const<837> {
    type Output = U837;
}

impl ToUInt for Const<838> {
    type Output = U838;
}

impl ToUInt for Const<839> {
    type Output = U839;
}

impl ToUInt for Const<840> {
    type Output = U840;
}

impl ToUInt for Const<841> {
    type Output = U841;
}

impl ToUInt for Const<842> {
    type Output = U842;
}

impl ToUInt for Const<843> {
    type Output = U843;
}

impl ToUInt for Const<844> {
    type Output = U844;
}

impl ToUInt for Const<845> {
    type Output = U845;
}

impl ToUInt for Const<846> {
    type Output = U846;
}

impl ToUInt for Const<847> {
    type Output = U847;
}

impl ToUInt for Const<848> {
    type Output = U848;
}

impl ToUInt for Const<849> {
    type Output = U849;
}

impl ToUInt for Const<850> {
    type Output = U850;
}

impl ToUInt for Const<851> {
    type Output = U851;
}

impl ToUInt for Const<852> {
    type Output = U852;
}

impl ToUInt for Const<853> {
    type Output = U853;
}

impl ToUInt for Const<854> {
    type Output = U854;
}

impl ToUInt for Const<855> {
    type Output = U855;
}

impl ToUInt for Const<856> {
    type Output = U856;
}

impl ToUInt for Const<857> {
    type Output = U857;
}

impl ToUInt for Const<858> {
    type Output = U858;
}

impl ToUInt for Const<859> {
    type Output = U859;
}

impl ToUInt for Const<860> {
    type Output = U860;
}

impl ToUInt for Const<861> {
    type Output = U861;
}

impl ToUInt for Const<862> {
    type Output = U862;
}

impl ToUInt for Const<863> {
    type Output = U863;
}

impl ToUInt for Const<864> {
    type Output = U864;
}

impl ToUInt for Const<865> {
    type Output = U865;
}

impl ToUInt for Const<866> {
    type Output = U866;
}

impl ToUInt for Const<867> {
    type Output = U867;
}

impl ToUInt for Const<868> {
    type Output = U868;
}

impl ToUInt for Const<869> {
    type Output = U869;
}

impl ToUInt for Const<870> {
    type Output = U870;
}

impl ToUInt for Const<871> {
    type Output = U871;
}

impl ToUInt for Const<872> {
    type Output = U872;
}

impl ToUInt for Const<873> {
    type Output = U873;
}

impl ToUInt for Const<874> {
    type Output = U874;
}

impl ToUInt for Const<875> {
    type Output = U875;
}

impl ToUInt for Const<876> {
    type Output = U876;
}

impl ToUInt for Const<877> {
    type Output = U877;
}

impl ToUInt for Const<878> {
    type Output = U878;
}

impl ToUInt for Const<879> {
    type Output = U879;
}

impl ToUInt for Const<880> {
    type Output = U880;
}

impl ToUInt for Const<881> {
    type Output = U881;
}

impl ToUInt for Const<882> {
    type Output = U882;
}

impl ToUInt for Const<883> {
    type Output = U883;
}

impl ToUInt for Const<884> {
    type Output = U884;
}

impl ToUInt for Const<885> {
    type Output = U885;
}

impl ToUInt for Const<886> {
    type Output = U886;
}

impl ToUInt for Const<887> {
    type Output = U887;
}

impl ToUInt for Const<888> {
    type Output = U888;
}

impl ToUInt for Const<889> {
    type Output = U889;
}

impl ToUInt for Const<890> {
    type Output = U890;
}

impl ToUInt for Const<891> {
    type Output = U891;
}

impl ToUInt for Const<892> {
    type Output = U892;
}

impl ToUInt for Const<893> {
    type Output = U893;
}

impl ToUInt for Const<894> {
    type Output = U894;
}

impl ToUInt for Const<895> {
    type Output = U895;
}

impl ToUInt for Const<896> {
    type Output = U896;
}

impl ToUInt for Const<897> {
    type Output = U897;
}

impl ToUInt for Const<898> {
    type Output = U898;
}

impl ToUInt for Const<899> {
    type Output = U899;
}

impl ToUInt for Const<900> {
    type Output = U900;
}

impl ToUInt for Const<901> {
    type Output = U901;
}

impl ToUInt for Const<902> {
    type Output = U902;
}

impl ToUInt for Const<903> {
    type Output = U903;
}

impl ToUInt for Const<904> {
    type Output = U904;
}

impl ToUInt for Const<905> {
    type Output = U905;
}

impl ToUInt for Const<906> {
    type Output = U906;
}

impl ToUInt for Const<907> {
    type Output = U907;
}

impl ToUInt for Const<908> {
    type Output = U908;
}

impl ToUInt for Const<909> {
    type Output = U909;
}

impl ToUInt for Const<910> {
    type Output = U910;
}

impl ToUInt for Const<911> {
    type Output = U911;
}

impl ToUInt for Const<912> {
    type Output = U912;
}

impl ToUInt for Const<913> {
    type Output = U913;
}

impl ToUInt for Const<914> {
    type Output = U914;
}

impl ToUInt for Const<915> {
    type Output = U915;
}

impl ToUInt for Const<916> {
    type Output = U916;
}

impl ToUInt for Const<917> {
    type Output = U917;
}

impl ToUInt for Const<918> {
    type Output = U918;
}

impl ToUInt for Const<919> {
    type Output = U919;
}

impl ToUInt for Const<920> {
    type Output = U920;
}

impl ToUInt for Const<921> {
    type Output = U921;
}

impl ToUInt for Const<922> {
    type Output = U922;
}

impl ToUInt for Const<923> {
    type Output = U923;
}

impl ToUInt for Const<924> {
    type Output = U924;
}

impl ToUInt for Const<925> {
    type Output = U925;
}

impl ToUInt for Const<926> {
    type Output = U926;
}

impl ToUInt for Const<927> {
    type Output = U927;
}

impl ToUInt for Const<928> {
    type Output = U928;
}

impl ToUInt for Const<929> {
    type Output = U929;
}

impl ToUInt for Const<930> {
    type Output = U930;
}

impl ToUInt for Const<931> {
    type Output = U931;
}

impl ToUInt for Const<932> {
    type Output = U932;
}

impl ToUInt for Const<933> {
    type Output = U933;
}

impl ToUInt for Const<934> {
    type Output = U934;
}

impl ToUInt for Const<935> {
    type Output = U935;
}

impl ToUInt for Const<936> {
    type Output = U936;
}

impl ToUInt for Const<937> {
    type Output = U937;
}

impl ToUInt for Const<938> {
    type Output = U938;
}

impl ToUInt for Const<939> {
    type Output = U939;
}

impl ToUInt for Const<940> {
    type Output = U940;
}

impl ToUInt for Const<941> {
    type Output = U941;
}

impl ToUInt for Const<942> {
    type Output = U942;
}

impl ToUInt for Const<943> {
    type Output = U943;
}

impl ToUInt for Const<944> {
    type Output = U944;
}

impl ToUInt for Const<945> {
    type Output = U945;
}

impl ToUInt for Const<946> {
    type Output = U946;
}

impl ToUInt for Const<947> {
    type Output = U947;
}

impl ToUInt for Const<948> {
    type Output = U948;
}

impl ToUInt for Const<949> {
    type Output = U949;
}

impl ToUInt for Const<950> {
    type Output = U950;
}

impl ToUInt for Const<951> {
    type Output = U951;
}

impl ToUInt for Const<952> {
    type Output = U952;
}

impl ToUInt for Const<953> {
    type Output = U953;
}

impl ToUInt for Const<954> {
    type Output = U954;
}

impl ToUInt for Const<955> {
    type Output = U955;
}

impl ToUInt for Const<956> {
    type Output = U956;
}

impl ToUInt for Const<957> {
    type Output = U957;
}

impl ToUInt for Const<958> {
    type Output = U958;
}

impl ToUInt for Const<959> {
    type Output = U959;
}

impl ToUInt for Const<960> {
    type Output = U960;
}

impl ToUInt for Const<961> {
    type Output = U961;
}

impl ToUInt for Const<962> {
    type Output = U962;
}

impl ToUInt for Const<963> {
    type Output = U963;
}

impl ToUInt for Const<964> {
    type Output = U964;
}

impl ToUInt for Const<965> {
    type Output = U965;
}

impl ToUInt for Const<966> {
    type Output = U966;
}

impl ToUInt for Const<967> {
    type Output = U967;
}

impl ToUInt for Const<968> {
    type Output = U968;
}

impl ToUInt for Const<969> {
    type Output = U969;
}

impl ToUInt for Const<970> {
    type Output = U970;
}

impl ToUInt for Const<971> {
    type Output = U971;
}

impl ToUInt for Const<972> {
    type Output = U972;
}

impl ToUInt for Const<973> {
    type Output = U973;
}

impl ToUInt for Const<974> {
    type Output = U974;
}

impl ToUInt for Const<975> {
    type Output = U975;
}

impl ToUInt for Const<976> {
    type Output = U976;
}

impl ToUInt for Const<977> {
    type Output = U977;
}

impl ToUInt for Const<978> {
    type Output = U978;
}

impl ToUInt for Const<979> {
    type Output = U979;
}

impl ToUInt for Const<980> {
    type Output = U980;
}

impl ToUInt for Const<981> {
    type Output = U981;
}

impl ToUInt for Const<982> {
    type Output = U982;
}

impl ToUInt for Const<983> {
    type Output = U983;
}

impl ToUInt for Const<984> {
    type Output = U984;
}

impl ToUInt for Const<985> {
    type Output = U985;
}

impl ToUInt for Const<986> {
    type Output = U986;
}

impl ToUInt for Const<987> {
    type Output = U987;
}

impl ToUInt for Const<988> {
    type Output = U988;
}

impl ToUInt for Const<989> {
    type Output = U989;
}

impl ToUInt for Const<990> {
    type Output = U990;
}

impl ToUInt for Const<991> {
    type Output = U991;
}

impl ToUInt for Const<992> {
    type Output = U992;
}

impl ToUInt for Const<993> {
    type Output = U993;
}

impl ToUInt for Const<994> {
    type Output = U994;
}

impl ToUInt for Const<995> {
    type Output = U995;
}

impl ToUInt for Const<996> {
    type Output = U996;
}

impl ToUInt for Const<997> {
    type Output = U997;
}

impl ToUInt for Const<998> {
    type Output = U998;
}

impl ToUInt for Const<999> {
    type Output = U999;
}

impl ToUInt for Const<1000> {
    type Output = U1000;
}

impl ToUInt for Const<1001> {
    type Output = U1001;
}

impl ToUInt for Const<1002> {
    type Output = U1002;
}

impl ToUInt for Const<1003> {
    type Output = U1003;
}

impl ToUInt for Const<1004> {
    type Output = U1004;
}

impl ToUInt for Const<1005> {
    type Output = U1005;
}

impl ToUInt for Const<1006> {
    type Output = U1006;
}

impl ToUInt for Const<1007> {
    type Output = U1007;
}

impl ToUInt for Const<1008> {
    type Output = U1008;
}

impl ToUInt for Const<1009> {
    type Output = U1009;
}

impl ToUInt for Const<1010> {
    type Output = U1010;
}

impl ToUInt for Const<1011> {
    type Output = U1011;
}

impl ToUInt for Const<1012> {
    type Output = U1012;
}

impl ToUInt for Const<1013> {
    type Output = U1013;
}

impl ToUInt for Const<1014> {
    type Output = U1014;
}

impl ToUInt for Const<1015> {
    type Output = U1015;
}

impl ToUInt for Const<1016> {
    type Output = U1016;
}

impl ToUInt for Const<1017> {
    type Output = U1017;
}

impl ToUInt for Const<1018> {
    type Output = U1018;
}

impl ToUInt for Const<1019> {
    type Output = U1019;
}

impl ToUInt for Const<1020> {
    type Output = U1020;
}

impl ToUInt for Const<1021> {
    type Output = U1021;
}

impl ToUInt for Const<1022> {
    type Output = U1022;
}

impl ToUInt for Const<1023> {
    type Output = U1023;
}

impl ToUInt for Const<1024> {
    type Output = U1024;
}

impl ToUInt for Const<3600> {
    type Output = U3600;
}

impl ToUInt for Const<2048> {
    type Output = U2048;
}

impl ToUInt for Const<4096> {
    type Output = U4096;
}

impl ToUInt for Const<8192> {
    type Output = U8192;
}

impl ToUInt for Const<16384> {
    type Output = U16384;
}

impl ToUInt for Const<32768> {
    type Output = U32768;
}

impl ToUInt for Const<65536> {
    type Output = U65536;
}

impl ToUInt for Const<131072> {
    type Output = U131072;
}

impl ToUInt for Const<262144> {
    type Output = U262144;
}

impl ToUInt for Const<524288> {
    type Output = U524288;
}

impl ToUInt for Const<1048576> {
    type Output = U1048576;
}

impl ToUInt for Const<2097152> {
    type Output = U2097152;
}

impl ToUInt for Const<4194304> {
    type Output = U4194304;
}

impl ToUInt for Const<8388608> {
    type Output = U8388608;
}

impl ToUInt for Const<16777216> {
    type Output = U16777216;
}

impl ToUInt for Const<33554432> {
    type Output = U33554432;
}

impl ToUInt for Const<67108864> {
    type Output = U67108864;
}

impl ToUInt for Const<134217728> {
    type Output = U134217728;
}

impl ToUInt for Const<268435456> {
    type Output = U268435456;
}

impl ToUInt for Const<536870912> {
    type Output = U536870912;
}

impl ToUInt for Const<1073741824> {
    type Output = U1073741824;
}

impl ToUInt for Const<2147483648> {
    type Output = U2147483648;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<4294967296> {
    type Output = U4294967296;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<8589934592> {
    type Output = U8589934592;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<17179869184> {
    type Output = U17179869184;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<34359738368> {
    type Output = U34359738368;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<68719476736> {
    type Output = U68719476736;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<137438953472> {
    type Output = U137438953472;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<274877906944> {
    type Output = U274877906944;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<549755813888> {
    type Output = U549755813888;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1099511627776> {
    type Output = U1099511627776;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<2199023255552> {
    type Output = U2199023255552;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<4398046511104> {
    type Output = U4398046511104;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<8796093022208> {
    type Output = U8796093022208;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<17592186044416> {
    type Output = U17592186044416;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<35184372088832> {
    type Output = U35184372088832;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<70368744177664> {
    type Output = U70368744177664;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<140737488355328> {
    type Output = U140737488355328;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<281474976710656> {
    type Output = U281474976710656;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<562949953421312> {
    type Output = U562949953421312;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1125899906842624> {
    type Output = U1125899906842624;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<2251799813685248> {
    type Output = U2251799813685248;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<4503599627370496> {
    type Output = U4503599627370496;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<9007199254740992> {
    type Output = U9007199254740992;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<18014398509481984> {
    type Output = U18014398509481984;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<36028797018963968> {
    type Output = U36028797018963968;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<72057594037927936> {
    type Output = U72057594037927936;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<144115188075855872> {
    type Output = U144115188075855872;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<288230376151711744> {
    type Output = U288230376151711744;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<576460752303423488> {
    type Output = U576460752303423488;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1152921504606846976> {
    type Output = U1152921504606846976;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<2305843009213693952> {
    type Output = U2305843009213693952;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<4611686018427387904> {
    type Output = U4611686018427387904;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<9223372036854775808> {
    type Output = U9223372036854775808;
}

impl ToUInt for Const<10000> {
    type Output = U10000;
}

impl ToUInt for Const<100000> {
    type Output = U100000;
}

impl ToUInt for Const<1000000> {
    type Output = U1000000;
}

impl ToUInt for Const<10000000> {
    type Output = U10000000;
}

impl ToUInt for Const<100000000> {
    type Output = U100000000;
}

impl ToUInt for Const<1000000000> {
    type Output = U1000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<10000000000> {
    type Output = U10000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<100000000000> {
    type Output = U100000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1000000000000> {
    type Output = U1000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<10000000000000> {
    type Output = U10000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<100000000000000> {
    type Output = U100000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1000000000000000> {
    type Output = U1000000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<10000000000000000> {
    type Output = U10000000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<100000000000000000> {
    type Output = U100000000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<1000000000000000000> {
    type Output = U1000000000000000000;
}

#[cfg(target_pointer_width = "64")]
impl ToUInt for Const<10000000000000000000> {
    type Output = U10000000000000000000;
}
