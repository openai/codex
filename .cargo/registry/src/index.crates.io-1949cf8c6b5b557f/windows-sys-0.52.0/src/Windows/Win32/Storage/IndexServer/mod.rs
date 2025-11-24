#[cfg(feature = "Win32_System_Com_StructuredStorage")]
::windows_targets::link!("query.dll" "system" #[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"] fn BindIFilterFromStorage(pstg : super::super::System::Com::StructuredStorage:: IStorage, punkouter : ::windows_sys::core::IUnknown, ppiunk : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_System_Com")]
::windows_targets::link!("query.dll" "system" #[doc = "Required features: `\"Win32_System_Com\"`"] fn BindIFilterFromStream(pstm : super::super::System::Com:: IStream, punkouter : ::windows_sys::core::IUnknown, ppiunk : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("query.dll" "system" fn LoadIFilter(pwcspath : ::windows_sys::core::PCWSTR, punkouter : ::windows_sys::core::IUnknown, ppiunk : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("query.dll" "system" fn LoadIFilterEx(pwcspath : ::windows_sys::core::PCWSTR, dwflags : u32, riid : *const ::windows_sys::core::GUID, ppiunk : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
pub type IFilter = *mut ::core::ffi::c_void;
pub type IPhraseSink = *mut ::core::ffi::c_void;
pub const CHUNK_EOC: CHUNK_BREAKTYPE = 4i32;
pub const CHUNK_EOP: CHUNK_BREAKTYPE = 3i32;
pub const CHUNK_EOS: CHUNK_BREAKTYPE = 2i32;
pub const CHUNK_EOW: CHUNK_BREAKTYPE = 1i32;
pub const CHUNK_FILTER_OWNED_VALUE: CHUNKSTATE = 4i32;
pub const CHUNK_NO_BREAK: CHUNK_BREAKTYPE = 0i32;
pub const CHUNK_TEXT: CHUNKSTATE = 1i32;
pub const CHUNK_VALUE: CHUNKSTATE = 2i32;
pub const CIADMIN: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("::_nodocstore_::");
pub const CICAT_ALL_OPENED: u32 = 32u32;
pub const CICAT_GET_STATE: u32 = 16u32;
pub const CICAT_NO_QUERY: u32 = 8u32;
pub const CICAT_READONLY: u32 = 2u32;
pub const CICAT_STOPPED: u32 = 1u32;
pub const CICAT_WRITABLE: u32 = 4u32;
pub const CINULLCATALOG: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("::_noindex_::");
pub const CI_PROVIDER_ALL: u32 = 4294967295u32;
pub const CI_PROVIDER_INDEXING_SERVICE: u32 = 2u32;
pub const CI_PROVIDER_MSSEARCH: u32 = 1u32;
pub const CI_STATE_ANNEALING_MERGE: u32 = 8u32;
pub const CI_STATE_BATTERY_POLICY: u32 = 262144u32;
pub const CI_STATE_BATTERY_POWER: u32 = 2048u32;
pub const CI_STATE_CONTENT_SCAN_REQUIRED: u32 = 4u32;
pub const CI_STATE_DELETION_MERGE: u32 = 32768u32;
pub const CI_STATE_HIGH_CPU: u32 = 131072u32;
pub const CI_STATE_HIGH_IO: u32 = 256u32;
pub const CI_STATE_INDEX_MIGRATION_MERGE: u32 = 64u32;
pub const CI_STATE_LOW_DISK: u32 = 65536u32;
pub const CI_STATE_LOW_MEMORY: u32 = 128u32;
pub const CI_STATE_MASTER_MERGE: u32 = 2u32;
pub const CI_STATE_MASTER_MERGE_PAUSED: u32 = 512u32;
pub const CI_STATE_READING_USNS: u32 = 16384u32;
pub const CI_STATE_READ_ONLY: u32 = 1024u32;
pub const CI_STATE_RECOVERING: u32 = 32u32;
pub const CI_STATE_SCANNING: u32 = 16u32;
pub const CI_STATE_SHADOW_MERGE: u32 = 1u32;
pub const CI_STATE_STARTING: u32 = 8192u32;
pub const CI_STATE_USER_ACTIVE: u32 = 4096u32;
pub const CI_VERSION_WDS30: u32 = 258u32;
pub const CI_VERSION_WDS40: u32 = 265u32;
pub const CI_VERSION_WIN70: u32 = 1792u32;
pub const CLSID_INDEX_SERVER_DSO: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf9ae8980_7e52_11d0_8964_00c04fd611d7);
pub const DBKIND_GUID: DBKINDENUM = 6i32;
pub const DBKIND_GUID_NAME: DBKINDENUM = 0i32;
pub const DBKIND_GUID_PROPID: DBKINDENUM = 1i32;
pub const DBKIND_NAME: DBKINDENUM = 2i32;
pub const DBKIND_PGUID_NAME: DBKINDENUM = 3i32;
pub const DBKIND_PGUID_PROPID: DBKINDENUM = 4i32;
pub const DBKIND_PROPID: DBKINDENUM = 5i32;
pub const DBPROPSET_CIFRMWRKCORE_EXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xafafaca5_b5d1_11d0_8c62_00c04fc2db8d);
pub const DBPROPSET_FSCIFRMWRK_EXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa9bd1526_6a80_11d0_8c9d_0020af1d740e);
pub const DBPROPSET_MSIDXS_ROWSETEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xaa6ee6b0_e828_11d0_b23e_00aa0047fc01);
pub const DBPROPSET_QUERYEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa7ac77ed_f8d7_11ce_a798_0020f8008025);
pub const DBPROPSET_SESS_QUERYEXT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x63623309_2d8b_4d17_b152_6e2956c26a70);
pub const DBPROP_APPLICATION_NAME: u32 = 11u32;
pub const DBPROP_CATALOGLISTID: u32 = 9u32;
pub const DBPROP_CI_CATALOG_NAME: u32 = 2u32;
pub const DBPROP_CI_DEPTHS: u32 = 4u32;
pub const DBPROP_CI_EXCLUDE_SCOPES: u32 = 5u32;
pub const DBPROP_CI_INCLUDE_SCOPES: u32 = 3u32;
pub const DBPROP_CI_PROVIDER: u32 = 8u32;
pub const DBPROP_CI_QUERY_TYPE: u32 = 7u32;
pub const DBPROP_CI_SCOPE_FLAGS: u32 = 4u32;
pub const DBPROP_CI_SECURITY_ID: u32 = 6u32;
pub const DBPROP_CLIENT_CLSID: u32 = 3u32;
pub const DBPROP_DEFAULT_EQUALS_BEHAVIOR: u32 = 2u32;
pub const DBPROP_DEFERCATALOGVERIFICATION: u32 = 8u32;
pub const DBPROP_DEFERNONINDEXEDTRIMMING: u32 = 3u32;
pub const DBPROP_DONOTCOMPUTEEXPENSIVEPROPS: u32 = 15u32;
pub const DBPROP_ENABLEROWSETEVENTS: u32 = 16u32;
pub const DBPROP_FIRSTROWS: u32 = 7u32;
pub const DBPROP_FREETEXTANYTERM: u32 = 12u32;
pub const DBPROP_FREETEXTUSESTEMMING: u32 = 13u32;
pub const DBPROP_GENERATEPARSETREE: u32 = 10u32;
pub const DBPROP_GENERICOPTIONS_STRING: u32 = 6u32;
pub const DBPROP_IGNORENOISEONLYCLAUSES: u32 = 5u32;
pub const DBPROP_IGNORESBRI: u32 = 14u32;
pub const DBPROP_MACHINE: u32 = 2u32;
pub const DBPROP_USECONTENTINDEX: u32 = 2u32;
pub const DBPROP_USEEXTENDEDDBTYPES: u32 = 4u32;
pub const DBSETFUNC_ALL: u32 = 1u32;
pub const DBSETFUNC_DISTINCT: u32 = 2u32;
pub const DBSETFUNC_NONE: u32 = 0u32;
pub const FILTER_E_ACCESS: ::windows_sys::core::HRESULT = -2147215613i32;
pub const FILTER_E_EMBEDDING_UNAVAILABLE: ::windows_sys::core::HRESULT = -2147215609i32;
pub const FILTER_E_END_OF_CHUNKS: ::windows_sys::core::HRESULT = -2147215616i32;
pub const FILTER_E_LINK_UNAVAILABLE: ::windows_sys::core::HRESULT = -2147215608i32;
pub const FILTER_E_NO_MORE_TEXT: ::windows_sys::core::HRESULT = -2147215615i32;
pub const FILTER_E_NO_MORE_VALUES: ::windows_sys::core::HRESULT = -2147215614i32;
pub const FILTER_E_NO_TEXT: ::windows_sys::core::HRESULT = -2147215611i32;
pub const FILTER_E_NO_VALUES: ::windows_sys::core::HRESULT = -2147215610i32;
pub const FILTER_E_PASSWORD: ::windows_sys::core::HRESULT = -2147215605i32;
pub const FILTER_E_UNKNOWNFORMAT: ::windows_sys::core::HRESULT = -2147215604i32;
pub const FILTER_S_LAST_TEXT: ::windows_sys::core::HRESULT = 268041i32;
pub const FILTER_S_LAST_VALUES: ::windows_sys::core::HRESULT = 268042i32;
pub const FILTER_W_MONIKER_CLIPPED: ::windows_sys::core::HRESULT = 268036i32;
pub const GENERATE_METHOD_EXACT: u32 = 0u32;
pub const GENERATE_METHOD_INFLECT: u32 = 2u32;
pub const GENERATE_METHOD_PREFIX: u32 = 1u32;
pub const IFILTER_FLAGS_OLE_PROPERTIES: IFILTER_FLAGS = 1i32;
pub const IFILTER_INIT_APPLY_CRAWL_ATTRIBUTES: IFILTER_INIT = 256i32;
pub const IFILTER_INIT_APPLY_INDEX_ATTRIBUTES: IFILTER_INIT = 16i32;
pub const IFILTER_INIT_APPLY_OTHER_ATTRIBUTES: IFILTER_INIT = 32i32;
pub const IFILTER_INIT_CANON_HYPHENS: IFILTER_INIT = 4i32;
pub const IFILTER_INIT_CANON_PARAGRAPHS: IFILTER_INIT = 1i32;
pub const IFILTER_INIT_CANON_SPACES: IFILTER_INIT = 8i32;
pub const IFILTER_INIT_DISABLE_EMBEDDED: IFILTER_INIT = 2048i32;
pub const IFILTER_INIT_EMIT_FORMATTING: IFILTER_INIT = 4096i32;
pub const IFILTER_INIT_FILTER_AGGRESSIVE_BREAK: IFILTER_INIT = 1024i32;
pub const IFILTER_INIT_FILTER_OWNED_VALUE_OK: IFILTER_INIT = 512i32;
pub const IFILTER_INIT_HARD_LINE_BREAKS: IFILTER_INIT = 2i32;
pub const IFILTER_INIT_INDEXING_ONLY: IFILTER_INIT = 64i32;
pub const IFILTER_INIT_SEARCH_LINKS: IFILTER_INIT = 128i32;
pub const LIFF_FORCE_TEXT_FILTER_FALLBACK: u32 = 3u32;
pub const LIFF_IMPLEMENT_TEXT_FILTER_FALLBACK_POLICY: u32 = 2u32;
pub const LIFF_LOAD_DEFINED_FILTER: u32 = 1u32;
pub const MSIDXSPROP_COMMAND_LOCALE_STRING: u32 = 3u32;
pub const MSIDXSPROP_MAX_RANK: u32 = 6u32;
pub const MSIDXSPROP_PARSE_TREE: u32 = 5u32;
pub const MSIDXSPROP_QUERY_RESTRICTION: u32 = 4u32;
pub const MSIDXSPROP_RESULTS_FOUND: u32 = 7u32;
pub const MSIDXSPROP_ROWSETQUERYSTATUS: u32 = 2u32;
pub const MSIDXSPROP_SAME_SORTORDER_USED: u32 = 14u32;
pub const MSIDXSPROP_SERVER_NLSVERSION: u32 = 12u32;
pub const MSIDXSPROP_SERVER_NLSVER_DEFINED: u32 = 13u32;
pub const MSIDXSPROP_SERVER_VERSION: u32 = 9u32;
pub const MSIDXSPROP_SERVER_WINVER_MAJOR: u32 = 10u32;
pub const MSIDXSPROP_SERVER_WINVER_MINOR: u32 = 11u32;
pub const MSIDXSPROP_WHEREID: u32 = 8u32;
pub const NOT_AN_ERROR: ::windows_sys::core::HRESULT = 524288i32;
pub const PID_FILENAME: u32 = 100u32;
pub const PROPID_QUERY_ALL: u32 = 6u32;
pub const PROPID_QUERY_HITCOUNT: u32 = 4u32;
pub const PROPID_QUERY_LASTSEENTIME: u32 = 10u32;
pub const PROPID_QUERY_RANK: u32 = 3u32;
pub const PROPID_QUERY_RANKVECTOR: u32 = 2u32;
pub const PROPID_QUERY_UNFILTERED: u32 = 7u32;
pub const PROPID_QUERY_VIRTUALPATH: u32 = 9u32;
pub const PROPID_QUERY_WORKID: u32 = 5u32;
pub const PROPID_STG_CONTENTS: u32 = 19u32;
pub const PROXIMITY_UNIT_CHAPTER: u32 = 3u32;
pub const PROXIMITY_UNIT_PARAGRAPH: u32 = 2u32;
pub const PROXIMITY_UNIT_SENTENCE: u32 = 1u32;
pub const PROXIMITY_UNIT_WORD: u32 = 0u32;
pub const PSGUID_FILENAME: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x41cf5ae0_f75a_4806_bd87_59c7d9248eb9);
pub const QUERY_DEEP: u32 = 1u32;
pub const QUERY_PHYSICAL_PATH: u32 = 0u32;
pub const QUERY_SHALLOW: u32 = 0u32;
pub const QUERY_VIRTUAL_PATH: u32 = 2u32;
pub const SCOPE_FLAG_DEEP: u32 = 2u32;
pub const SCOPE_FLAG_INCLUDE: u32 = 1u32;
pub const SCOPE_FLAG_MASK: u32 = 255u32;
pub const SCOPE_TYPE_MASK: u32 = 4294967040u32;
pub const SCOPE_TYPE_VPATH: u32 = 512u32;
pub const SCOPE_TYPE_WINPATH: u32 = 256u32;
pub const STAT_BUSY: u32 = 0u32;
pub const STAT_COALESCE_COMP_ALL_NOISE: u32 = 8192u32;
pub const STAT_CONTENT_OUT_OF_DATE: u32 = 32u32;
pub const STAT_CONTENT_QUERY_INCOMPLETE: u32 = 128u32;
pub const STAT_DONE: u32 = 2u32;
pub const STAT_ERROR: u32 = 1u32;
pub const STAT_MISSING_PROP_IN_RELDOC: u32 = 2048u32;
pub const STAT_MISSING_RELDOC: u32 = 1024u32;
pub const STAT_NOISE_WORDS: u32 = 16u32;
pub const STAT_PARTIAL_SCOPE: u32 = 8u32;
pub const STAT_REFRESH: u32 = 3u32;
pub const STAT_REFRESH_INCOMPLETE: u32 = 64u32;
pub const STAT_RELDOC_ACCESS_DENIED: u32 = 4096u32;
pub const STAT_SHARING_VIOLATION: u32 = 512u32;
pub const STAT_TIME_LIMIT_EXCEEDED: u32 = 256u32;
pub const VECTOR_RANK_DICE: u32 = 3u32;
pub const VECTOR_RANK_INNER: u32 = 2u32;
pub const VECTOR_RANK_JACCARD: u32 = 4u32;
pub const VECTOR_RANK_MAX: u32 = 1u32;
pub const VECTOR_RANK_MIN: u32 = 0u32;
pub const WORDREP_BREAK_EOC: WORDREP_BREAK_TYPE = 3i32;
pub const WORDREP_BREAK_EOP: WORDREP_BREAK_TYPE = 2i32;
pub const WORDREP_BREAK_EOS: WORDREP_BREAK_TYPE = 1i32;
pub const WORDREP_BREAK_EOW: WORDREP_BREAK_TYPE = 0i32;
pub type CHUNKSTATE = i32;
pub type CHUNK_BREAKTYPE = i32;
pub type DBKINDENUM = i32;
pub type IFILTER_FLAGS = i32;
pub type IFILTER_INIT = i32;
pub type WORDREP_BREAK_TYPE = i32;
#[repr(C)]
pub struct CI_STATE {
    pub cbStruct: u32,
    pub cWordList: u32,
    pub cPersistentIndex: u32,
    pub cQueries: u32,
    pub cDocuments: u32,
    pub cFreshTest: u32,
    pub dwMergeProgress: u32,
    pub eState: u32,
    pub cFilteredDocuments: u32,
    pub cTotalDocuments: u32,
    pub cPendingScans: u32,
    pub dwIndexSize: u32,
    pub cUniqueKeys: u32,
    pub cSecQDocuments: u32,
    pub dwPropCacheSize: u32,
}
impl ::core::marker::Copy for CI_STATE {}
impl ::core::clone::Clone for CI_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub struct DBID {
    pub uGuid: DBID_0,
    pub eKind: u32,
    pub uName: DBID_1,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for DBID {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for DBID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub union DBID_0 {
    pub guid: ::windows_sys::core::GUID,
    pub pguid: *mut ::windows_sys::core::GUID,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for DBID_0 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for DBID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
pub union DBID_1 {
    pub pwszName: ::windows_sys::core::PWSTR,
    pub ulPropid: u32,
}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::marker::Copy for DBID_1 {}
#[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
impl ::core::clone::Clone for DBID_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
#[cfg(target_arch = "x86")]
pub struct DBID {
    pub uGuid: DBID_0,
    pub eKind: u32,
    pub uName: DBID_1,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for DBID {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for DBID {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
#[cfg(target_arch = "x86")]
pub union DBID_0 {
    pub guid: ::windows_sys::core::GUID,
    pub pguid: *mut ::windows_sys::core::GUID,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for DBID_0 {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for DBID_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C, packed(2))]
#[cfg(target_arch = "x86")]
pub union DBID_1 {
    pub pwszName: ::windows_sys::core::PWSTR,
    pub ulPropid: u32,
}
#[cfg(target_arch = "x86")]
impl ::core::marker::Copy for DBID_1 {}
#[cfg(target_arch = "x86")]
impl ::core::clone::Clone for DBID_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILTERREGION {
    pub idChunk: u32,
    pub cwcStart: u32,
    pub cwcExtent: u32,
}
impl ::core::marker::Copy for FILTERREGION {}
impl ::core::clone::Clone for FILTERREGION {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"]
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
pub struct FULLPROPSPEC {
    pub guidPropSet: ::windows_sys::core::GUID,
    pub psProperty: super::super::System::Com::StructuredStorage::PROPSPEC,
}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::marker::Copy for FULLPROPSPEC {}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::clone::Clone for FULLPROPSPEC {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_System_Com_StructuredStorage\"`"]
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
pub struct STAT_CHUNK {
    pub idChunk: u32,
    pub breakType: CHUNK_BREAKTYPE,
    pub flags: CHUNKSTATE,
    pub locale: u32,
    pub attribute: FULLPROPSPEC,
    pub idChunkSource: u32,
    pub cwcStartSource: u32,
    pub cwcLenSource: u32,
}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::marker::Copy for STAT_CHUNK {}
#[cfg(feature = "Win32_System_Com_StructuredStorage")]
impl ::core::clone::Clone for STAT_CHUNK {
    fn clone(&self) -> Self {
        *self
    }
}
