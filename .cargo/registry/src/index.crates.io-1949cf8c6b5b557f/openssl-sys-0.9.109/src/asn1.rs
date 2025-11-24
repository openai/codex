use libc::*;

use super::*;

// ASN.1 tag values
pub const V_ASN1_EOC: c_int = 0;
pub const V_ASN1_BOOLEAN: c_int = 1;
pub const V_ASN1_INTEGER: c_int = 2;
pub const V_ASN1_BIT_STRING: c_int = 3;
pub const V_ASN1_OCTET_STRING: c_int = 4;
pub const V_ASN1_NULL: c_int = 5;
pub const V_ASN1_OBJECT: c_int = 6;
pub const V_ASN1_OBJECT_DESCRIPTOR: c_int = 7;
pub const V_ASN1_EXTERNAL: c_int = 8;
pub const V_ASN1_REAL: c_int = 9;
pub const V_ASN1_ENUMERATED: c_int = 10;
pub const V_ASN1_UTF8STRING: c_int = 12;
pub const V_ASN1_SEQUENCE: c_int = 16;
pub const V_ASN1_SET: c_int = 17;
pub const V_ASN1_NUMERICSTRING: c_int = 18;
pub const V_ASN1_PRINTABLESTRING: c_int = 19;
pub const V_ASN1_T61STRING: c_int = 20;
pub const V_ASN1_TELETEXSTRING: c_int = 20; // alias
pub const V_ASN1_VIDEOTEXSTRING: c_int = 21;
pub const V_ASN1_IA5STRING: c_int = 22;
pub const V_ASN1_UTCTIME: c_int = 23;
pub const V_ASN1_GENERALIZEDTIME: c_int = 24;
pub const V_ASN1_GRAPHICSTRING: c_int = 25;
pub const V_ASN1_ISO64STRING: c_int = 26;
pub const V_ASN1_VISIBLESTRING: c_int = 26; // alias
pub const V_ASN1_GENERALSTRING: c_int = 27;
pub const V_ASN1_UNIVERSALSTRING: c_int = 28;
pub const V_ASN1_BMPSTRING: c_int = 30;

pub const MBSTRING_FLAG: c_int = 0x1000;
pub const MBSTRING_UTF8: c_int = MBSTRING_FLAG;
pub const MBSTRING_ASC: c_int = MBSTRING_FLAG | 1;
pub const MBSTRING_BMP: c_int = MBSTRING_FLAG | 2;
pub const MBSTRING_UNIV: c_int = MBSTRING_FLAG | 4;
