#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn AbortDoc(hdc : super::super::Graphics::Gdi:: HDC) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("winspool.drv" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn DeviceCapabilitiesA(pdevice : ::windows_sys::core::PCSTR, pport : ::windows_sys::core::PCSTR, fwcapability : PRINTER_DEVICE_CAPABILITIES, poutput : ::windows_sys::core::PSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEA) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("winspool.drv" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn DeviceCapabilitiesW(pdevice : ::windows_sys::core::PCWSTR, pport : ::windows_sys::core::PCWSTR, fwcapability : PRINTER_DEVICE_CAPABILITIES, poutput : ::windows_sys::core::PWSTR, pdevmode : *const super::super::Graphics::Gdi:: DEVMODEW) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn EndDoc(hdc : super::super::Graphics::Gdi:: HDC) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn EndPage(hdc : super::super::Graphics::Gdi:: HDC) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn Escape(hdc : super::super::Graphics::Gdi:: HDC, iescape : i32, cjin : i32, pvin : ::windows_sys::core::PCSTR, pvout : *mut ::core::ffi::c_void) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ExtEscape(hdc : super::super::Graphics::Gdi:: HDC, iescape : i32, cjinput : i32, lpindata : ::windows_sys::core::PCSTR, cjoutput : i32, lpoutdata : ::windows_sys::core::PSTR) -> i32);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("user32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn PrintWindow(hwnd : super::super::Foundation:: HWND, hdcblt : super::super::Graphics::Gdi:: HDC, nflags : PRINT_WINDOW_FLAGS) -> super::super::Foundation:: BOOL);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn SetAbortProc(hdc : super::super::Graphics::Gdi:: HDC, proc : ABORTPROC) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn StartDocA(hdc : super::super::Graphics::Gdi:: HDC, lpdi : *const DOCINFOA) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn StartDocW(hdc : super::super::Graphics::Gdi:: HDC, lpdi : *const DOCINFOW) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn StartPage(hdc : super::super::Graphics::Gdi:: HDC) -> i32);
pub type IXpsDocumentPackageTarget = *mut ::core::ffi::c_void;
pub type IXpsDocumentPackageTarget3D = *mut ::core::ffi::c_void;
pub type IXpsOMBrush = *mut ::core::ffi::c_void;
pub type IXpsOMCanvas = *mut ::core::ffi::c_void;
pub type IXpsOMColorProfileResource = *mut ::core::ffi::c_void;
pub type IXpsOMColorProfileResourceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMCoreProperties = *mut ::core::ffi::c_void;
pub type IXpsOMDashCollection = *mut ::core::ffi::c_void;
pub type IXpsOMDictionary = *mut ::core::ffi::c_void;
pub type IXpsOMDocument = *mut ::core::ffi::c_void;
pub type IXpsOMDocumentCollection = *mut ::core::ffi::c_void;
pub type IXpsOMDocumentSequence = *mut ::core::ffi::c_void;
pub type IXpsOMDocumentStructureResource = *mut ::core::ffi::c_void;
pub type IXpsOMFontResource = *mut ::core::ffi::c_void;
pub type IXpsOMFontResourceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMGeometry = *mut ::core::ffi::c_void;
pub type IXpsOMGeometryFigure = *mut ::core::ffi::c_void;
pub type IXpsOMGeometryFigureCollection = *mut ::core::ffi::c_void;
pub type IXpsOMGlyphs = *mut ::core::ffi::c_void;
pub type IXpsOMGlyphsEditor = *mut ::core::ffi::c_void;
pub type IXpsOMGradientBrush = *mut ::core::ffi::c_void;
pub type IXpsOMGradientStop = *mut ::core::ffi::c_void;
pub type IXpsOMGradientStopCollection = *mut ::core::ffi::c_void;
pub type IXpsOMImageBrush = *mut ::core::ffi::c_void;
pub type IXpsOMImageResource = *mut ::core::ffi::c_void;
pub type IXpsOMImageResourceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMLinearGradientBrush = *mut ::core::ffi::c_void;
pub type IXpsOMMatrixTransform = *mut ::core::ffi::c_void;
pub type IXpsOMNameCollection = *mut ::core::ffi::c_void;
pub type IXpsOMObjectFactory = *mut ::core::ffi::c_void;
pub type IXpsOMObjectFactory1 = *mut ::core::ffi::c_void;
pub type IXpsOMPackage = *mut ::core::ffi::c_void;
pub type IXpsOMPackage1 = *mut ::core::ffi::c_void;
pub type IXpsOMPackageTarget = *mut ::core::ffi::c_void;
pub type IXpsOMPackageWriter = *mut ::core::ffi::c_void;
pub type IXpsOMPackageWriter3D = *mut ::core::ffi::c_void;
pub type IXpsOMPage = *mut ::core::ffi::c_void;
pub type IXpsOMPage1 = *mut ::core::ffi::c_void;
pub type IXpsOMPageReference = *mut ::core::ffi::c_void;
pub type IXpsOMPageReferenceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMPart = *mut ::core::ffi::c_void;
pub type IXpsOMPartResources = *mut ::core::ffi::c_void;
pub type IXpsOMPartUriCollection = *mut ::core::ffi::c_void;
pub type IXpsOMPath = *mut ::core::ffi::c_void;
pub type IXpsOMPrintTicketResource = *mut ::core::ffi::c_void;
pub type IXpsOMRadialGradientBrush = *mut ::core::ffi::c_void;
pub type IXpsOMRemoteDictionaryResource = *mut ::core::ffi::c_void;
pub type IXpsOMRemoteDictionaryResource1 = *mut ::core::ffi::c_void;
pub type IXpsOMRemoteDictionaryResourceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMResource = *mut ::core::ffi::c_void;
pub type IXpsOMShareable = *mut ::core::ffi::c_void;
pub type IXpsOMSignatureBlockResource = *mut ::core::ffi::c_void;
pub type IXpsOMSignatureBlockResourceCollection = *mut ::core::ffi::c_void;
pub type IXpsOMSolidColorBrush = *mut ::core::ffi::c_void;
pub type IXpsOMStoryFragmentsResource = *mut ::core::ffi::c_void;
pub type IXpsOMThumbnailGenerator = *mut ::core::ffi::c_void;
pub type IXpsOMTileBrush = *mut ::core::ffi::c_void;
pub type IXpsOMVisual = *mut ::core::ffi::c_void;
pub type IXpsOMVisualBrush = *mut ::core::ffi::c_void;
pub type IXpsOMVisualCollection = *mut ::core::ffi::c_void;
pub type IXpsSignature = *mut ::core::ffi::c_void;
pub type IXpsSignatureBlock = *mut ::core::ffi::c_void;
pub type IXpsSignatureBlockCollection = *mut ::core::ffi::c_void;
pub type IXpsSignatureCollection = *mut ::core::ffi::c_void;
pub type IXpsSignatureManager = *mut ::core::ffi::c_void;
pub type IXpsSignatureRequest = *mut ::core::ffi::c_void;
pub type IXpsSignatureRequestCollection = *mut ::core::ffi::c_void;
pub type IXpsSigningOptions = *mut ::core::ffi::c_void;
pub const DC_BINNAMES: PRINTER_DEVICE_CAPABILITIES = 12u16;
pub const DC_BINS: PRINTER_DEVICE_CAPABILITIES = 6u16;
pub const DC_COLLATE: PRINTER_DEVICE_CAPABILITIES = 22u16;
pub const DC_COLORDEVICE: PRINTER_DEVICE_CAPABILITIES = 32u16;
pub const DC_COPIES: PRINTER_DEVICE_CAPABILITIES = 18u16;
pub const DC_DRIVER: PRINTER_DEVICE_CAPABILITIES = 11u16;
pub const DC_DUPLEX: PRINTER_DEVICE_CAPABILITIES = 7u16;
pub const DC_ENUMRESOLUTIONS: PRINTER_DEVICE_CAPABILITIES = 13u16;
pub const DC_EXTRA: PRINTER_DEVICE_CAPABILITIES = 9u16;
pub const DC_FIELDS: PRINTER_DEVICE_CAPABILITIES = 1u16;
pub const DC_FILEDEPENDENCIES: PRINTER_DEVICE_CAPABILITIES = 14u16;
pub const DC_MAXEXTENT: PRINTER_DEVICE_CAPABILITIES = 5u16;
pub const DC_MEDIAREADY: PRINTER_DEVICE_CAPABILITIES = 29u16;
pub const DC_MEDIATYPENAMES: PRINTER_DEVICE_CAPABILITIES = 34u16;
pub const DC_MEDIATYPES: PRINTER_DEVICE_CAPABILITIES = 35u16;
pub const DC_MINEXTENT: PRINTER_DEVICE_CAPABILITIES = 4u16;
pub const DC_NUP: PRINTER_DEVICE_CAPABILITIES = 33u16;
pub const DC_ORIENTATION: PRINTER_DEVICE_CAPABILITIES = 17u16;
pub const DC_PAPERNAMES: PRINTER_DEVICE_CAPABILITIES = 16u16;
pub const DC_PAPERS: PRINTER_DEVICE_CAPABILITIES = 2u16;
pub const DC_PAPERSIZE: PRINTER_DEVICE_CAPABILITIES = 3u16;
pub const DC_PERSONALITY: PRINTER_DEVICE_CAPABILITIES = 25u16;
pub const DC_PRINTERMEM: PRINTER_DEVICE_CAPABILITIES = 28u16;
pub const DC_PRINTRATE: PRINTER_DEVICE_CAPABILITIES = 26u16;
pub const DC_PRINTRATEPPM: PRINTER_DEVICE_CAPABILITIES = 31u16;
pub const DC_PRINTRATEUNIT: PRINTER_DEVICE_CAPABILITIES = 27u16;
pub const DC_SIZE: PRINTER_DEVICE_CAPABILITIES = 8u16;
pub const DC_STAPLE: PRINTER_DEVICE_CAPABILITIES = 30u16;
pub const DC_TRUETYPE: PRINTER_DEVICE_CAPABILITIES = 15u16;
pub const DC_VERSION: PRINTER_DEVICE_CAPABILITIES = 10u16;
pub const PSINJECT_BEGINDEFAULTS: PSINJECT_POINT = 12u16;
pub const PSINJECT_BEGINPAGESETUP: PSINJECT_POINT = 101u16;
pub const PSINJECT_BEGINPROLOG: PSINJECT_POINT = 14u16;
pub const PSINJECT_BEGINSETUP: PSINJECT_POINT = 16u16;
pub const PSINJECT_BEGINSTREAM: PSINJECT_POINT = 1u16;
pub const PSINJECT_BOUNDINGBOX: PSINJECT_POINT = 9u16;
pub const PSINJECT_COMMENTS: PSINJECT_POINT = 11u16;
pub const PSINJECT_DOCNEEDEDRES: PSINJECT_POINT = 5u16;
pub const PSINJECT_DOCSUPPLIEDRES: PSINJECT_POINT = 6u16;
pub const PSINJECT_DOCUMENTPROCESSCOLORS: PSINJECT_POINT = 10u16;
pub const PSINJECT_DOCUMENTPROCESSCOLORSATEND: PSINJECT_POINT = 21u16;
pub const PSINJECT_ENDDEFAULTS: PSINJECT_POINT = 13u16;
pub const PSINJECT_ENDPAGECOMMENTS: PSINJECT_POINT = 107u16;
pub const PSINJECT_ENDPAGESETUP: PSINJECT_POINT = 102u16;
pub const PSINJECT_ENDPROLOG: PSINJECT_POINT = 15u16;
pub const PSINJECT_ENDSETUP: PSINJECT_POINT = 17u16;
pub const PSINJECT_ENDSTREAM: PSINJECT_POINT = 20u16;
pub const PSINJECT_EOF: PSINJECT_POINT = 19u16;
pub const PSINJECT_ORIENTATION: PSINJECT_POINT = 8u16;
pub const PSINJECT_PAGEBBOX: PSINJECT_POINT = 106u16;
pub const PSINJECT_PAGENUMBER: PSINJECT_POINT = 100u16;
pub const PSINJECT_PAGEORDER: PSINJECT_POINT = 7u16;
pub const PSINJECT_PAGES: PSINJECT_POINT = 4u16;
pub const PSINJECT_PAGESATEND: PSINJECT_POINT = 3u16;
pub const PSINJECT_PAGETRAILER: PSINJECT_POINT = 103u16;
pub const PSINJECT_PLATECOLOR: PSINJECT_POINT = 104u16;
pub const PSINJECT_PSADOBE: PSINJECT_POINT = 2u16;
pub const PSINJECT_SHOWPAGE: PSINJECT_POINT = 105u16;
pub const PSINJECT_TRAILER: PSINJECT_POINT = 18u16;
pub const PSINJECT_VMRESTORE: PSINJECT_POINT = 201u16;
pub const PSINJECT_VMSAVE: PSINJECT_POINT = 200u16;
pub const PW_CLIENTONLY: PRINT_WINDOW_FLAGS = 1u32;
pub const XPS_COLOR_INTERPOLATION_SCRGBLINEAR: XPS_COLOR_INTERPOLATION = 1i32;
pub const XPS_COLOR_INTERPOLATION_SRGBLINEAR: XPS_COLOR_INTERPOLATION = 2i32;
pub const XPS_COLOR_TYPE_CONTEXT: XPS_COLOR_TYPE = 3i32;
pub const XPS_COLOR_TYPE_SCRGB: XPS_COLOR_TYPE = 2i32;
pub const XPS_COLOR_TYPE_SRGB: XPS_COLOR_TYPE = 1i32;
pub const XPS_DASH_CAP_FLAT: XPS_DASH_CAP = 1i32;
pub const XPS_DASH_CAP_ROUND: XPS_DASH_CAP = 2i32;
pub const XPS_DASH_CAP_SQUARE: XPS_DASH_CAP = 3i32;
pub const XPS_DASH_CAP_TRIANGLE: XPS_DASH_CAP = 4i32;
pub const XPS_DOCUMENT_TYPE_OPENXPS: XPS_DOCUMENT_TYPE = 3i32;
pub const XPS_DOCUMENT_TYPE_UNSPECIFIED: XPS_DOCUMENT_TYPE = 1i32;
pub const XPS_DOCUMENT_TYPE_XPS: XPS_DOCUMENT_TYPE = 2i32;
pub const XPS_E_ABSOLUTE_REFERENCE: ::windows_sys::core::HRESULT = -2142108159i32;
pub const XPS_E_ALREADY_OWNED: ::windows_sys::core::HRESULT = -2142108413i32;
pub const XPS_E_BLEED_BOX_PAGE_DIMENSIONS_NOT_IN_SYNC: ::windows_sys::core::HRESULT = -2142108407i32;
pub const XPS_E_BOTH_PATHFIGURE_AND_ABBR_SYNTAX_PRESENT: ::windows_sys::core::HRESULT = -2142108409i32;
pub const XPS_E_BOTH_RESOURCE_AND_SOURCEATTR_PRESENT: ::windows_sys::core::HRESULT = -2142108408i32;
pub const XPS_E_CARET_OUTSIDE_STRING: ::windows_sys::core::HRESULT = -2142108923i32;
pub const XPS_E_CARET_OUT_OF_ORDER: ::windows_sys::core::HRESULT = -2142108922i32;
pub const XPS_E_COLOR_COMPONENT_OUT_OF_RANGE: ::windows_sys::core::HRESULT = -2142108410i32;
pub const XPS_E_DICTIONARY_ITEM_NAMED: ::windows_sys::core::HRESULT = -2142108671i32;
pub const XPS_E_DUPLICATE_NAMES: ::windows_sys::core::HRESULT = -2142109175i32;
pub const XPS_E_DUPLICATE_RESOURCE_KEYS: ::windows_sys::core::HRESULT = -2142109184i32;
pub const XPS_E_INDEX_OUT_OF_RANGE: ::windows_sys::core::HRESULT = -2142108416i32;
pub const XPS_E_INVALID_BLEED_BOX: ::windows_sys::core::HRESULT = -2142109692i32;
pub const XPS_E_INVALID_CONTENT_BOX: ::windows_sys::core::HRESULT = -2142109685i32;
pub const XPS_E_INVALID_CONTENT_TYPE: ::windows_sys::core::HRESULT = -2142109682i32;
pub const XPS_E_INVALID_FLOAT: ::windows_sys::core::HRESULT = -2142109689i32;
pub const XPS_E_INVALID_FONT_URI: ::windows_sys::core::HRESULT = -2142109686i32;
pub const XPS_E_INVALID_LANGUAGE: ::windows_sys::core::HRESULT = -2142109696i32;
pub const XPS_E_INVALID_LOOKUP_TYPE: ::windows_sys::core::HRESULT = -2142109690i32;
pub const XPS_E_INVALID_MARKUP: ::windows_sys::core::HRESULT = -2142109684i32;
pub const XPS_E_INVALID_NAME: ::windows_sys::core::HRESULT = -2142109695i32;
pub const XPS_E_INVALID_NUMBER_OF_COLOR_CHANNELS: ::windows_sys::core::HRESULT = -2142108158i32;
pub const XPS_E_INVALID_NUMBER_OF_POINTS_IN_CURVE_SEGMENTS: ::windows_sys::core::HRESULT = -2142108160i32;
pub const XPS_E_INVALID_OBFUSCATED_FONT_URI: ::windows_sys::core::HRESULT = -2142109681i32;
pub const XPS_E_INVALID_PAGE_SIZE: ::windows_sys::core::HRESULT = -2142109693i32;
pub const XPS_E_INVALID_RESOURCE_KEY: ::windows_sys::core::HRESULT = -2142109694i32;
pub const XPS_E_INVALID_SIGNATUREBLOCK_MARKUP: ::windows_sys::core::HRESULT = -2142108789i32;
pub const XPS_E_INVALID_THUMBNAIL_IMAGE_TYPE: ::windows_sys::core::HRESULT = -2142109691i32;
pub const XPS_E_INVALID_XML_ENCODING: ::windows_sys::core::HRESULT = -2142109683i32;
pub const XPS_E_MAPPING_OUTSIDE_INDICES: ::windows_sys::core::HRESULT = -2142108924i32;
pub const XPS_E_MAPPING_OUTSIDE_STRING: ::windows_sys::core::HRESULT = -2142108925i32;
pub const XPS_E_MAPPING_OUT_OF_ORDER: ::windows_sys::core::HRESULT = -2142108926i32;
pub const XPS_E_MARKUP_COMPATIBILITY_ELEMENTS: ::windows_sys::core::HRESULT = -2142108791i32;
pub const XPS_E_MISSING_COLORPROFILE: ::windows_sys::core::HRESULT = -2142109436i32;
pub const XPS_E_MISSING_DISCARDCONTROL: ::windows_sys::core::HRESULT = -2142109422i32;
pub const XPS_E_MISSING_DOCUMENT: ::windows_sys::core::HRESULT = -2142109431i32;
pub const XPS_E_MISSING_DOCUMENTSEQUENCE_RELATIONSHIP: ::windows_sys::core::HRESULT = -2142109432i32;
pub const XPS_E_MISSING_FONTURI: ::windows_sys::core::HRESULT = -2142109433i32;
pub const XPS_E_MISSING_GLYPHS: ::windows_sys::core::HRESULT = -2142109438i32;
pub const XPS_E_MISSING_IMAGE_IN_IMAGEBRUSH: ::windows_sys::core::HRESULT = -2142109426i32;
pub const XPS_E_MISSING_LOOKUP: ::windows_sys::core::HRESULT = -2142109439i32;
pub const XPS_E_MISSING_NAME: ::windows_sys::core::HRESULT = -2142109440i32;
pub const XPS_E_MISSING_PAGE_IN_DOCUMENT: ::windows_sys::core::HRESULT = -2142109428i32;
pub const XPS_E_MISSING_PAGE_IN_PAGEREFERENCE: ::windows_sys::core::HRESULT = -2142109427i32;
pub const XPS_E_MISSING_PART_REFERENCE: ::windows_sys::core::HRESULT = -2142109424i32;
pub const XPS_E_MISSING_PART_STREAM: ::windows_sys::core::HRESULT = -2142109421i32;
pub const XPS_E_MISSING_REFERRED_DOCUMENT: ::windows_sys::core::HRESULT = -2142109430i32;
pub const XPS_E_MISSING_REFERRED_PAGE: ::windows_sys::core::HRESULT = -2142109429i32;
pub const XPS_E_MISSING_RELATIONSHIP_TARGET: ::windows_sys::core::HRESULT = -2142109435i32;
pub const XPS_E_MISSING_RESOURCE_KEY: ::windows_sys::core::HRESULT = -2142109425i32;
pub const XPS_E_MISSING_RESOURCE_RELATIONSHIP: ::windows_sys::core::HRESULT = -2142109434i32;
pub const XPS_E_MISSING_RESTRICTED_FONT_RELATIONSHIP: ::windows_sys::core::HRESULT = -2142109423i32;
pub const XPS_E_MISSING_SEGMENT_DATA: ::windows_sys::core::HRESULT = -2142109437i32;
pub const XPS_E_MULTIPLE_DOCUMENTSEQUENCE_RELATIONSHIPS: ::windows_sys::core::HRESULT = -2142109182i32;
pub const XPS_E_MULTIPLE_PRINTTICKETS_ON_DOCUMENT: ::windows_sys::core::HRESULT = -2142109178i32;
pub const XPS_E_MULTIPLE_PRINTTICKETS_ON_DOCUMENTSEQUENCE: ::windows_sys::core::HRESULT = -2142109177i32;
pub const XPS_E_MULTIPLE_PRINTTICKETS_ON_PAGE: ::windows_sys::core::HRESULT = -2142109179i32;
pub const XPS_E_MULTIPLE_REFERENCES_TO_PART: ::windows_sys::core::HRESULT = -2142109176i32;
pub const XPS_E_MULTIPLE_RESOURCES: ::windows_sys::core::HRESULT = -2142109183i32;
pub const XPS_E_MULTIPLE_THUMBNAILS_ON_PACKAGE: ::windows_sys::core::HRESULT = -2142109180i32;
pub const XPS_E_MULTIPLE_THUMBNAILS_ON_PAGE: ::windows_sys::core::HRESULT = -2142109181i32;
pub const XPS_E_NEGATIVE_FLOAT: ::windows_sys::core::HRESULT = -2142108918i32;
pub const XPS_E_NESTED_REMOTE_DICTIONARY: ::windows_sys::core::HRESULT = -2142108670i32;
pub const XPS_E_NOT_ENOUGH_GRADIENT_STOPS: ::windows_sys::core::HRESULT = -2142108405i32;
pub const XPS_E_NO_CUSTOM_OBJECTS: ::windows_sys::core::HRESULT = -2142108414i32;
pub const XPS_E_OBJECT_DETACHED: ::windows_sys::core::HRESULT = -2142108790i32;
pub const XPS_E_ODD_BIDILEVEL: ::windows_sys::core::HRESULT = -2142108921i32;
pub const XPS_E_ONE_TO_ONE_MAPPING_EXPECTED: ::windows_sys::core::HRESULT = -2142108920i32;
pub const XPS_E_PACKAGE_ALREADY_OPENED: ::windows_sys::core::HRESULT = -2142108793i32;
pub const XPS_E_PACKAGE_NOT_OPENED: ::windows_sys::core::HRESULT = -2142108794i32;
pub const XPS_E_PACKAGE_WRITER_NOT_CLOSED: ::windows_sys::core::HRESULT = -2142108404i32;
pub const XPS_E_RELATIONSHIP_EXTERNAL: ::windows_sys::core::HRESULT = -2142108406i32;
pub const XPS_E_RESOURCE_NOT_OWNED: ::windows_sys::core::HRESULT = -2142108412i32;
pub const XPS_E_RESTRICTED_FONT_NOT_OBFUSCATED: ::windows_sys::core::HRESULT = -2142108919i32;
pub const XPS_E_SIGNATUREID_DUP: ::windows_sys::core::HRESULT = -2142108792i32;
pub const XPS_E_SIGREQUESTID_DUP: ::windows_sys::core::HRESULT = -2142108795i32;
pub const XPS_E_STRING_TOO_LONG: ::windows_sys::core::HRESULT = -2142108928i32;
pub const XPS_E_TOO_MANY_INDICES: ::windows_sys::core::HRESULT = -2142108927i32;
pub const XPS_E_UNAVAILABLE_PACKAGE: ::windows_sys::core::HRESULT = -2142109420i32;
pub const XPS_E_UNEXPECTED_COLORPROFILE: ::windows_sys::core::HRESULT = -2142108411i32;
pub const XPS_E_UNEXPECTED_CONTENT_TYPE: ::windows_sys::core::HRESULT = -2142109688i32;
pub const XPS_E_UNEXPECTED_RELATIONSHIP_TYPE: ::windows_sys::core::HRESULT = -2142109680i32;
pub const XPS_E_UNEXPECTED_RESTRICTED_FONT_RELATIONSHIP: ::windows_sys::core::HRESULT = -2142109679i32;
pub const XPS_E_VISUAL_CIRCULAR_REF: ::windows_sys::core::HRESULT = -2142108415i32;
pub const XPS_E_XKEY_ATTR_PRESENT_OUTSIDE_RES_DICT: ::windows_sys::core::HRESULT = -2142108672i32;
pub const XPS_FILL_RULE_EVENODD: XPS_FILL_RULE = 1i32;
pub const XPS_FILL_RULE_NONZERO: XPS_FILL_RULE = 2i32;
pub const XPS_FONT_EMBEDDING_NORMAL: XPS_FONT_EMBEDDING = 1i32;
pub const XPS_FONT_EMBEDDING_OBFUSCATED: XPS_FONT_EMBEDDING = 2i32;
pub const XPS_FONT_EMBEDDING_RESTRICTED: XPS_FONT_EMBEDDING = 3i32;
pub const XPS_FONT_EMBEDDING_RESTRICTED_UNOBFUSCATED: XPS_FONT_EMBEDDING = 4i32;
pub const XPS_IMAGE_TYPE_JPEG: XPS_IMAGE_TYPE = 1i32;
pub const XPS_IMAGE_TYPE_JXR: XPS_IMAGE_TYPE = 5i32;
pub const XPS_IMAGE_TYPE_PNG: XPS_IMAGE_TYPE = 2i32;
pub const XPS_IMAGE_TYPE_TIFF: XPS_IMAGE_TYPE = 3i32;
pub const XPS_IMAGE_TYPE_WDP: XPS_IMAGE_TYPE = 4i32;
pub const XPS_INTERLEAVING_OFF: XPS_INTERLEAVING = 1i32;
pub const XPS_INTERLEAVING_ON: XPS_INTERLEAVING = 2i32;
pub const XPS_LINE_CAP_FLAT: XPS_LINE_CAP = 1i32;
pub const XPS_LINE_CAP_ROUND: XPS_LINE_CAP = 2i32;
pub const XPS_LINE_CAP_SQUARE: XPS_LINE_CAP = 3i32;
pub const XPS_LINE_CAP_TRIANGLE: XPS_LINE_CAP = 4i32;
pub const XPS_LINE_JOIN_BEVEL: XPS_LINE_JOIN = 2i32;
pub const XPS_LINE_JOIN_MITER: XPS_LINE_JOIN = 1i32;
pub const XPS_LINE_JOIN_ROUND: XPS_LINE_JOIN = 3i32;
pub const XPS_OBJECT_TYPE_CANVAS: XPS_OBJECT_TYPE = 1i32;
pub const XPS_OBJECT_TYPE_GEOMETRY: XPS_OBJECT_TYPE = 5i32;
pub const XPS_OBJECT_TYPE_GLYPHS: XPS_OBJECT_TYPE = 2i32;
pub const XPS_OBJECT_TYPE_IMAGE_BRUSH: XPS_OBJECT_TYPE = 7i32;
pub const XPS_OBJECT_TYPE_LINEAR_GRADIENT_BRUSH: XPS_OBJECT_TYPE = 8i32;
pub const XPS_OBJECT_TYPE_MATRIX_TRANSFORM: XPS_OBJECT_TYPE = 4i32;
pub const XPS_OBJECT_TYPE_PATH: XPS_OBJECT_TYPE = 3i32;
pub const XPS_OBJECT_TYPE_RADIAL_GRADIENT_BRUSH: XPS_OBJECT_TYPE = 9i32;
pub const XPS_OBJECT_TYPE_SOLID_COLOR_BRUSH: XPS_OBJECT_TYPE = 6i32;
pub const XPS_OBJECT_TYPE_VISUAL_BRUSH: XPS_OBJECT_TYPE = 10i32;
pub const XPS_SEGMENT_STROKE_PATTERN_ALL: XPS_SEGMENT_STROKE_PATTERN = 1i32;
pub const XPS_SEGMENT_STROKE_PATTERN_MIXED: XPS_SEGMENT_STROKE_PATTERN = 3i32;
pub const XPS_SEGMENT_STROKE_PATTERN_NONE: XPS_SEGMENT_STROKE_PATTERN = 2i32;
pub const XPS_SEGMENT_TYPE_ARC_LARGE_CLOCKWISE: XPS_SEGMENT_TYPE = 1i32;
pub const XPS_SEGMENT_TYPE_ARC_LARGE_COUNTERCLOCKWISE: XPS_SEGMENT_TYPE = 2i32;
pub const XPS_SEGMENT_TYPE_ARC_SMALL_CLOCKWISE: XPS_SEGMENT_TYPE = 3i32;
pub const XPS_SEGMENT_TYPE_ARC_SMALL_COUNTERCLOCKWISE: XPS_SEGMENT_TYPE = 4i32;
pub const XPS_SEGMENT_TYPE_BEZIER: XPS_SEGMENT_TYPE = 5i32;
pub const XPS_SEGMENT_TYPE_LINE: XPS_SEGMENT_TYPE = 6i32;
pub const XPS_SEGMENT_TYPE_QUADRATIC_BEZIER: XPS_SEGMENT_TYPE = 7i32;
pub const XPS_SIGNATURE_STATUS_BROKEN: XPS_SIGNATURE_STATUS = 3i32;
pub const XPS_SIGNATURE_STATUS_INCOMPLETE: XPS_SIGNATURE_STATUS = 2i32;
pub const XPS_SIGNATURE_STATUS_INCOMPLIANT: XPS_SIGNATURE_STATUS = 1i32;
pub const XPS_SIGNATURE_STATUS_QUESTIONABLE: XPS_SIGNATURE_STATUS = 4i32;
pub const XPS_SIGNATURE_STATUS_VALID: XPS_SIGNATURE_STATUS = 5i32;
pub const XPS_SIGN_FLAGS_IGNORE_MARKUP_COMPATIBILITY: XPS_SIGN_FLAGS = 1i32;
pub const XPS_SIGN_FLAGS_NONE: XPS_SIGN_FLAGS = 0i32;
pub const XPS_SIGN_POLICY_ALL: XPS_SIGN_POLICY = 15i32;
pub const XPS_SIGN_POLICY_CORE_PROPERTIES: XPS_SIGN_POLICY = 1i32;
pub const XPS_SIGN_POLICY_DISCARD_CONTROL: XPS_SIGN_POLICY = 8i32;
pub const XPS_SIGN_POLICY_NONE: XPS_SIGN_POLICY = 0i32;
pub const XPS_SIGN_POLICY_PRINT_TICKET: XPS_SIGN_POLICY = 4i32;
pub const XPS_SIGN_POLICY_SIGNATURE_RELATIONSHIPS: XPS_SIGN_POLICY = 2i32;
pub const XPS_SPREAD_METHOD_PAD: XPS_SPREAD_METHOD = 1i32;
pub const XPS_SPREAD_METHOD_REFLECT: XPS_SPREAD_METHOD = 2i32;
pub const XPS_SPREAD_METHOD_REPEAT: XPS_SPREAD_METHOD = 3i32;
pub const XPS_STYLE_SIMULATION_BOLD: XPS_STYLE_SIMULATION = 3i32;
pub const XPS_STYLE_SIMULATION_BOLDITALIC: XPS_STYLE_SIMULATION = 4i32;
pub const XPS_STYLE_SIMULATION_ITALIC: XPS_STYLE_SIMULATION = 2i32;
pub const XPS_STYLE_SIMULATION_NONE: XPS_STYLE_SIMULATION = 1i32;
pub const XPS_THUMBNAIL_SIZE_LARGE: XPS_THUMBNAIL_SIZE = 4i32;
pub const XPS_THUMBNAIL_SIZE_MEDIUM: XPS_THUMBNAIL_SIZE = 3i32;
pub const XPS_THUMBNAIL_SIZE_SMALL: XPS_THUMBNAIL_SIZE = 2i32;
pub const XPS_THUMBNAIL_SIZE_VERYSMALL: XPS_THUMBNAIL_SIZE = 1i32;
pub const XPS_TILE_MODE_FLIPX: XPS_TILE_MODE = 3i32;
pub const XPS_TILE_MODE_FLIPXY: XPS_TILE_MODE = 5i32;
pub const XPS_TILE_MODE_FLIPY: XPS_TILE_MODE = 4i32;
pub const XPS_TILE_MODE_NONE: XPS_TILE_MODE = 1i32;
pub const XPS_TILE_MODE_TILE: XPS_TILE_MODE = 2i32;
pub const XpsOMObjectFactory: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xe974d26d_3d9b_4d47_88cc_3872f2dc3585);
pub const XpsOMThumbnailGenerator: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7e4a23e2_b969_4761_be35_1a8ced58e323);
pub const XpsSignatureManager: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xb0c43320_2315_44a2_b70a_0943a140a8ee);
pub type PRINTER_DEVICE_CAPABILITIES = u16;
pub type PRINT_WINDOW_FLAGS = u32;
pub type PSINJECT_POINT = u16;
pub type XPS_COLOR_INTERPOLATION = i32;
pub type XPS_COLOR_TYPE = i32;
pub type XPS_DASH_CAP = i32;
pub type XPS_DOCUMENT_TYPE = i32;
pub type XPS_FILL_RULE = i32;
pub type XPS_FONT_EMBEDDING = i32;
pub type XPS_IMAGE_TYPE = i32;
pub type XPS_INTERLEAVING = i32;
pub type XPS_LINE_CAP = i32;
pub type XPS_LINE_JOIN = i32;
pub type XPS_OBJECT_TYPE = i32;
pub type XPS_SEGMENT_STROKE_PATTERN = i32;
pub type XPS_SEGMENT_TYPE = i32;
pub type XPS_SIGNATURE_STATUS = i32;
pub type XPS_SIGN_FLAGS = i32;
pub type XPS_SIGN_POLICY = i32;
pub type XPS_SPREAD_METHOD = i32;
pub type XPS_STYLE_SIMULATION = i32;
pub type XPS_THUMBNAIL_SIZE = i32;
pub type XPS_TILE_MODE = i32;
#[repr(C)]
pub struct DOCINFOA {
    pub cbSize: i32,
    pub lpszDocName: ::windows_sys::core::PCSTR,
    pub lpszOutput: ::windows_sys::core::PCSTR,
    pub lpszDatatype: ::windows_sys::core::PCSTR,
    pub fwType: u32,
}
impl ::core::marker::Copy for DOCINFOA {}
impl ::core::clone::Clone for DOCINFOA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DOCINFOW {
    pub cbSize: i32,
    pub lpszDocName: ::windows_sys::core::PCWSTR,
    pub lpszOutput: ::windows_sys::core::PCWSTR,
    pub lpszDatatype: ::windows_sys::core::PCWSTR,
    pub fwType: u32,
}
impl ::core::marker::Copy for DOCINFOW {}
impl ::core::clone::Clone for DOCINFOW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct DRAWPATRECT {
    pub ptPosition: super::super::Foundation::POINT,
    pub ptSize: super::super::Foundation::POINT,
    pub wStyle: u16,
    pub wPattern: u16,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for DRAWPATRECT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for DRAWPATRECT {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HPTPROVIDER = isize;
#[repr(C)]
pub struct PSFEATURE_CUSTPAPER {
    pub lOrientation: i32,
    pub lWidth: i32,
    pub lHeight: i32,
    pub lWidthOffset: i32,
    pub lHeightOffset: i32,
}
impl ::core::marker::Copy for PSFEATURE_CUSTPAPER {}
impl ::core::clone::Clone for PSFEATURE_CUSTPAPER {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub struct PSFEATURE_OUTPUT {
    pub bPageIndependent: super::super::Foundation::BOOL,
    pub bSetPageDevice: super::super::Foundation::BOOL,
}
#[cfg(feature = "Win32_Foundation")]
impl ::core::marker::Copy for PSFEATURE_OUTPUT {}
#[cfg(feature = "Win32_Foundation")]
impl ::core::clone::Clone for PSFEATURE_OUTPUT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct PSINJECTDATA {
    pub DataBytes: u32,
    pub InjectionPoint: PSINJECT_POINT,
    pub PageNumber: u16,
}
impl ::core::marker::Copy for PSINJECTDATA {}
impl ::core::clone::Clone for PSINJECTDATA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_COLOR {
    pub colorType: XPS_COLOR_TYPE,
    pub value: XPS_COLOR_0,
}
impl ::core::marker::Copy for XPS_COLOR {}
impl ::core::clone::Clone for XPS_COLOR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union XPS_COLOR_0 {
    pub sRGB: XPS_COLOR_0_1,
    pub scRGB: XPS_COLOR_0_2,
    pub context: XPS_COLOR_0_0,
}
impl ::core::marker::Copy for XPS_COLOR_0 {}
impl ::core::clone::Clone for XPS_COLOR_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_COLOR_0_0 {
    pub channelCount: u8,
    pub channels: [f32; 9],
}
impl ::core::marker::Copy for XPS_COLOR_0_0 {}
impl ::core::clone::Clone for XPS_COLOR_0_0 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_COLOR_0_1 {
    pub alpha: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
impl ::core::marker::Copy for XPS_COLOR_0_1 {}
impl ::core::clone::Clone for XPS_COLOR_0_1 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_COLOR_0_2 {
    pub alpha: f32,
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}
impl ::core::marker::Copy for XPS_COLOR_0_2 {}
impl ::core::clone::Clone for XPS_COLOR_0_2 {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_DASH {
    pub length: f32,
    pub gap: f32,
}
impl ::core::marker::Copy for XPS_DASH {}
impl ::core::clone::Clone for XPS_DASH {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_GLYPH_INDEX {
    pub index: i32,
    pub advanceWidth: f32,
    pub horizontalOffset: f32,
    pub verticalOffset: f32,
}
impl ::core::marker::Copy for XPS_GLYPH_INDEX {}
impl ::core::clone::Clone for XPS_GLYPH_INDEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_GLYPH_MAPPING {
    pub unicodeStringStart: u32,
    pub unicodeStringLength: u16,
    pub glyphIndicesStart: u32,
    pub glyphIndicesLength: u16,
}
impl ::core::marker::Copy for XPS_GLYPH_MAPPING {}
impl ::core::clone::Clone for XPS_GLYPH_MAPPING {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_MATRIX {
    pub m11: f32,
    pub m12: f32,
    pub m21: f32,
    pub m22: f32,
    pub m31: f32,
    pub m32: f32,
}
impl ::core::marker::Copy for XPS_MATRIX {}
impl ::core::clone::Clone for XPS_MATRIX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_POINT {
    pub x: f32,
    pub y: f32,
}
impl ::core::marker::Copy for XPS_POINT {}
impl ::core::clone::Clone for XPS_POINT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_RECT {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}
impl ::core::marker::Copy for XPS_RECT {}
impl ::core::clone::Clone for XPS_RECT {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct XPS_SIZE {
    pub width: f32,
    pub height: f32,
}
impl ::core::marker::Copy for XPS_SIZE {}
impl ::core::clone::Clone for XPS_SIZE {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"]
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
pub type ABORTPROC = ::core::option::Option<unsafe extern "system" fn(param0: super::super::Graphics::Gdi::HDC, param1: i32) -> super::super::Foundation::BOOL>;
