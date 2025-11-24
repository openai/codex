::windows_targets::link!("kernel32.dll" "system" fn CompareStringA(locale : u32, dwcmpflags : u32, lpstring1 : *const i8, cchcount1 : i32, lpstring2 : *const i8, cchcount2 : i32) -> COMPARESTRING_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CompareStringEx(lplocalename : ::windows_sys::core::PCWSTR, dwcmpflags : COMPARE_STRING_FLAGS, lpstring1 : ::windows_sys::core::PCWSTR, cchcount1 : i32, lpstring2 : ::windows_sys::core::PCWSTR, cchcount2 : i32, lpversioninformation : *const NLSVERSIONINFO, lpreserved : *const ::core::ffi::c_void, lparam : super::Foundation:: LPARAM) -> COMPARESTRING_RESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn CompareStringOrdinal(lpstring1 : ::windows_sys::core::PCWSTR, cchcount1 : i32, lpstring2 : ::windows_sys::core::PCWSTR, cchcount2 : i32, bignorecase : super::Foundation:: BOOL) -> COMPARESTRING_RESULT);
::windows_targets::link!("kernel32.dll" "system" fn CompareStringW(locale : u32, dwcmpflags : u32, lpstring1 : ::windows_sys::core::PCWSTR, cchcount1 : i32, lpstring2 : ::windows_sys::core::PCWSTR, cchcount2 : i32) -> COMPARESTRING_RESULT);
::windows_targets::link!("kernel32.dll" "system" fn ConvertDefaultLocale(locale : u32) -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumCalendarInfoA(lpcalinfoenumproc : CALINFO_ENUMPROCA, locale : u32, calendar : u32, caltype : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumCalendarInfoExA(lpcalinfoenumprocex : CALINFO_ENUMPROCEXA, locale : u32, calendar : u32, caltype : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumCalendarInfoExEx(pcalinfoenumprocexex : CALINFO_ENUMPROCEXEX, lplocalename : ::windows_sys::core::PCWSTR, calendar : u32, lpreserved : ::windows_sys::core::PCWSTR, caltype : u32, lparam : super::Foundation:: LPARAM) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumCalendarInfoExW(lpcalinfoenumprocex : CALINFO_ENUMPROCEXW, locale : u32, calendar : u32, caltype : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumCalendarInfoW(lpcalinfoenumproc : CALINFO_ENUMPROCW, locale : u32, calendar : u32, caltype : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDateFormatsA(lpdatefmtenumproc : DATEFMT_ENUMPROCA, locale : u32, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDateFormatsExA(lpdatefmtenumprocex : DATEFMT_ENUMPROCEXA, locale : u32, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDateFormatsExEx(lpdatefmtenumprocexex : DATEFMT_ENUMPROCEXEX, lplocalename : ::windows_sys::core::PCWSTR, dwflags : ENUM_DATE_FORMATS_FLAGS, lparam : super::Foundation:: LPARAM) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDateFormatsExW(lpdatefmtenumprocex : DATEFMT_ENUMPROCEXW, locale : u32, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumDateFormatsW(lpdatefmtenumproc : DATEFMT_ENUMPROCW, locale : u32, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumLanguageGroupLocalesA(lplanggrouplocaleenumproc : LANGGROUPLOCALE_ENUMPROCA, languagegroup : u32, dwflags : u32, lparam : isize) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumLanguageGroupLocalesW(lplanggrouplocaleenumproc : LANGGROUPLOCALE_ENUMPROCW, languagegroup : u32, dwflags : u32, lparam : isize) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemCodePagesA(lpcodepageenumproc : CODEPAGE_ENUMPROCA, dwflags : ENUM_SYSTEM_CODE_PAGES_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemCodePagesW(lpcodepageenumproc : CODEPAGE_ENUMPROCW, dwflags : ENUM_SYSTEM_CODE_PAGES_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemGeoID(geoclass : u32, parentgeoid : i32, lpgeoenumproc : GEO_ENUMPROC) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemGeoNames(geoclass : u32, geoenumproc : GEO_ENUMNAMEPROC, data : super::Foundation:: LPARAM) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemLanguageGroupsA(lplanguagegroupenumproc : LANGUAGEGROUP_ENUMPROCA, dwflags : ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS, lparam : isize) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemLanguageGroupsW(lplanguagegroupenumproc : LANGUAGEGROUP_ENUMPROCW, dwflags : ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS, lparam : isize) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemLocalesA(lplocaleenumproc : LOCALE_ENUMPROCA, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemLocalesEx(lplocaleenumprocex : LOCALE_ENUMPROCEX, dwflags : u32, lparam : super::Foundation:: LPARAM, lpreserved : *const ::core::ffi::c_void) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumSystemLocalesW(lplocaleenumproc : LOCALE_ENUMPROCW, dwflags : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumTimeFormatsA(lptimefmtenumproc : TIMEFMT_ENUMPROCA, locale : u32, dwflags : TIME_FORMAT_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumTimeFormatsEx(lptimefmtenumprocex : TIMEFMT_ENUMPROCEX, lplocalename : ::windows_sys::core::PCWSTR, dwflags : u32, lparam : super::Foundation:: LPARAM) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumTimeFormatsW(lptimefmtenumproc : TIMEFMT_ENUMPROCW, locale : u32, dwflags : TIME_FORMAT_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumUILanguagesA(lpuilanguageenumproc : UILANGUAGE_ENUMPROCA, dwflags : u32, lparam : isize) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn EnumUILanguagesW(lpuilanguageenumproc : UILANGUAGE_ENUMPROCW, dwflags : u32, lparam : isize) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn FindNLSString(locale : u32, dwfindnlsstringflags : u32, lpstringsource : ::windows_sys::core::PCWSTR, cchsource : i32, lpstringvalue : ::windows_sys::core::PCWSTR, cchvalue : i32, pcchfound : *mut i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindNLSStringEx(lplocalename : ::windows_sys::core::PCWSTR, dwfindnlsstringflags : u32, lpstringsource : ::windows_sys::core::PCWSTR, cchsource : i32, lpstringvalue : ::windows_sys::core::PCWSTR, cchvalue : i32, pcchfound : *mut i32, lpversioninformation : *const NLSVERSIONINFO, lpreserved : *const ::core::ffi::c_void, sorthandle : super::Foundation:: LPARAM) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn FindStringOrdinal(dwfindstringordinalflags : u32, lpstringsource : ::windows_sys::core::PCWSTR, cchsource : i32, lpstringvalue : ::windows_sys::core::PCWSTR, cchvalue : i32, bignorecase : super::Foundation:: BOOL) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn FoldStringA(dwmapflags : FOLD_STRING_MAP_FLAGS, lpsrcstr : ::windows_sys::core::PCSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PSTR, cchdest : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn FoldStringW(dwmapflags : FOLD_STRING_MAP_FLAGS, lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PWSTR, cchdest : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetACP() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCPInfo(codepage : u32, lpcpinfo : *mut CPINFO) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCPInfoExA(codepage : u32, dwflags : u32, lpcpinfoex : *mut CPINFOEXA) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetCPInfoExW(codepage : u32, dwflags : u32, lpcpinfoex : *mut CPINFOEXW) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetCalendarInfoA(locale : u32, calendar : u32, caltype : u32, lpcaldata : ::windows_sys::core::PSTR, cchdata : i32, lpvalue : *mut u32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetCalendarInfoEx(lplocalename : ::windows_sys::core::PCWSTR, calendar : u32, lpreserved : ::windows_sys::core::PCWSTR, caltype : u32, lpcaldata : ::windows_sys::core::PWSTR, cchdata : i32, lpvalue : *mut u32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetCalendarInfoW(locale : u32, calendar : u32, caltype : u32, lpcaldata : ::windows_sys::core::PWSTR, cchdata : i32, lpvalue : *mut u32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetCurrencyFormatA(locale : u32, dwflags : u32, lpvalue : ::windows_sys::core::PCSTR, lpformat : *const CURRENCYFMTA, lpcurrencystr : ::windows_sys::core::PSTR, cchcurrency : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetCurrencyFormatEx(lplocalename : ::windows_sys::core::PCWSTR, dwflags : u32, lpvalue : ::windows_sys::core::PCWSTR, lpformat : *const CURRENCYFMTW, lpcurrencystr : ::windows_sys::core::PWSTR, cchcurrency : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetCurrencyFormatW(locale : u32, dwflags : u32, lpvalue : ::windows_sys::core::PCWSTR, lpformat : *const CURRENCYFMTW, lpcurrencystr : ::windows_sys::core::PWSTR, cchcurrency : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDateFormatA(locale : u32, dwflags : u32, lpdate : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCSTR, lpdatestr : ::windows_sys::core::PSTR, cchdate : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDateFormatEx(lplocalename : ::windows_sys::core::PCWSTR, dwflags : ENUM_DATE_FORMATS_FLAGS, lpdate : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCWSTR, lpdatestr : ::windows_sys::core::PWSTR, cchdate : i32, lpcalendar : ::windows_sys::core::PCWSTR) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDateFormatW(locale : u32, dwflags : u32, lpdate : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCWSTR, lpdatestr : ::windows_sys::core::PWSTR, cchdate : i32) -> i32);
::windows_targets::link!("bcp47mrm.dll" "system" fn GetDistanceOfClosestLanguageInList(pszlanguage : ::windows_sys::core::PCWSTR, pszlanguageslist : ::windows_sys::core::PCWSTR, wchlistdelimiter : u16, pclosestdistance : *mut f64) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDurationFormat(locale : u32, dwflags : u32, lpduration : *const super::Foundation:: SYSTEMTIME, ullduration : u64, lpformat : ::windows_sys::core::PCWSTR, lpdurationstr : ::windows_sys::core::PWSTR, cchduration : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetDurationFormatEx(lplocalename : ::windows_sys::core::PCWSTR, dwflags : u32, lpduration : *const super::Foundation:: SYSTEMTIME, ullduration : u64, lpformat : ::windows_sys::core::PCWSTR, lpdurationstr : ::windows_sys::core::PWSTR, cchduration : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetFileMUIInfo(dwflags : u32, pcwszfilepath : ::windows_sys::core::PCWSTR, pfilemuiinfo : *mut FILEMUIINFO, pcbfilemuiinfo : *mut u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetFileMUIPath(dwflags : u32, pcwszfilepath : ::windows_sys::core::PCWSTR, pwszlanguage : ::windows_sys::core::PWSTR, pcchlanguage : *mut u32, pwszfilemuipath : ::windows_sys::core::PWSTR, pcchfilemuipath : *mut u32, pululenumerator : *mut u64) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetGeoInfoA(location : i32, geotype : SYSGEOTYPE, lpgeodata : ::windows_sys::core::PSTR, cchdata : i32, langid : u16) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetGeoInfoEx(location : ::windows_sys::core::PCWSTR, geotype : SYSGEOTYPE, geodata : ::windows_sys::core::PWSTR, geodatacount : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetGeoInfoW(location : i32, geotype : SYSGEOTYPE, lpgeodata : ::windows_sys::core::PWSTR, cchdata : i32, langid : u16) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetLocaleInfoA(locale : u32, lctype : u32, lplcdata : ::windows_sys::core::PSTR, cchdata : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetLocaleInfoEx(lplocalename : ::windows_sys::core::PCWSTR, lctype : u32, lplcdata : ::windows_sys::core::PWSTR, cchdata : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetLocaleInfoW(locale : u32, lctype : u32, lplcdata : ::windows_sys::core::PWSTR, cchdata : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNLSVersion(function : u32, locale : u32, lpversioninformation : *mut NLSVERSIONINFO) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetNLSVersionEx(function : u32, lplocalename : ::windows_sys::core::PCWSTR, lpversioninformation : *mut NLSVERSIONINFOEX) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetNumberFormatA(locale : u32, dwflags : u32, lpvalue : ::windows_sys::core::PCSTR, lpformat : *const NUMBERFMTA, lpnumberstr : ::windows_sys::core::PSTR, cchnumber : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetNumberFormatEx(lplocalename : ::windows_sys::core::PCWSTR, dwflags : u32, lpvalue : ::windows_sys::core::PCWSTR, lpformat : *const NUMBERFMTW, lpnumberstr : ::windows_sys::core::PWSTR, cchnumber : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetNumberFormatW(locale : u32, dwflags : u32, lpvalue : ::windows_sys::core::PCWSTR, lpformat : *const NUMBERFMTW, lpnumberstr : ::windows_sys::core::PWSTR, cchnumber : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetOEMCP() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetProcessPreferredUILanguages(dwflags : u32, pulnumlanguages : *mut u32, pwszlanguagesbuffer : ::windows_sys::core::PWSTR, pcchlanguagesbuffer : *mut u32) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetStringScripts(dwflags : u32, lpstring : ::windows_sys::core::PCWSTR, cchstring : i32, lpscripts : ::windows_sys::core::PWSTR, cchscripts : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetStringTypeA(locale : u32, dwinfotype : u32, lpsrcstr : ::windows_sys::core::PCSTR, cchsrc : i32, lpchartype : *mut u16) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetStringTypeExA(locale : u32, dwinfotype : u32, lpsrcstr : ::windows_sys::core::PCSTR, cchsrc : i32, lpchartype : *mut u16) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetStringTypeExW(locale : u32, dwinfotype : u32, lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpchartype : *mut u16) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetStringTypeW(dwinfotype : u32, lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpchartype : *mut u16) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetSystemDefaultLCID() -> u32);
::windows_targets::link!("kernel32.dll" "system" fn GetSystemDefaultLangID() -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GetSystemDefaultLocaleName(lplocalename : ::windows_sys::core::PWSTR, cchlocalename : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetSystemDefaultUILanguage() -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetSystemPreferredUILanguages(dwflags : u32, pulnumlanguages : *mut u32, pwszlanguagesbuffer : ::windows_sys::core::PWSTR, pcchlanguagesbuffer : *mut u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GetTextCharset(hdc : super::Graphics::Gdi:: HDC) -> i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn GetTextCharsetInfo(hdc : super::Graphics::Gdi:: HDC, lpsig : *mut FONTSIGNATURE, dwflags : u32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetThreadLocale() -> u32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetThreadPreferredUILanguages(dwflags : u32, pulnumlanguages : *mut u32, pwszlanguagesbuffer : ::windows_sys::core::PWSTR, pcchlanguagesbuffer : *mut u32) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetThreadUILanguage() -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTimeFormatA(locale : u32, dwflags : u32, lptime : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCSTR, lptimestr : ::windows_sys::core::PSTR, cchtime : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTimeFormatEx(lplocalename : ::windows_sys::core::PCWSTR, dwflags : TIME_FORMAT_FLAGS, lptime : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCWSTR, lptimestr : ::windows_sys::core::PWSTR, cchtime : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetTimeFormatW(locale : u32, dwflags : u32, lptime : *const super::Foundation:: SYSTEMTIME, lpformat : ::windows_sys::core::PCWSTR, lptimestr : ::windows_sys::core::PWSTR, cchtime : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetUILanguageInfo(dwflags : u32, pwmszlanguage : ::windows_sys::core::PCWSTR, pwszfallbacklanguages : ::windows_sys::core::PWSTR, pcchfallbacklanguages : *mut u32, pattributes : *mut u32) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn GetUserDefaultGeoName(geoname : ::windows_sys::core::PWSTR, geonamecount : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetUserDefaultLCID() -> u32);
::windows_targets::link!("kernel32.dll" "system" fn GetUserDefaultLangID() -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GetUserDefaultLocaleName(lplocalename : ::windows_sys::core::PWSTR, cchlocalename : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn GetUserDefaultUILanguage() -> u16);
::windows_targets::link!("kernel32.dll" "system" fn GetUserGeoID(geoclass : SYSGEOCLASS) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn GetUserPreferredUILanguages(dwflags : u32, pulnumlanguages : *mut u32, pwszlanguagesbuffer : ::windows_sys::core::PWSTR, pcchlanguagesbuffer : *mut u32) -> super::Foundation:: BOOL);
::windows_targets::link!("normaliz.dll" "system" fn IdnToAscii(dwflags : u32, lpunicodecharstr : ::windows_sys::core::PCWSTR, cchunicodechar : i32, lpasciicharstr : ::windows_sys::core::PWSTR, cchasciichar : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn IdnToNameprepUnicode(dwflags : u32, lpunicodecharstr : ::windows_sys::core::PCWSTR, cchunicodechar : i32, lpnameprepcharstr : ::windows_sys::core::PWSTR, cchnameprepchar : i32) -> i32);
::windows_targets::link!("normaliz.dll" "system" fn IdnToUnicode(dwflags : u32, lpasciicharstr : ::windows_sys::core::PCWSTR, cchasciichar : i32, lpunicodecharstr : ::windows_sys::core::PWSTR, cchunicodechar : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDBCSLeadByte(testchar : u8) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsDBCSLeadByteEx(codepage : u32, testchar : u8) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsNLSDefinedString(function : u32, dwflags : u32, lpversioninformation : *const NLSVERSIONINFO, lpstring : ::windows_sys::core::PCWSTR, cchstr : i32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsNormalizedString(normform : NORM_FORM, lpstring : ::windows_sys::core::PCWSTR, cwlength : i32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("advapi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsTextUnicode(lpv : *const ::core::ffi::c_void, isize : i32, lpiresult : *mut IS_TEXT_UNICODE_RESULT) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsValidCodePage(codepage : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsValidLanguageGroup(languagegroup : u32, dwflags : ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsValidLocale(locale : u32, dwflags : IS_VALID_LOCALE_FLAGS) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn IsValidLocaleName(lplocalename : ::windows_sys::core::PCWSTR) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn IsValidNLSVersion(function : u32, lplocalename : ::windows_sys::core::PCWSTR, lpversioninformation : *const NLSVERSIONINFOEX) -> u32);
::windows_targets::link!("bcp47mrm.dll" "system" fn IsWellFormedTag(psztag : ::windows_sys::core::PCWSTR) -> u8);
::windows_targets::link!("kernel32.dll" "system" fn LCIDToLocaleName(locale : u32, lpname : ::windows_sys::core::PWSTR, cchname : i32, dwflags : u32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn LCMapStringA(locale : u32, dwmapflags : u32, lpsrcstr : ::windows_sys::core::PCSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PSTR, cchdest : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn LCMapStringEx(lplocalename : ::windows_sys::core::PCWSTR, dwmapflags : u32, lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PWSTR, cchdest : i32, lpversioninformation : *const NLSVERSIONINFO, lpreserved : *const ::core::ffi::c_void, sorthandle : super::Foundation:: LPARAM) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn LCMapStringW(locale : u32, dwmapflags : u32, lpsrcstr : ::windows_sys::core::PCWSTR, cchsrc : i32, lpdeststr : ::windows_sys::core::PWSTR, cchdest : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn LocaleNameToLCID(lpname : ::windows_sys::core::PCWSTR, dwflags : u32) -> u32);
::windows_targets::link!("elscore.dll" "system" fn MappingDoAction(pbag : *mut MAPPING_PROPERTY_BAG, dwrangeindex : u32, pszactionid : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("elscore.dll" "system" fn MappingFreePropertyBag(pbag : *const MAPPING_PROPERTY_BAG) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("elscore.dll" "system" fn MappingFreeServices(pserviceinfo : *const MAPPING_SERVICE_INFO) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("elscore.dll" "system" fn MappingGetServices(poptions : *const MAPPING_ENUM_OPTIONS, prgservices : *mut *mut MAPPING_SERVICE_INFO, pdwservicescount : *mut u32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("elscore.dll" "system" fn MappingRecognizeText(pserviceinfo : *const MAPPING_SERVICE_INFO, psztext : ::windows_sys::core::PCWSTR, dwlength : u32, dwindex : u32, poptions : *const MAPPING_OPTIONS, pbag : *mut MAPPING_PROPERTY_BAG) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("kernel32.dll" "system" fn MultiByteToWideChar(codepage : u32, dwflags : MULTI_BYTE_TO_WIDE_CHAR_FLAGS, lpmultibytestr : ::windows_sys::core::PCSTR, cbmultibyte : i32, lpwidecharstr : ::windows_sys::core::PWSTR, cchwidechar : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn NormalizeString(normform : NORM_FORM, lpsrcstring : ::windows_sys::core::PCWSTR, cwsrclength : i32, lpdststring : ::windows_sys::core::PWSTR, cwdstlength : i32) -> i32);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn NotifyUILanguageChange(dwflags : u32, pcwstrnewlanguage : ::windows_sys::core::PCWSTR, pcwstrpreviouslanguage : ::windows_sys::core::PCWSTR, dwreserved : u32, pdwstatusrtrn : *mut u32) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn ResolveLocaleName(lpnametoresolve : ::windows_sys::core::PCWSTR, lplocalename : ::windows_sys::core::PWSTR, cchlocalename : i32) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn RestoreThreadPreferredUILanguages(snapshot : HSAVEDUILANGUAGES) -> ());
::windows_targets::link!("usp10.dll" "system" fn ScriptApplyDigitSubstitution(psds : *const SCRIPT_DIGITSUBSTITUTE, psc : *mut SCRIPT_CONTROL, pss : *mut SCRIPT_STATE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptApplyLogicalWidth(pidx : *const i32, cchars : i32, cglyphs : i32, pwlogclust : *const u16, psva : *const SCRIPT_VISATTR, piadvance : *const i32, psa : *const SCRIPT_ANALYSIS, pabc : *mut super::Graphics::Gdi:: ABC, pijustify : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptBreak(pwcchars : ::windows_sys::core::PCWSTR, cchars : i32, psa : *const SCRIPT_ANALYSIS, psla : *mut SCRIPT_LOGATTR) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ScriptCPtoX(icp : i32, ftrailing : super::Foundation:: BOOL, cchars : i32, cglyphs : i32, pwlogclust : *const u16, psva : *const SCRIPT_VISATTR, piadvance : *const i32, psa : *const SCRIPT_ANALYSIS, pix : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptCacheGetHeight(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, tmheight : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptFreeCache(psc : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetCMap(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, pwcinchars : ::windows_sys::core::PCWSTR, cchars : i32, dwflags : u32, pwoutglyphs : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetFontAlternateGlyphs(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, tagfeature : u32, wglyphid : u16, cmaxalternates : i32, palternateglyphs : *mut u16, pcalternates : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetFontFeatureTags(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, cmaxtags : i32, pfeaturetags : *mut u32, pctags : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetFontLanguageTags(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, tagscript : u32, cmaxtags : i32, plangsystags : *mut u32, pctags : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetFontProperties(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, sfp : *mut SCRIPT_FONTPROPERTIES) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetFontScriptTags(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, cmaxtags : i32, pscripttags : *mut u32, pctags : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptGetGlyphABCWidth(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, wglyph : u16, pabc : *mut super::Graphics::Gdi:: ABC) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptGetLogicalWidths(psa : *const SCRIPT_ANALYSIS, cchars : i32, cglyphs : i32, piglyphwidth : *const i32, pwlogclust : *const u16, psva : *const SCRIPT_VISATTR, pidx : *const i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptGetProperties(ppsp : *mut *mut *mut SCRIPT_PROPERTIES, pinumscripts : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptIsComplex(pwcinchars : ::windows_sys::core::PCWSTR, cinchars : i32, dwflags : SCRIPT_IS_COMPLEX_FLAGS) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptItemize(pwcinchars : ::windows_sys::core::PCWSTR, cinchars : i32, cmaxitems : i32, pscontrol : *const SCRIPT_CONTROL, psstate : *const SCRIPT_STATE, pitems : *mut SCRIPT_ITEM, pcitems : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptItemizeOpenType(pwcinchars : ::windows_sys::core::PCWSTR, cinchars : i32, cmaxitems : i32, pscontrol : *const SCRIPT_CONTROL, psstate : *const SCRIPT_STATE, pitems : *mut SCRIPT_ITEM, pscripttags : *mut u32, pcitems : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptJustify(psva : *const SCRIPT_VISATTR, piadvance : *const i32, cglyphs : i32, idx : i32, iminkashida : i32, pijustify : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptLayout(cruns : i32, pblevel : *const u8, pivisualtological : *mut i32, pilogicaltovisual : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptPlace(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, pwglyphs : *const u16, cglyphs : i32, psva : *const SCRIPT_VISATTR, psa : *mut SCRIPT_ANALYSIS, piadvance : *mut i32, pgoffset : *mut GOFFSET, pabc : *mut super::Graphics::Gdi:: ABC) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptPlaceOpenType(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *mut SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, rcrangechars : *const i32, rprangeproperties : *const *const TEXTRANGE_PROPERTIES, cranges : i32, pwcchars : ::windows_sys::core::PCWSTR, pwlogclust : *const u16, pcharprops : *const SCRIPT_CHARPROP, cchars : i32, pwglyphs : *const u16, pglyphprops : *const SCRIPT_GLYPHPROP, cglyphs : i32, piadvance : *mut i32, pgoffset : *mut GOFFSET, pabc : *mut super::Graphics::Gdi:: ABC) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptPositionSingleGlyph(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, tagfeature : u32, lparameter : i32, wglyphid : u16, iadvance : i32, goffset : GOFFSET, pioutadvance : *mut i32, poutgoffset : *mut GOFFSET) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptRecordDigitSubstitution(locale : u32, psds : *mut SCRIPT_DIGITSUBSTITUTE) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptShape(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, pwcchars : ::windows_sys::core::PCWSTR, cchars : i32, cmaxglyphs : i32, psa : *mut SCRIPT_ANALYSIS, pwoutglyphs : *mut u16, pwlogclust : *mut u16, psva : *mut SCRIPT_VISATTR, pcglyphs : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptShapeOpenType(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *mut SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, rcrangechars : *const i32, rprangeproperties : *const *const TEXTRANGE_PROPERTIES, cranges : i32, pwcchars : ::windows_sys::core::PCWSTR, cchars : i32, cmaxglyphs : i32, pwlogclust : *mut u16, pcharprops : *mut SCRIPT_CHARPROP, pwoutglyphs : *mut u16, poutglyphprops : *mut SCRIPT_GLYPHPROP, pcglyphs : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptStringAnalyse(hdc : super::Graphics::Gdi:: HDC, pstring : *const ::core::ffi::c_void, cstring : i32, cglyphs : i32, icharset : i32, dwflags : u32, ireqwidth : i32, pscontrol : *const SCRIPT_CONTROL, psstate : *const SCRIPT_STATE, pidx : *const i32, ptabdef : *const SCRIPT_TABDEF, pbinclass : *const u8, pssa : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ScriptStringCPtoX(ssa : *const ::core::ffi::c_void, icp : i32, ftrailing : super::Foundation:: BOOL, px : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptStringFree(pssa : *mut *mut ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptStringGetLogicalWidths(ssa : *const ::core::ffi::c_void, pidx : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptStringGetOrder(ssa : *const ::core::ffi::c_void, puorder : *mut u32) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn ScriptStringOut(ssa : *const ::core::ffi::c_void, ix : i32, iy : i32, uoptions : super::Graphics::Gdi:: ETO_OPTIONS, prc : *const super::Foundation:: RECT, iminsel : i32, imaxsel : i32, fdisabled : super::Foundation:: BOOL) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptStringValidate(ssa : *const ::core::ffi::c_void) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptStringXtoCP(ssa : *const ::core::ffi::c_void, ix : i32, pich : *mut i32, pitrailing : *mut i32) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptString_pLogAttr(ssa : *const ::core::ffi::c_void) -> *mut SCRIPT_LOGATTR);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn ScriptString_pSize(ssa : *const ::core::ffi::c_void) -> *mut super::Foundation:: SIZE);
::windows_targets::link!("usp10.dll" "system" fn ScriptString_pcOutChars(ssa : *const ::core::ffi::c_void) -> *mut i32);
#[cfg(feature = "Win32_Graphics_Gdi")]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Graphics_Gdi\"`"] fn ScriptSubstituteSingleGlyph(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, psa : *const SCRIPT_ANALYSIS, tagscript : u32, taglangsys : u32, tagfeature : u32, lparameter : i32, wglyphid : u16, pwoutglyphid : *mut u16) -> ::windows_sys::core::HRESULT);
#[cfg(all(feature = "Win32_Foundation", feature = "Win32_Graphics_Gdi"))]
::windows_targets::link!("usp10.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`, `\"Win32_Graphics_Gdi\"`"] fn ScriptTextOut(hdc : super::Graphics::Gdi:: HDC, psc : *mut *mut ::core::ffi::c_void, x : i32, y : i32, fuoptions : u32, lprc : *const super::Foundation:: RECT, psa : *const SCRIPT_ANALYSIS, pwcreserved : ::windows_sys::core::PCWSTR, ireserved : i32, pwglyphs : *const u16, cglyphs : i32, piadvance : *const i32, pijustify : *const i32, pgoffset : *const GOFFSET) -> ::windows_sys::core::HRESULT);
::windows_targets::link!("usp10.dll" "system" fn ScriptXtoCP(ix : i32, cchars : i32, cglyphs : i32, pwlogclust : *const u16, psva : *const SCRIPT_VISATTR, piadvance : *const i32, psa : *const SCRIPT_ANALYSIS, picp : *mut i32, pitrailing : *mut i32) -> ::windows_sys::core::HRESULT);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCalendarInfoA(locale : u32, calendar : u32, caltype : u32, lpcaldata : ::windows_sys::core::PCSTR) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetCalendarInfoW(locale : u32, calendar : u32, caltype : u32, lpcaldata : ::windows_sys::core::PCWSTR) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetLocaleInfoA(locale : u32, lctype : u32, lplcdata : ::windows_sys::core::PCSTR) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetLocaleInfoW(locale : u32, lctype : u32, lplcdata : ::windows_sys::core::PCWSTR) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetProcessPreferredUILanguages(dwflags : u32, pwszlanguagesbuffer : ::windows_sys::core::PCWSTR, pulnumlanguages : *mut u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetThreadLocale(locale : u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetThreadPreferredUILanguages(dwflags : u32, pwszlanguagesbuffer : ::windows_sys::core::PCWSTR, pulnumlanguages : *mut u32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetThreadPreferredUILanguages2(flags : u32, languages : ::windows_sys::core::PCWSTR, numlanguagesset : *mut u32, snapshot : *mut HSAVEDUILANGUAGES) -> super::Foundation:: BOOL);
::windows_targets::link!("kernel32.dll" "system" fn SetThreadUILanguage(langid : u16) -> u16);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetUserGeoID(geoid : i32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn SetUserGeoName(geoname : ::windows_sys::core::PCWSTR) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("gdi32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn TranslateCharsetInfo(lpsrc : *mut u32, lpcs : *mut CHARSETINFO, dwflags : TRANSLATE_CHARSET_INFO_FLAGS) -> super::Foundation:: BOOL);
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_FROM_U_CALLBACK_ESCAPE(context : *const ::core::ffi::c_void, fromuargs : *mut UConverterFromUnicodeArgs, codeunits : *const u16, length : i32, codepoint : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_FROM_U_CALLBACK_SKIP(context : *const ::core::ffi::c_void, fromuargs : *mut UConverterFromUnicodeArgs, codeunits : *const u16, length : i32, codepoint : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_FROM_U_CALLBACK_STOP(context : *const ::core::ffi::c_void, fromuargs : *mut UConverterFromUnicodeArgs, codeunits : *const u16, length : i32, codepoint : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_FROM_U_CALLBACK_SUBSTITUTE(context : *const ::core::ffi::c_void, fromuargs : *mut UConverterFromUnicodeArgs, codeunits : *const u16, length : i32, codepoint : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_TO_U_CALLBACK_ESCAPE(context : *const ::core::ffi::c_void, touargs : *mut UConverterToUnicodeArgs, codeunits : ::windows_sys::core::PCSTR, length : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_TO_U_CALLBACK_SKIP(context : *const ::core::ffi::c_void, touargs : *mut UConverterToUnicodeArgs, codeunits : ::windows_sys::core::PCSTR, length : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_TO_U_CALLBACK_STOP(context : *const ::core::ffi::c_void, touargs : *mut UConverterToUnicodeArgs, codeunits : ::windows_sys::core::PCSTR, length : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn UCNV_TO_U_CALLBACK_SUBSTITUTE(context : *const ::core::ffi::c_void, touargs : *mut UConverterToUnicodeArgs, codeunits : ::windows_sys::core::PCSTR, length : i32, reason : UConverterCallbackReason, err : *mut UErrorCode) -> ());
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn VerifyScripts(dwflags : u32, lplocalescripts : ::windows_sys::core::PCWSTR, cchlocalescripts : i32, lptestscripts : ::windows_sys::core::PCWSTR, cchtestscripts : i32) -> super::Foundation:: BOOL);
#[cfg(feature = "Win32_Foundation")]
::windows_targets::link!("kernel32.dll" "system" #[doc = "Required features: `\"Win32_Foundation\"`"] fn WideCharToMultiByte(codepage : u32, dwflags : u32, lpwidecharstr : ::windows_sys::core::PCWSTR, cchwidechar : i32, lpmultibytestr : ::windows_sys::core::PSTR, cbmultibyte : i32, lpdefaultchar : ::windows_sys::core::PCSTR, lpuseddefaultchar : *mut super::Foundation:: BOOL) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrcatA(lpstring1 : ::windows_sys::core::PSTR, lpstring2 : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrcatW(lpstring1 : ::windows_sys::core::PWSTR, lpstring2 : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrcmpA(lpstring1 : ::windows_sys::core::PCSTR, lpstring2 : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrcmpW(lpstring1 : ::windows_sys::core::PCWSTR, lpstring2 : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrcmpiA(lpstring1 : ::windows_sys::core::PCSTR, lpstring2 : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrcmpiW(lpstring1 : ::windows_sys::core::PCWSTR, lpstring2 : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrcpyA(lpstring1 : ::windows_sys::core::PSTR, lpstring2 : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrcpyW(lpstring1 : ::windows_sys::core::PWSTR, lpstring2 : ::windows_sys::core::PCWSTR) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrcpynA(lpstring1 : ::windows_sys::core::PSTR, lpstring2 : ::windows_sys::core::PCSTR, imaxlength : i32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrcpynW(lpstring1 : ::windows_sys::core::PWSTR, lpstring2 : ::windows_sys::core::PCWSTR, imaxlength : i32) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("kernel32.dll" "system" fn lstrlenA(lpstring : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("kernel32.dll" "system" fn lstrlenW(lpstring : ::windows_sys::core::PCWSTR) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_UCharsToChars(us : *const u16, cs : ::windows_sys::core::PCSTR, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_austrcpy(dst : ::windows_sys::core::PCSTR, src : *const u16) -> ::windows_sys::core::PSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_austrncpy(dst : ::windows_sys::core::PCSTR, src : *const u16, n : i32) -> ::windows_sys::core::PSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_catclose(catd : *mut UResourceBundle) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_catgets(catd : *mut UResourceBundle, set_num : i32, msg_num : i32, s : *const u16, len : *mut i32, ec : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_catopen(name : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, ec : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn u_charAge(c : i32, versionarray : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_charDigitValue(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_charDirection(c : i32) -> UCharDirection);
::windows_targets::link!("icu.dll" "cdecl" fn u_charFromName(namechoice : UCharNameChoice, name : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_charMirror(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_charName(code : i32, namechoice : UCharNameChoice, buffer : ::windows_sys::core::PCSTR, bufferlength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_charType(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_charsToUChars(cs : ::windows_sys::core::PCSTR, us : *mut u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_cleanup() -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_countChar32(s : *const u16, length : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_digit(ch : i32, radix : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_enumCharNames(start : i32, limit : i32, r#fn : *mut UEnumCharNamesFn, context : *mut ::core::ffi::c_void, namechoice : UCharNameChoice, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_enumCharTypes(enumrange : *mut UCharEnumTypeRange, context : *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_errorName(code : UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_foldCase(c : i32, options : u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_forDigit(digit : i32, radix : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_formatMessage(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, result : *mut u16, resultlength : i32, status : *mut UErrorCode, ...) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_formatMessageWithError(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, result : *mut u16, resultlength : i32, parseerror : *mut UParseError, status : *mut UErrorCode, ...) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getBidiPairedBracket(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getBinaryPropertySet(property : UProperty, perrorcode : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn u_getCombiningClass(c : i32) -> u8);
::windows_targets::link!("icu.dll" "cdecl" fn u_getDataVersion(dataversionfillin : *mut u8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_getFC_NFKC_Closure(c : i32, dest : *mut u16, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getIntPropertyMap(property : UProperty, perrorcode : *mut UErrorCode) -> *mut UCPMap);
::windows_targets::link!("icu.dll" "cdecl" fn u_getIntPropertyMaxValue(which : UProperty) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getIntPropertyMinValue(which : UProperty) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getIntPropertyValue(c : i32, which : UProperty) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getNumericValue(c : i32) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn u_getPropertyEnum(alias : ::windows_sys::core::PCSTR) -> UProperty);
::windows_targets::link!("icu.dll" "cdecl" fn u_getPropertyName(property : UProperty, namechoice : UPropertyNameChoice) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_getPropertyValueEnum(property : UProperty, alias : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_getPropertyValueName(property : UProperty, value : i32, namechoice : UPropertyNameChoice) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_getUnicodeVersion(versionarray : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_getVersion(versionarray : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_hasBinaryProperty(c : i32, which : UProperty) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_init(status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_isIDIgnorable(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isIDPart(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isIDStart(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isISOControl(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isJavaIDPart(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isJavaIDStart(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isJavaSpaceChar(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isMirrored(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isUAlphabetic(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isULowercase(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isUUppercase(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isUWhiteSpace(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isWhitespace(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isalnum(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isalpha(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isbase(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isblank(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_iscntrl(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isdefined(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isdigit(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isgraph(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_islower(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isprint(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_ispunct(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isspace(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_istitle(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isupper(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_isxdigit(c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_memcasecmp(s1 : *const u16, s2 : *const u16, length : i32, options : u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_memchr(s : *const u16, c : u16, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memchr32(s : *const u16, c : i32, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memcmp(buf1 : *const u16, buf2 : *const u16, count : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_memcmpCodePointOrder(s1 : *const u16, s2 : *const u16, count : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_memcpy(dest : *mut u16, src : *const u16, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memmove(dest : *mut u16, src : *const u16, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memrchr(s : *const u16, c : u16, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memrchr32(s : *const u16, c : i32, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_memset(dest : *mut u16, c : u16, count : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_parseMessage(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, source : *const u16, sourcelength : i32, status : *mut UErrorCode, ...) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_parseMessageWithError(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, source : *const u16, sourcelength : i32, parseerror : *mut UParseError, status : *mut UErrorCode, ...) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_setMemoryFunctions(context : *const ::core::ffi::c_void, a : *mut UMemAllocFn, r : *mut UMemReallocFn, f : *mut UMemFreeFn, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_shapeArabic(source : *const u16, sourcelength : i32, dest : *mut u16, destsize : i32, options : u32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strCaseCompare(s1 : *const u16, length1 : i32, s2 : *const u16, length2 : i32, options : u32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strCompare(s1 : *const u16, length1 : i32, s2 : *const u16, length2 : i32, codepointorder : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strCompareIter(iter1 : *mut UCharIterator, iter2 : *mut UCharIterator, codepointorder : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFindFirst(s : *const u16, length : i32, substring : *const u16, sublength : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFindLast(s : *const u16, length : i32, substring : *const u16, sublength : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFoldCase(dest : *mut u16, destcapacity : i32, src : *const u16, srclength : i32, options : u32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromJavaModifiedUTF8WithSub(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : ::windows_sys::core::PCSTR, srclength : i32, subchar : i32, pnumsubstitutions : *mut i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromUTF32(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : *const i32, srclength : i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromUTF32WithSub(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : *const i32, srclength : i32, subchar : i32, pnumsubstitutions : *mut i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromUTF8(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromUTF8Lenient(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromUTF8WithSub(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : ::windows_sys::core::PCSTR, srclength : i32, subchar : i32, pnumsubstitutions : *mut i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strFromWCS(dest : *mut u16, destcapacity : i32, pdestlength : *mut i32, src : ::windows_sys::core::PCWSTR, srclength : i32, perrorcode : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strHasMoreChar32Than(s : *const u16, length : i32, number : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToJavaModifiedUTF8(dest : ::windows_sys::core::PCSTR, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToLower(dest : *mut u16, destcapacity : i32, src : *const u16, srclength : i32, locale : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToTitle(dest : *mut u16, destcapacity : i32, src : *const u16, srclength : i32, titleiter : *mut UBreakIterator, locale : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToUTF32(dest : *mut i32, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> *mut i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToUTF32WithSub(dest : *mut i32, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, subchar : i32, pnumsubstitutions : *mut i32, perrorcode : *mut UErrorCode) -> *mut i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToUTF8(dest : ::windows_sys::core::PCSTR, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToUTF8WithSub(dest : ::windows_sys::core::PCSTR, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, subchar : i32, pnumsubstitutions : *mut i32, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToUpper(dest : *mut u16, destcapacity : i32, src : *const u16, srclength : i32, locale : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strToWCS(dest : ::windows_sys::core::PCWSTR, destcapacity : i32, pdestlength : *mut i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PWSTR);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcasecmp(s1 : *const u16, s2 : *const u16, options : u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcat(dst : *mut u16, src : *const u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strchr(s : *const u16, c : u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strchr32(s : *const u16, c : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcmp(s1 : *const u16, s2 : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcmpCodePointOrder(s1 : *const u16, s2 : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcpy(dst : *mut u16, src : *const u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strcspn(string : *const u16, matchset : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strlen(s : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strncasecmp(s1 : *const u16, s2 : *const u16, n : i32, options : u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strncat(dst : *mut u16, src : *const u16, n : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strncmp(ucs1 : *const u16, ucs2 : *const u16, n : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strncmpCodePointOrder(s1 : *const u16, s2 : *const u16, n : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strncpy(dst : *mut u16, src : *const u16, n : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strpbrk(string : *const u16, matchset : *const u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strrchr(s : *const u16, c : u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strrchr32(s : *const u16, c : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strrstr(s : *const u16, substring : *const u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strspn(string : *const u16, matchset : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_strstr(s : *const u16, substring : *const u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_strtok_r(src : *mut u16, delim : *const u16, savestate : *mut *mut u16) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_tolower(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_totitle(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_toupper(c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_uastrcpy(dst : *mut u16, src : ::windows_sys::core::PCSTR) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_uastrncpy(dst : *mut u16, src : ::windows_sys::core::PCSTR, n : i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn u_unescape(src : ::windows_sys::core::PCSTR, dest : *mut u16, destcapacity : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_unescapeAt(charat : UNESCAPE_CHAR_AT, offset : *mut i32, length : i32, context : *mut ::core::ffi::c_void) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_versionFromString(versionarray : *mut u8, versionstring : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_versionFromUString(versionarray : *mut u8, versionstring : *const u16) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_versionToString(versionarray : *const u8, versionstring : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_vformatMessage(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, result : *mut u16, resultlength : i32, ap : *mut i8, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_vformatMessageWithError(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, result : *mut u16, resultlength : i32, parseerror : *mut UParseError, ap : *mut i8, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn u_vparseMessage(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, source : *const u16, sourcelength : i32, ap : *mut i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn u_vparseMessageWithError(locale : ::windows_sys::core::PCSTR, pattern : *const u16, patternlength : i32, source : *const u16, sourcelength : i32, ap : *mut i8, parseerror : *mut UParseError, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_close(pbidi : *mut UBiDi) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_countParagraphs(pbidi : *mut UBiDi) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_countRuns(pbidi : *mut UBiDi, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getBaseDirection(text : *const u16, length : i32) -> UBiDiDirection);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getClassCallback(pbidi : *mut UBiDi, r#fn : *mut UBiDiClassCallback, context : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getCustomizedClass(pbidi : *mut UBiDi, c : i32) -> UCharDirection);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getDirection(pbidi : *const UBiDi) -> UBiDiDirection);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLength(pbidi : *const UBiDi) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLevelAt(pbidi : *const UBiDi, charindex : i32) -> u8);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLevels(pbidi : *mut UBiDi, perrorcode : *mut UErrorCode) -> *mut u8);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLogicalIndex(pbidi : *mut UBiDi, visualindex : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLogicalMap(pbidi : *mut UBiDi, indexmap : *mut i32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getLogicalRun(pbidi : *const UBiDi, logicalposition : i32, plogicallimit : *mut i32, plevel : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getParaLevel(pbidi : *const UBiDi) -> u8);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getParagraph(pbidi : *const UBiDi, charindex : i32, pparastart : *mut i32, pparalimit : *mut i32, pparalevel : *mut u8, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getParagraphByIndex(pbidi : *const UBiDi, paraindex : i32, pparastart : *mut i32, pparalimit : *mut i32, pparalevel : *mut u8, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getProcessedLength(pbidi : *const UBiDi) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getReorderingMode(pbidi : *mut UBiDi) -> UBiDiReorderingMode);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getReorderingOptions(pbidi : *mut UBiDi) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getResultLength(pbidi : *const UBiDi) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getText(pbidi : *const UBiDi) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getVisualIndex(pbidi : *mut UBiDi, logicalindex : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getVisualMap(pbidi : *mut UBiDi, indexmap : *mut i32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_getVisualRun(pbidi : *mut UBiDi, runindex : i32, plogicalstart : *mut i32, plength : *mut i32) -> UBiDiDirection);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_invertMap(srcmap : *const i32, destmap : *mut i32, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_isInverse(pbidi : *mut UBiDi) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_isOrderParagraphsLTR(pbidi : *mut UBiDi) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_open() -> *mut UBiDi);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_openSized(maxlength : i32, maxruncount : i32, perrorcode : *mut UErrorCode) -> *mut UBiDi);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_orderParagraphsLTR(pbidi : *mut UBiDi, orderparagraphsltr : i8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_reorderLogical(levels : *const u8, length : i32, indexmap : *mut i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_reorderVisual(levels : *const u8, length : i32, indexmap : *mut i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setClassCallback(pbidi : *mut UBiDi, newfn : UBiDiClassCallback, newcontext : *const ::core::ffi::c_void, oldfn : *mut UBiDiClassCallback, oldcontext : *const *const ::core::ffi::c_void, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setContext(pbidi : *mut UBiDi, prologue : *const u16, prolength : i32, epilogue : *const u16, epilength : i32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setInverse(pbidi : *mut UBiDi, isinverse : i8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setLine(pparabidi : *const UBiDi, start : i32, limit : i32, plinebidi : *mut UBiDi, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setPara(pbidi : *mut UBiDi, text : *const u16, length : i32, paralevel : u8, embeddinglevels : *mut u8, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setReorderingMode(pbidi : *mut UBiDi, reorderingmode : UBiDiReorderingMode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_setReorderingOptions(pbidi : *mut UBiDi, reorderingoptions : u32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_writeReordered(pbidi : *mut UBiDi, dest : *mut u16, destsize : i32, options : u16, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubidi_writeReverse(src : *const u16, srclength : i32, dest : *mut u16, destsize : i32, options : u16, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubiditransform_close(pbiditransform : *mut UBiDiTransform) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubiditransform_open(perrorcode : *mut UErrorCode) -> *mut UBiDiTransform);
::windows_targets::link!("icu.dll" "cdecl" fn ubiditransform_transform(pbiditransform : *mut UBiDiTransform, src : *const u16, srclength : i32, dest : *mut u16, destsize : i32, inparalevel : u8, inorder : UBiDiOrder, outparalevel : u8, outorder : UBiDiOrder, domirroring : UBiDiMirroring, shapingoptions : u32, perrorcode : *mut UErrorCode) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ublock_getCode(c : i32) -> UBlockCode);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_close(bi : *mut UBreakIterator) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_current(bi : *const UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_first(bi : *mut UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_following(bi : *mut UBreakIterator, offset : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_getAvailable(index : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_getBinaryRules(bi : *mut UBreakIterator, binaryrules : *mut u8, rulescapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_getLocaleByType(bi : *const UBreakIterator, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_getRuleStatus(bi : *mut UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_getRuleStatusVec(bi : *mut UBreakIterator, fillinvec : *mut i32, capacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_isBoundary(bi : *mut UBreakIterator, offset : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_last(bi : *mut UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_next(bi : *mut UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_open(r#type : UBreakIteratorType, locale : ::windows_sys::core::PCSTR, text : *const u16, textlength : i32, status : *mut UErrorCode) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_openBinaryRules(binaryrules : *const u8, ruleslength : i32, text : *const u16, textlength : i32, status : *mut UErrorCode) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_openRules(rules : *const u16, ruleslength : i32, text : *const u16, textlength : i32, parseerr : *mut UParseError, status : *mut UErrorCode) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_preceding(bi : *mut UBreakIterator, offset : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_previous(bi : *mut UBreakIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_refreshUText(bi : *mut UBreakIterator, text : *mut UText, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_safeClone(bi : *const UBreakIterator, stackbuffer : *mut ::core::ffi::c_void, pbuffersize : *mut i32, status : *mut UErrorCode) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_setText(bi : *mut UBreakIterator, text : *const u16, textlength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ubrk_setUText(bi : *mut UBreakIterator, text : *mut UText, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_add(cal : *mut *mut ::core::ffi::c_void, field : UCalendarDateFields, amount : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_clear(calendar : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_clearField(cal : *mut *mut ::core::ffi::c_void, field : UCalendarDateFields) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_clone(cal : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_close(cal : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_equivalentTo(cal1 : *const *const ::core::ffi::c_void, cal2 : *const *const ::core::ffi::c_void) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_get(cal : *const *const ::core::ffi::c_void, field : UCalendarDateFields, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getAttribute(cal : *const *const ::core::ffi::c_void, attr : UCalendarAttribute) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getAvailable(localeindex : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getCanonicalTimeZoneID(id : *const u16, len : i32, result : *mut u16, resultcapacity : i32, issystemid : *mut i8, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getDSTSavings(zoneid : *const u16, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getDayOfWeekType(cal : *const *const ::core::ffi::c_void, dayofweek : UCalendarDaysOfWeek, status : *mut UErrorCode) -> UCalendarWeekdayType);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getDefaultTimeZone(result : *mut u16, resultcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getFieldDifference(cal : *mut *mut ::core::ffi::c_void, target : f64, field : UCalendarDateFields, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getGregorianChange(cal : *const *const ::core::ffi::c_void, perrorcode : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getHostTimeZone(result : *mut u16, resultcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getKeywordValuesForLocale(key : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, commonlyused : i8, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getLimit(cal : *const *const ::core::ffi::c_void, field : UCalendarDateFields, r#type : UCalendarLimitType, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getLocaleByType(cal : *const *const ::core::ffi::c_void, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getMillis(cal : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getNow() -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getTZDataVersion(status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getTimeZoneDisplayName(cal : *const *const ::core::ffi::c_void, r#type : UCalendarDisplayNameType, locale : ::windows_sys::core::PCSTR, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getTimeZoneID(cal : *const *const ::core::ffi::c_void, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getTimeZoneIDForWindowsID(winid : *const u16, len : i32, region : ::windows_sys::core::PCSTR, id : *mut u16, idcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getTimeZoneTransitionDate(cal : *const *const ::core::ffi::c_void, r#type : UTimeZoneTransitionType, transition : *mut f64, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getType(cal : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getWeekendTransition(cal : *const *const ::core::ffi::c_void, dayofweek : UCalendarDaysOfWeek, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_getWindowsTimeZoneID(id : *const u16, len : i32, winid : *mut u16, winidcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_inDaylightTime(cal : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_isSet(cal : *const *const ::core::ffi::c_void, field : UCalendarDateFields) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_isWeekend(cal : *const *const ::core::ffi::c_void, date : f64, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_open(zoneid : *const u16, len : i32, locale : ::windows_sys::core::PCSTR, r#type : UCalendarType, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_openCountryTimeZones(country : ::windows_sys::core::PCSTR, ec : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_openTimeZoneIDEnumeration(zonetype : USystemTimeZoneType, region : ::windows_sys::core::PCSTR, rawoffset : *const i32, ec : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_openTimeZones(ec : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucal_roll(cal : *mut *mut ::core::ffi::c_void, field : UCalendarDateFields, amount : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_set(cal : *mut *mut ::core::ffi::c_void, field : UCalendarDateFields, value : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setAttribute(cal : *mut *mut ::core::ffi::c_void, attr : UCalendarAttribute, newvalue : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setDate(cal : *mut *mut ::core::ffi::c_void, year : i32, month : i32, date : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setDateTime(cal : *mut *mut ::core::ffi::c_void, year : i32, month : i32, date : i32, hour : i32, minute : i32, second : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setDefaultTimeZone(zoneid : *const u16, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setGregorianChange(cal : *mut *mut ::core::ffi::c_void, date : f64, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setMillis(cal : *mut *mut ::core::ffi::c_void, datetime : f64, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucal_setTimeZone(cal : *mut *mut ::core::ffi::c_void, zoneid : *const u16, len : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_close(csm : *mut UCaseMap) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_getBreakIterator(csm : *const UCaseMap) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_getLocale(csm : *const UCaseMap) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_getOptions(csm : *const UCaseMap) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_open(locale : ::windows_sys::core::PCSTR, options : u32, perrorcode : *mut UErrorCode) -> *mut UCaseMap);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_setBreakIterator(csm : *mut UCaseMap, itertoadopt : *mut UBreakIterator, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_setLocale(csm : *mut UCaseMap, locale : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_setOptions(csm : *mut UCaseMap, options : u32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_toTitle(csm : *mut UCaseMap, dest : *mut u16, destcapacity : i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_utf8FoldCase(csm : *const UCaseMap, dest : ::windows_sys::core::PCSTR, destcapacity : i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_utf8ToLower(csm : *const UCaseMap, dest : ::windows_sys::core::PCSTR, destcapacity : i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_utf8ToTitle(csm : *mut UCaseMap, dest : ::windows_sys::core::PCSTR, destcapacity : i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucasemap_utf8ToUpper(csm : *const UCaseMap, dest : ::windows_sys::core::PCSTR, destcapacity : i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_close(ucfpos : *mut UConstrainedFieldPosition) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_constrainCategory(ucfpos : *mut UConstrainedFieldPosition, category : i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_constrainField(ucfpos : *mut UConstrainedFieldPosition, category : i32, field : i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_getCategory(ucfpos : *const UConstrainedFieldPosition, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_getField(ucfpos : *const UConstrainedFieldPosition, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_getIndexes(ucfpos : *const UConstrainedFieldPosition, pstart : *mut i32, plimit : *mut i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_getInt64IterationContext(ucfpos : *const UConstrainedFieldPosition, ec : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_matchesField(ucfpos : *const UConstrainedFieldPosition, category : i32, field : i32, ec : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_open(ec : *mut UErrorCode) -> *mut UConstrainedFieldPosition);
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_reset(ucfpos : *mut UConstrainedFieldPosition, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_setInt64IterationContext(ucfpos : *mut UConstrainedFieldPosition, context : i64, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucfpos_setState(ucfpos : *mut UConstrainedFieldPosition, category : i32, field : i32, start : i32, limit : i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_cbFromUWriteBytes(args : *mut UConverterFromUnicodeArgs, source : ::windows_sys::core::PCSTR, length : i32, offsetindex : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_cbFromUWriteSub(args : *mut UConverterFromUnicodeArgs, offsetindex : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_cbFromUWriteUChars(args : *mut UConverterFromUnicodeArgs, source : *const *const u16, sourcelimit : *const u16, offsetindex : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_cbToUWriteSub(args : *mut UConverterToUnicodeArgs, offsetindex : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_cbToUWriteUChars(args : *mut UConverterToUnicodeArgs, source : *const u16, length : i32, offsetindex : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_close(converter : *mut UConverter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_compareNames(name1 : ::windows_sys::core::PCSTR, name2 : ::windows_sys::core::PCSTR) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_convert(toconvertername : ::windows_sys::core::PCSTR, fromconvertername : ::windows_sys::core::PCSTR, target : ::windows_sys::core::PCSTR, targetcapacity : i32, source : ::windows_sys::core::PCSTR, sourcelength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_convertEx(targetcnv : *mut UConverter, sourcecnv : *mut UConverter, target : *mut *mut i8, targetlimit : ::windows_sys::core::PCSTR, source : *const *const i8, sourcelimit : ::windows_sys::core::PCSTR, pivotstart : *mut u16, pivotsource : *mut *mut u16, pivottarget : *mut *mut u16, pivotlimit : *const u16, reset : i8, flush : i8, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_countAliases(alias : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> u16);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_countStandards() -> u16);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_detectUnicodeSignature(source : ::windows_sys::core::PCSTR, sourcelength : i32, signaturelength : *mut i32, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_fixFileSeparator(cnv : *const UConverter, source : *mut u16, sourcelen : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_flushCache() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_fromAlgorithmic(cnv : *mut UConverter, algorithmictype : UConverterType, target : ::windows_sys::core::PCSTR, targetcapacity : i32, source : ::windows_sys::core::PCSTR, sourcelength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_fromUChars(cnv : *mut UConverter, dest : ::windows_sys::core::PCSTR, destcapacity : i32, src : *const u16, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_fromUCountPending(cnv : *const UConverter, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_fromUnicode(converter : *mut UConverter, target : *mut *mut i8, targetlimit : ::windows_sys::core::PCSTR, source : *const *const u16, sourcelimit : *const u16, offsets : *mut i32, flush : i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getAlias(alias : ::windows_sys::core::PCSTR, n : u16, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getAliases(alias : ::windows_sys::core::PCSTR, aliases : *const *const i8, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getAvailableName(n : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getCCSID(converter : *const UConverter, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getCanonicalName(alias : ::windows_sys::core::PCSTR, standard : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getDefaultName() -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getDisplayName(converter : *const UConverter, displaylocale : ::windows_sys::core::PCSTR, displayname : *mut u16, displaynamecapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getFromUCallBack(converter : *const UConverter, action : *mut UConverterFromUCallback, context : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getInvalidChars(converter : *const UConverter, errbytes : ::windows_sys::core::PCSTR, len : *mut i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getInvalidUChars(converter : *const UConverter, erruchars : *mut u16, len : *mut i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getMaxCharSize(converter : *const UConverter) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getMinCharSize(converter : *const UConverter) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getName(converter : *const UConverter, err : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getNextUChar(converter : *mut UConverter, source : *const *const i8, sourcelimit : ::windows_sys::core::PCSTR, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getPlatform(converter : *const UConverter, err : *mut UErrorCode) -> UConverterPlatform);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getStandard(n : u16, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getStandardName(name : ::windows_sys::core::PCSTR, standard : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getStarters(converter : *const UConverter, starters : *mut i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getSubstChars(converter : *const UConverter, subchars : ::windows_sys::core::PCSTR, len : *mut i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getToUCallBack(converter : *const UConverter, action : *mut UConverterToUCallback, context : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getType(converter : *const UConverter) -> UConverterType);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_getUnicodeSet(cnv : *const UConverter, setfillin : *mut USet, whichset : UConverterUnicodeSet, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_isAmbiguous(cnv : *const UConverter) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_isFixedWidth(cnv : *mut UConverter, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_open(convertername : ::windows_sys::core::PCSTR, err : *mut UErrorCode) -> *mut UConverter);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_openAllNames(perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_openCCSID(codepage : i32, platform : UConverterPlatform, err : *mut UErrorCode) -> *mut UConverter);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_openPackage(packagename : ::windows_sys::core::PCSTR, convertername : ::windows_sys::core::PCSTR, err : *mut UErrorCode) -> *mut UConverter);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_openStandardNames(convname : ::windows_sys::core::PCSTR, standard : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_openU(name : *const u16, err : *mut UErrorCode) -> *mut UConverter);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_reset(converter : *mut UConverter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_resetFromUnicode(converter : *mut UConverter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_resetToUnicode(converter : *mut UConverter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_safeClone(cnv : *const UConverter, stackbuffer : *mut ::core::ffi::c_void, pbuffersize : *mut i32, status : *mut UErrorCode) -> *mut UConverter);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setDefaultName(name : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setFallback(cnv : *mut UConverter, usesfallback : i8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setFromUCallBack(converter : *mut UConverter, newaction : UConverterFromUCallback, newcontext : *const ::core::ffi::c_void, oldaction : *mut UConverterFromUCallback, oldcontext : *const *const ::core::ffi::c_void, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setSubstChars(converter : *mut UConverter, subchars : ::windows_sys::core::PCSTR, len : i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setSubstString(cnv : *mut UConverter, s : *const u16, length : i32, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_setToUCallBack(converter : *mut UConverter, newaction : UConverterToUCallback, newcontext : *const ::core::ffi::c_void, oldaction : *mut UConverterToUCallback, oldcontext : *const *const ::core::ffi::c_void, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_toAlgorithmic(algorithmictype : UConverterType, cnv : *mut UConverter, target : ::windows_sys::core::PCSTR, targetcapacity : i32, source : ::windows_sys::core::PCSTR, sourcelength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_toUChars(cnv : *mut UConverter, dest : *mut u16, destcapacity : i32, src : ::windows_sys::core::PCSTR, srclength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_toUCountPending(cnv : *const UConverter, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_toUnicode(converter : *mut UConverter, target : *mut *mut u16, targetlimit : *const u16, source : *const *const i8, sourcelimit : ::windows_sys::core::PCSTR, offsets : *mut i32, flush : i8, err : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnv_usesFallback(cnv : *const UConverter) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_close(sel : *mut UConverterSelector) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_open(converterlist : *const *const i8, converterlistsize : i32, excludedcodepoints : *const USet, whichset : UConverterUnicodeSet, status : *mut UErrorCode) -> *mut UConverterSelector);
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_openFromSerialized(buffer : *const ::core::ffi::c_void, length : i32, status : *mut UErrorCode) -> *mut UConverterSelector);
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_selectForString(sel : *const UConverterSelector, s : *const u16, length : i32, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_selectForUTF8(sel : *const UConverterSelector, s : ::windows_sys::core::PCSTR, length : i32, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucnvsel_serialize(sel : *const UConverterSelector, buffer : *mut ::core::ffi::c_void, buffercapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_cloneBinary(coll : *const UCollator, buffer : *mut u8, capacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_close(coll : *mut UCollator) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_closeElements(elems : *mut UCollationElements) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_equal(coll : *const UCollator, source : *const u16, sourcelength : i32, target : *const u16, targetlength : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getAttribute(coll : *const UCollator, attr : UColAttribute, status : *mut UErrorCode) -> UColAttributeValue);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getAvailable(localeindex : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getBound(source : *const u8, sourcelength : i32, boundtype : UColBoundMode, nooflevels : u32, result : *mut u8, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getContractionsAndExpansions(coll : *const UCollator, contractions : *mut USet, expansions : *mut USet, addprefixes : i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getDisplayName(objloc : ::windows_sys::core::PCSTR, disploc : ::windows_sys::core::PCSTR, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getEquivalentReorderCodes(reordercode : i32, dest : *mut i32, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getFunctionalEquivalent(result : ::windows_sys::core::PCSTR, resultcapacity : i32, keyword : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, isavailable : *mut i8, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getKeywordValues(keyword : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getKeywordValuesForLocale(key : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, commonlyused : i8, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getKeywords(status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getLocaleByType(coll : *const UCollator, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getMaxExpansion(elems : *const UCollationElements, order : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getMaxVariable(coll : *const UCollator) -> UColReorderCode);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getOffset(elems : *const UCollationElements) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getReorderCodes(coll : *const UCollator, dest : *mut i32, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getRules(coll : *const UCollator, length : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getRulesEx(coll : *const UCollator, delta : UColRuleOption, buffer : *mut u16, bufferlen : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getSortKey(coll : *const UCollator, source : *const u16, sourcelength : i32, result : *mut u8, resultlength : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getStrength(coll : *const UCollator) -> UColAttributeValue);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getTailoredSet(coll : *const UCollator, status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getUCAVersion(coll : *const UCollator, info : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getVariableTop(coll : *const UCollator, status : *mut UErrorCode) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_getVersion(coll : *const UCollator, info : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_greater(coll : *const UCollator, source : *const u16, sourcelength : i32, target : *const u16, targetlength : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_greaterOrEqual(coll : *const UCollator, source : *const u16, sourcelength : i32, target : *const u16, targetlength : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_keyHashCode(key : *const u8, length : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_mergeSortkeys(src1 : *const u8, src1length : i32, src2 : *const u8, src2length : i32, dest : *mut u8, destcapacity : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_next(elems : *mut UCollationElements, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_nextSortKeyPart(coll : *const UCollator, iter : *mut UCharIterator, state : *mut u32, dest : *mut u8, count : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_open(loc : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UCollator);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_openAvailableLocales(status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_openBinary(bin : *const u8, length : i32, base : *const UCollator, status : *mut UErrorCode) -> *mut UCollator);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_openElements(coll : *const UCollator, text : *const u16, textlength : i32, status : *mut UErrorCode) -> *mut UCollationElements);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_openRules(rules : *const u16, ruleslength : i32, normalizationmode : UColAttributeValue, strength : UColAttributeValue, parseerror : *mut UParseError, status : *mut UErrorCode) -> *mut UCollator);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_previous(elems : *mut UCollationElements, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_primaryOrder(order : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_reset(elems : *mut UCollationElements) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_safeClone(coll : *const UCollator, stackbuffer : *mut ::core::ffi::c_void, pbuffersize : *mut i32, status : *mut UErrorCode) -> *mut UCollator);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_secondaryOrder(order : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setAttribute(coll : *mut UCollator, attr : UColAttribute, value : UColAttributeValue, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setMaxVariable(coll : *mut UCollator, group : UColReorderCode, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setOffset(elems : *mut UCollationElements, offset : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setReorderCodes(coll : *mut UCollator, reordercodes : *const i32, reordercodeslength : i32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setStrength(coll : *mut UCollator, strength : UColAttributeValue) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_setText(elems : *mut UCollationElements, text : *const u16, textlength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucol_strcoll(coll : *const UCollator, source : *const u16, sourcelength : i32, target : *const u16, targetlength : i32) -> UCollationResult);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_strcollIter(coll : *const UCollator, siter : *mut UCharIterator, titer : *mut UCharIterator, status : *mut UErrorCode) -> UCollationResult);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_strcollUTF8(coll : *const UCollator, source : ::windows_sys::core::PCSTR, sourcelength : i32, target : ::windows_sys::core::PCSTR, targetlength : i32, status : *mut UErrorCode) -> UCollationResult);
::windows_targets::link!("icu.dll" "cdecl" fn ucol_tertiaryOrder(order : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucpmap_get(map : *const UCPMap, c : i32) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ucpmap_getRange(map : *const UCPMap, start : i32, option : UCPMapRangeOption, surrogatevalue : u32, filter : *mut UCPMapValueFilter, context : *const ::core::ffi::c_void, pvalue : *mut u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_close(trie : *mut UCPTrie) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_get(trie : *const UCPTrie, c : i32) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_getRange(trie : *const UCPTrie, start : i32, option : UCPMapRangeOption, surrogatevalue : u32, filter : *mut UCPMapValueFilter, context : *const ::core::ffi::c_void, pvalue : *mut u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_getType(trie : *const UCPTrie) -> UCPTrieType);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_getValueWidth(trie : *const UCPTrie) -> UCPTrieValueWidth);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_internalSmallIndex(trie : *const UCPTrie, c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_internalSmallU8Index(trie : *const UCPTrie, lt1 : i32, t2 : u8, t3 : u8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_internalU8PrevIndex(trie : *const UCPTrie, c : i32, start : *const u8, src : *const u8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_openFromBinary(r#type : UCPTrieType, valuewidth : UCPTrieValueWidth, data : *const ::core::ffi::c_void, length : i32, pactuallength : *mut i32, perrorcode : *mut UErrorCode) -> *mut UCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn ucptrie_toBinary(trie : *const UCPTrie, data : *mut ::core::ffi::c_void, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_close(ucsd : *mut UCharsetDetector) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_detect(ucsd : *mut UCharsetDetector, status : *mut UErrorCode) -> *mut UCharsetMatch);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_detectAll(ucsd : *mut UCharsetDetector, matchesfound : *mut i32, status : *mut UErrorCode) -> *mut *mut UCharsetMatch);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_enableInputFilter(ucsd : *mut UCharsetDetector, filter : i8) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_getAllDetectableCharsets(ucsd : *const UCharsetDetector, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_getConfidence(ucsm : *const UCharsetMatch, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_getLanguage(ucsm : *const UCharsetMatch, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_getName(ucsm : *const UCharsetMatch, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_getUChars(ucsm : *const UCharsetMatch, buf : *mut u16, cap : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_isInputFilterEnabled(ucsd : *const UCharsetDetector) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_open(status : *mut UErrorCode) -> *mut UCharsetDetector);
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_setDeclaredEncoding(ucsd : *mut UCharsetDetector, encoding : ::windows_sys::core::PCSTR, length : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucsdet_setText(ucsd : *mut UCharsetDetector, textin : ::windows_sys::core::PCSTR, len : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_countCurrencies(locale : ::windows_sys::core::PCSTR, date : f64, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_forLocale(locale : ::windows_sys::core::PCSTR, buff : *mut u16, buffcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_forLocaleAndDate(locale : ::windows_sys::core::PCSTR, date : f64, index : i32, buff : *mut u16, buffcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getDefaultFractionDigits(currency : *const u16, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getDefaultFractionDigitsForUsage(currency : *const u16, usage : UCurrencyUsage, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getKeywordValuesForLocale(key : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, commonlyused : i8, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getName(currency : *const u16, locale : ::windows_sys::core::PCSTR, namestyle : UCurrNameStyle, ischoiceformat : *mut i8, len : *mut i32, ec : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getNumericCode(currency : *const u16) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getPluralName(currency : *const u16, locale : ::windows_sys::core::PCSTR, ischoiceformat : *mut i8, pluralcount : ::windows_sys::core::PCSTR, len : *mut i32, ec : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getRoundingIncrement(currency : *const u16, ec : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_getRoundingIncrementForUsage(currency : *const u16, usage : UCurrencyUsage, ec : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_isAvailable(isocode : *const u16, from : f64, to : f64, errorcode : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_openISOCurrencies(currtype : u32, perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_register(isocode : *const u16, locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ucurr_unregister(key : *mut ::core::ffi::c_void, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn udat_adoptNumberFormat(fmt : *mut *mut ::core::ffi::c_void, numberformattoadopt : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_adoptNumberFormatForFields(fmt : *mut *mut ::core::ffi::c_void, fields : *const u16, numberformattoset : *mut *mut ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_applyPattern(format : *mut *mut ::core::ffi::c_void, localized : i8, pattern : *const u16, patternlength : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_clone(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udat_close(format : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_countSymbols(fmt : *const *const ::core::ffi::c_void, r#type : UDateFormatSymbolType) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_format(format : *const *const ::core::ffi::c_void, datetoformat : f64, result : *mut u16, resultlength : i32, position : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_formatCalendar(format : *const *const ::core::ffi::c_void, calendar : *mut *mut ::core::ffi::c_void, result : *mut u16, capacity : i32, position : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_formatCalendarForFields(format : *const *const ::core::ffi::c_void, calendar : *mut *mut ::core::ffi::c_void, result : *mut u16, capacity : i32, fpositer : *mut UFieldPositionIterator, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_formatForFields(format : *const *const ::core::ffi::c_void, datetoformat : f64, result : *mut u16, resultlength : i32, fpositer : *mut UFieldPositionIterator, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_get2DigitYearStart(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getAvailable(localeindex : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getBooleanAttribute(fmt : *const *const ::core::ffi::c_void, attr : UDateFormatBooleanAttribute, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getCalendar(fmt : *const *const ::core::ffi::c_void) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getContext(fmt : *const *const ::core::ffi::c_void, r#type : UDisplayContextType, status : *mut UErrorCode) -> UDisplayContext);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getLocaleByType(fmt : *const *const ::core::ffi::c_void, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getNumberFormat(fmt : *const *const ::core::ffi::c_void) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getNumberFormatForField(fmt : *const *const ::core::ffi::c_void, field : u16) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udat_getSymbols(fmt : *const *const ::core::ffi::c_void, r#type : UDateFormatSymbolType, symbolindex : i32, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udat_isLenient(fmt : *const *const ::core::ffi::c_void) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn udat_open(timestyle : UDateFormatStyle, datestyle : UDateFormatStyle, locale : ::windows_sys::core::PCSTR, tzid : *const u16, tzidlength : i32, pattern : *const u16, patternlength : i32, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udat_parse(format : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn udat_parseCalendar(format : *const *const ::core::ffi::c_void, calendar : *mut *mut ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_set2DigitYearStart(fmt : *mut *mut ::core::ffi::c_void, d : f64, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setBooleanAttribute(fmt : *mut *mut ::core::ffi::c_void, attr : UDateFormatBooleanAttribute, newvalue : i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setCalendar(fmt : *mut *mut ::core::ffi::c_void, calendartoset : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setContext(fmt : *mut *mut ::core::ffi::c_void, value : UDisplayContext, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setLenient(fmt : *mut *mut ::core::ffi::c_void, islenient : i8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setNumberFormat(fmt : *mut *mut ::core::ffi::c_void, numberformattoset : *const *const ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_setSymbols(format : *mut *mut ::core::ffi::c_void, r#type : UDateFormatSymbolType, symbolindex : i32, value : *mut u16, valuelength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udat_toCalendarDateField(field : UDateFormatField) -> UCalendarDateFields);
::windows_targets::link!("icu.dll" "cdecl" fn udat_toPattern(fmt : *const *const ::core::ffi::c_void, localized : i8, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_addPattern(dtpg : *mut *mut ::core::ffi::c_void, pattern : *const u16, patternlength : i32, r#override : i8, conflictingpattern : *mut u16, capacity : i32, plength : *mut i32, perrorcode : *mut UErrorCode) -> UDateTimePatternConflict);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_clone(dtpg : *const *const ::core::ffi::c_void, perrorcode : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_close(dtpg : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getAppendItemFormat(dtpg : *const *const ::core::ffi::c_void, field : UDateTimePatternField, plength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getAppendItemName(dtpg : *const *const ::core::ffi::c_void, field : UDateTimePatternField, plength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getBaseSkeleton(unuseddtpg : *mut *mut ::core::ffi::c_void, pattern : *const u16, length : i32, baseskeleton : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getBestPattern(dtpg : *mut *mut ::core::ffi::c_void, skeleton : *const u16, length : i32, bestpattern : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getBestPatternWithOptions(dtpg : *mut *mut ::core::ffi::c_void, skeleton : *const u16, length : i32, options : UDateTimePatternMatchOptions, bestpattern : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getDateTimeFormat(dtpg : *const *const ::core::ffi::c_void, plength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getDecimal(dtpg : *const *const ::core::ffi::c_void, plength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getFieldDisplayName(dtpg : *const *const ::core::ffi::c_void, field : UDateTimePatternField, width : UDateTimePGDisplayWidth, fieldname : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getPatternForSkeleton(dtpg : *const *const ::core::ffi::c_void, skeleton : *const u16, skeletonlength : i32, plength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_getSkeleton(unuseddtpg : *mut *mut ::core::ffi::c_void, pattern : *const u16, length : i32, skeleton : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_open(locale : ::windows_sys::core::PCSTR, perrorcode : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_openBaseSkeletons(dtpg : *const *const ::core::ffi::c_void, perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_openEmpty(perrorcode : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_openSkeletons(dtpg : *const *const ::core::ffi::c_void, perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_replaceFieldTypes(dtpg : *mut *mut ::core::ffi::c_void, pattern : *const u16, patternlength : i32, skeleton : *const u16, skeletonlength : i32, dest : *mut u16, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_replaceFieldTypesWithOptions(dtpg : *mut *mut ::core::ffi::c_void, pattern : *const u16, patternlength : i32, skeleton : *const u16, skeletonlength : i32, options : UDateTimePatternMatchOptions, dest : *mut u16, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_setAppendItemFormat(dtpg : *mut *mut ::core::ffi::c_void, field : UDateTimePatternField, value : *const u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_setAppendItemName(dtpg : *mut *mut ::core::ffi::c_void, field : UDateTimePatternField, value : *const u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_setDateTimeFormat(dtpg : *const *const ::core::ffi::c_void, dtformat : *const u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udatpg_setDecimal(dtpg : *mut *mut ::core::ffi::c_void, decimal : *const u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_close(formatter : *mut UDateIntervalFormat) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_closeResult(uresult : *mut UFormattedDateInterval) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_format(formatter : *const UDateIntervalFormat, fromdate : f64, todate : f64, result : *mut u16, resultcapacity : i32, position : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_open(locale : ::windows_sys::core::PCSTR, skeleton : *const u16, skeletonlength : i32, tzid : *const u16, tzidlength : i32, status : *mut UErrorCode) -> *mut UDateIntervalFormat);
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_openResult(ec : *mut UErrorCode) -> *mut UFormattedDateInterval);
::windows_targets::link!("icu.dll" "cdecl" fn udtitvfmt_resultAsValue(uresult : *const UFormattedDateInterval, ec : *mut UErrorCode) -> *mut UFormattedValue);
::windows_targets::link!("icu.dll" "cdecl" fn uenum_close(en : *mut UEnumeration) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uenum_count(en : *mut UEnumeration, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uenum_next(en : *mut UEnumeration, resultlength : *mut i32, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uenum_openCharStringsEnumeration(strings : *const *const i8, count : i32, ec : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uenum_openUCharStringsEnumeration(strings : *const *const u16, count : i32, ec : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uenum_reset(en : *mut UEnumeration, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uenum_unext(en : *mut UEnumeration, resultlength : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ufieldpositer_close(fpositer : *mut UFieldPositionIterator) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ufieldpositer_next(fpositer : *mut UFieldPositionIterator, beginindex : *mut i32, endindex : *mut i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ufieldpositer_open(status : *mut UErrorCode) -> *mut UFieldPositionIterator);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_close(fmt : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getArrayItemByIndex(fmt : *mut *mut ::core::ffi::c_void, n : i32, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getArrayLength(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getDate(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getDecNumChars(fmt : *mut *mut ::core::ffi::c_void, len : *mut i32, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getDouble(fmt : *mut *mut ::core::ffi::c_void, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getInt64(fmt : *mut *mut ::core::ffi::c_void, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getLong(fmt : *mut *mut ::core::ffi::c_void, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getObject(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getType(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> UFormattableType);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_getUChars(fmt : *mut *mut ::core::ffi::c_void, len : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_isNumeric(fmt : *const *const ::core::ffi::c_void) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ufmt_open(status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn ufmtval_getString(ufmtval : *const UFormattedValue, plength : *mut i32, ec : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ufmtval_nextPosition(ufmtval : *const UFormattedValue, ucfpos : *mut UConstrainedFieldPosition, ec : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ugender_getInstance(locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UGenderInfo);
::windows_targets::link!("icu.dll" "cdecl" fn ugender_getListGender(genderinfo : *const UGenderInfo, genders : *const UGender, size : i32, status : *mut UErrorCode) -> UGender);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_close(idna : *mut UIDNA) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uidna_labelToASCII(idna : *const UIDNA, label : *const u16, length : i32, dest : *mut u16, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_labelToASCII_UTF8(idna : *const UIDNA, label : ::windows_sys::core::PCSTR, length : i32, dest : ::windows_sys::core::PCSTR, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_labelToUnicode(idna : *const UIDNA, label : *const u16, length : i32, dest : *mut u16, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_labelToUnicodeUTF8(idna : *const UIDNA, label : ::windows_sys::core::PCSTR, length : i32, dest : ::windows_sys::core::PCSTR, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_nameToASCII(idna : *const UIDNA, name : *const u16, length : i32, dest : *mut u16, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_nameToASCII_UTF8(idna : *const UIDNA, name : ::windows_sys::core::PCSTR, length : i32, dest : ::windows_sys::core::PCSTR, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_nameToUnicode(idna : *const UIDNA, name : *const u16, length : i32, dest : *mut u16, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_nameToUnicodeUTF8(idna : *const UIDNA, name : ::windows_sys::core::PCSTR, length : i32, dest : ::windows_sys::core::PCSTR, capacity : i32, pinfo : *mut UIDNAInfo, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uidna_openUTS46(options : u32, perrorcode : *mut UErrorCode) -> *mut UIDNA);
::windows_targets::link!("icu.dll" "cdecl" fn uiter_current32(iter : *mut UCharIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uiter_getState(iter : *const UCharIterator) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn uiter_next32(iter : *mut UCharIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uiter_previous32(iter : *mut UCharIterator) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uiter_setState(iter : *mut UCharIterator, state : u32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uiter_setString(iter : *mut UCharIterator, s : *const u16, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uiter_setUTF16BE(iter : *mut UCharIterator, s : ::windows_sys::core::PCSTR, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uiter_setUTF8(iter : *mut UCharIterator, s : ::windows_sys::core::PCSTR, length : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uldn_close(ldn : *mut ULocaleDisplayNames) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uldn_getContext(ldn : *const ULocaleDisplayNames, r#type : UDisplayContextType, perrorcode : *mut UErrorCode) -> UDisplayContext);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_getDialectHandling(ldn : *const ULocaleDisplayNames) -> UDialectHandling);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_getLocale(ldn : *const ULocaleDisplayNames) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_keyDisplayName(ldn : *const ULocaleDisplayNames, key : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_keyValueDisplayName(ldn : *const ULocaleDisplayNames, key : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_languageDisplayName(ldn : *const ULocaleDisplayNames, lang : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_localeDisplayName(ldn : *const ULocaleDisplayNames, locale : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_open(locale : ::windows_sys::core::PCSTR, dialecthandling : UDialectHandling, perrorcode : *mut UErrorCode) -> *mut ULocaleDisplayNames);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_openForContext(locale : ::windows_sys::core::PCSTR, contexts : *mut UDisplayContext, length : i32, perrorcode : *mut UErrorCode) -> *mut ULocaleDisplayNames);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_regionDisplayName(ldn : *const ULocaleDisplayNames, region : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_scriptCodeDisplayName(ldn : *const ULocaleDisplayNames, scriptcode : UScriptCode, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_scriptDisplayName(ldn : *const ULocaleDisplayNames, script : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uldn_variantDisplayName(ldn : *const ULocaleDisplayNames, variant : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_close(listfmt : *mut UListFormatter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_closeResult(uresult : *mut UFormattedList) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_format(listfmt : *const UListFormatter, strings : *const *const u16, stringlengths : *const i32, stringcount : i32, result : *mut u16, resultcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_formatStringsToResult(listfmt : *const UListFormatter, strings : *const *const u16, stringlengths : *const i32, stringcount : i32, uresult : *mut UFormattedList, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_open(locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UListFormatter);
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_openForType(locale : ::windows_sys::core::PCSTR, r#type : UListFormatterType, width : UListFormatterWidth, status : *mut UErrorCode) -> *mut UListFormatter);
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_openResult(ec : *mut UErrorCode) -> *mut UFormattedList);
::windows_targets::link!("icu.dll" "cdecl" fn ulistfmt_resultAsValue(uresult : *const UFormattedList, ec : *mut UErrorCode) -> *mut UFormattedValue);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_acceptLanguage(result : ::windows_sys::core::PCSTR, resultavailable : i32, outresult : *mut UAcceptResult, acceptlist : *const *const i8, acceptlistcount : i32, availablelocales : *mut UEnumeration, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_acceptLanguageFromHTTP(result : ::windows_sys::core::PCSTR, resultavailable : i32, outresult : *mut UAcceptResult, httpacceptlanguage : ::windows_sys::core::PCSTR, availablelocales : *mut UEnumeration, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_addLikelySubtags(localeid : ::windows_sys::core::PCSTR, maximizedlocaleid : ::windows_sys::core::PCSTR, maximizedlocaleidcapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_canonicalize(localeid : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, namecapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_forLanguageTag(langtag : ::windows_sys::core::PCSTR, localeid : ::windows_sys::core::PCSTR, localeidcapacity : i32, parsedlength : *mut i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getAvailable(n : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getBaseName(localeid : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, namecapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getCharacterOrientation(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> ULayoutType);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getCountry(localeid : ::windows_sys::core::PCSTR, country : ::windows_sys::core::PCSTR, countrycapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDefault() -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayCountry(locale : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, country : *mut u16, countrycapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayKeyword(keyword : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, dest : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayKeywordValue(locale : ::windows_sys::core::PCSTR, keyword : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, dest : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayLanguage(locale : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, language : *mut u16, languagecapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayName(localeid : ::windows_sys::core::PCSTR, inlocaleid : ::windows_sys::core::PCSTR, result : *mut u16, maxresultsize : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayScript(locale : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, script : *mut u16, scriptcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getDisplayVariant(locale : ::windows_sys::core::PCSTR, displaylocale : ::windows_sys::core::PCSTR, variant : *mut u16, variantcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getISO3Country(localeid : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getISO3Language(localeid : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getISOCountries() -> *mut *mut i8);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getISOLanguages() -> *mut *mut i8);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getKeywordValue(localeid : ::windows_sys::core::PCSTR, keywordname : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PCSTR, buffercapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getLCID(localeid : ::windows_sys::core::PCSTR) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getLanguage(localeid : ::windows_sys::core::PCSTR, language : ::windows_sys::core::PCSTR, languagecapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getLineOrientation(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> ULayoutType);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getLocaleForLCID(hostid : u32, locale : ::windows_sys::core::PCSTR, localecapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getName(localeid : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, namecapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getParent(localeid : ::windows_sys::core::PCSTR, parent : ::windows_sys::core::PCSTR, parentcapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getScript(localeid : ::windows_sys::core::PCSTR, script : ::windows_sys::core::PCSTR, scriptcapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_getVariant(localeid : ::windows_sys::core::PCSTR, variant : ::windows_sys::core::PCSTR, variantcapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_isRightToLeft(locale : ::windows_sys::core::PCSTR) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_minimizeSubtags(localeid : ::windows_sys::core::PCSTR, minimizedlocaleid : ::windows_sys::core::PCSTR, minimizedlocaleidcapacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_openAvailableByType(r#type : ULocAvailableType, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_openKeywords(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_setDefault(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uloc_setKeywordValue(keywordname : ::windows_sys::core::PCSTR, keywordvalue : ::windows_sys::core::PCSTR, buffer : ::windows_sys::core::PCSTR, buffercapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_toLanguageTag(localeid : ::windows_sys::core::PCSTR, langtag : ::windows_sys::core::PCSTR, langtagcapacity : i32, strict : i8, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_toLegacyKey(keyword : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_toLegacyType(keyword : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_toUnicodeLocaleKey(keyword : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uloc_toUnicodeLocaleType(keyword : ::windows_sys::core::PCSTR, value : ::windows_sys::core::PCSTR) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_close(uld : *mut ULocaleData) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getCLDRVersion(versionarray : *mut u8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getDelimiter(uld : *mut ULocaleData, r#type : ULocaleDataDelimiterType, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getExemplarSet(uld : *mut ULocaleData, fillin : *mut USet, options : u32, extype : ULocaleDataExemplarSetType, status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getLocaleDisplayPattern(uld : *mut ULocaleData, pattern : *mut u16, patterncapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getLocaleSeparator(uld : *mut ULocaleData, separator : *mut u16, separatorcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getMeasurementSystem(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> UMeasurementSystem);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getNoSubstitute(uld : *mut ULocaleData) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_getPaperSize(localeid : ::windows_sys::core::PCSTR, height : *mut i32, width : *mut i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_open(localeid : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut ULocaleData);
::windows_targets::link!("icu.dll" "cdecl" fn ulocdata_setNoSubstitute(uld : *mut ULocaleData, setting : i8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umsg_applyPattern(fmt : *mut *mut ::core::ffi::c_void, pattern : *const u16, patternlength : i32, parseerror : *mut UParseError, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umsg_autoQuoteApostrophe(pattern : *const u16, patternlength : i32, dest : *mut u16, destcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_clone(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_close(format : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umsg_format(fmt : *const *const ::core::ffi::c_void, result : *mut u16, resultlength : i32, status : *mut UErrorCode, ...) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_getLocale(fmt : *const *const ::core::ffi::c_void) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_open(pattern : *const u16, patternlength : i32, locale : ::windows_sys::core::PCSTR, parseerror : *mut UParseError, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_parse(fmt : *const *const ::core::ffi::c_void, source : *const u16, sourcelength : i32, count : *mut i32, status : *mut UErrorCode, ...) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umsg_setLocale(fmt : *mut *mut ::core::ffi::c_void, locale : ::windows_sys::core::PCSTR) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umsg_toPattern(fmt : *const *const ::core::ffi::c_void, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_vformat(fmt : *const *const ::core::ffi::c_void, result : *mut u16, resultlength : i32, ap : *mut i8, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn umsg_vparse(fmt : *const *const ::core::ffi::c_void, source : *const u16, sourcelength : i32, count : *mut i32, ap : *mut i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_buildImmutable(trie : *mut UMutableCPTrie, r#type : UCPTrieType, valuewidth : UCPTrieValueWidth, perrorcode : *mut UErrorCode) -> *mut UCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_clone(other : *const UMutableCPTrie, perrorcode : *mut UErrorCode) -> *mut UMutableCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_close(trie : *mut UMutableCPTrie) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_fromUCPMap(map : *const UCPMap, perrorcode : *mut UErrorCode) -> *mut UMutableCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_fromUCPTrie(trie : *const UCPTrie, perrorcode : *mut UErrorCode) -> *mut UMutableCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_get(trie : *const UMutableCPTrie, c : i32) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_getRange(trie : *const UMutableCPTrie, start : i32, option : UCPMapRangeOption, surrogatevalue : u32, filter : *mut UCPMapValueFilter, context : *const ::core::ffi::c_void, pvalue : *mut u32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_open(initialvalue : u32, errorvalue : u32, perrorcode : *mut UErrorCode) -> *mut UMutableCPTrie);
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_set(trie : *mut UMutableCPTrie, c : i32, value : u32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn umutablecptrie_setRange(trie : *mut UMutableCPTrie, start : i32, end : i32, value : u32, perrorcode : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_append(norm2 : *const UNormalizer2, first : *mut u16, firstlength : i32, firstcapacity : i32, second : *const u16, secondlength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_close(norm2 : *mut UNormalizer2) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_composePair(norm2 : *const UNormalizer2, a : i32, b : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getCombiningClass(norm2 : *const UNormalizer2, c : i32) -> u8);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getDecomposition(norm2 : *const UNormalizer2, c : i32, decomposition : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getInstance(packagename : ::windows_sys::core::PCSTR, name : ::windows_sys::core::PCSTR, mode : UNormalization2Mode, perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getNFCInstance(perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getNFDInstance(perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getNFKCCasefoldInstance(perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getNFKCInstance(perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getNFKDInstance(perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_getRawDecomposition(norm2 : *const UNormalizer2, c : i32, decomposition : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_hasBoundaryAfter(norm2 : *const UNormalizer2, c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_hasBoundaryBefore(norm2 : *const UNormalizer2, c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_isInert(norm2 : *const UNormalizer2, c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_isNormalized(norm2 : *const UNormalizer2, s : *const u16, length : i32, perrorcode : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_normalize(norm2 : *const UNormalizer2, src : *const u16, length : i32, dest : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_normalizeSecondAndAppend(norm2 : *const UNormalizer2, first : *mut u16, firstlength : i32, firstcapacity : i32, second : *const u16, secondlength : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_openFiltered(norm2 : *const UNormalizer2, filterset : *const USet, perrorcode : *mut UErrorCode) -> *mut UNormalizer2);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_quickCheck(norm2 : *const UNormalizer2, s : *const u16, length : i32, perrorcode : *mut UErrorCode) -> UNormalizationCheckResult);
::windows_targets::link!("icu.dll" "cdecl" fn unorm2_spanQuickCheckYes(norm2 : *const UNormalizer2, s : *const u16, length : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unorm_compare(s1 : *const u16, length1 : i32, s2 : *const u16, length2 : i32, options : u32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_applyPattern(format : *mut *mut ::core::ffi::c_void, localized : i8, pattern : *const u16, patternlength : i32, parseerror : *mut UParseError, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_clone(fmt : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn unum_close(fmt : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_countAvailable() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_format(fmt : *const *const ::core::ffi::c_void, number : i32, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatDecimal(fmt : *const *const ::core::ffi::c_void, number : ::windows_sys::core::PCSTR, length : i32, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatDouble(fmt : *const *const ::core::ffi::c_void, number : f64, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatDoubleCurrency(fmt : *const *const ::core::ffi::c_void, number : f64, currency : *mut u16, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatDoubleForFields(format : *const *const ::core::ffi::c_void, number : f64, result : *mut u16, resultlength : i32, fpositer : *mut UFieldPositionIterator, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatInt64(fmt : *const *const ::core::ffi::c_void, number : i64, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_formatUFormattable(fmt : *const *const ::core::ffi::c_void, number : *const *const ::core::ffi::c_void, result : *mut u16, resultlength : i32, pos : *mut UFieldPosition, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getAttribute(fmt : *const *const ::core::ffi::c_void, attr : UNumberFormatAttribute) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getAvailable(localeindex : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getContext(fmt : *const *const ::core::ffi::c_void, r#type : UDisplayContextType, status : *mut UErrorCode) -> UDisplayContext);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getDoubleAttribute(fmt : *const *const ::core::ffi::c_void, attr : UNumberFormatAttribute) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getLocaleByType(fmt : *const *const ::core::ffi::c_void, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getSymbol(fmt : *const *const ::core::ffi::c_void, symbol : UNumberFormatSymbol, buffer : *mut u16, size : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_getTextAttribute(fmt : *const *const ::core::ffi::c_void, tag : UNumberFormatTextAttribute, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_open(style : UNumberFormatStyle, pattern : *const u16, patternlength : i32, locale : ::windows_sys::core::PCSTR, parseerr : *mut UParseError, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parse(fmt : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parseDecimal(fmt : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, outbuf : ::windows_sys::core::PCSTR, outbuflength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parseDouble(fmt : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parseDoubleCurrency(fmt : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, currency : *mut u16, status : *mut UErrorCode) -> f64);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parseInt64(fmt : *const *const ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn unum_parseToUFormattable(fmt : *const *const ::core::ffi::c_void, result : *mut *mut ::core::ffi::c_void, text : *const u16, textlength : i32, parsepos : *mut i32, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn unum_setAttribute(fmt : *mut *mut ::core::ffi::c_void, attr : UNumberFormatAttribute, newvalue : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_setContext(fmt : *mut *mut ::core::ffi::c_void, value : UDisplayContext, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_setDoubleAttribute(fmt : *mut *mut ::core::ffi::c_void, attr : UNumberFormatAttribute, newvalue : f64) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_setSymbol(fmt : *mut *mut ::core::ffi::c_void, symbol : UNumberFormatSymbol, value : *const u16, length : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_setTextAttribute(fmt : *mut *mut ::core::ffi::c_void, tag : UNumberFormatTextAttribute, newvalue : *const u16, newvaluelength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unum_toPattern(fmt : *const *const ::core::ffi::c_void, ispatternlocalized : i8, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_close(uformatter : *mut UNumberFormatter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_closeResult(uresult : *mut UFormattedNumber) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_formatDecimal(uformatter : *const UNumberFormatter, value : ::windows_sys::core::PCSTR, valuelen : i32, uresult : *mut UFormattedNumber, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_formatDouble(uformatter : *const UNumberFormatter, value : f64, uresult : *mut UFormattedNumber, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_formatInt(uformatter : *const UNumberFormatter, value : i64, uresult : *mut UFormattedNumber, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_openForSkeletonAndLocale(skeleton : *const u16, skeletonlen : i32, locale : ::windows_sys::core::PCSTR, ec : *mut UErrorCode) -> *mut UNumberFormatter);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_openForSkeletonAndLocaleWithError(skeleton : *const u16, skeletonlen : i32, locale : ::windows_sys::core::PCSTR, perror : *mut UParseError, ec : *mut UErrorCode) -> *mut UNumberFormatter);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_openResult(ec : *mut UErrorCode) -> *mut UFormattedNumber);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_resultAsValue(uresult : *const UFormattedNumber, ec : *mut UErrorCode) -> *mut UFormattedValue);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_resultGetAllFieldPositions(uresult : *const UFormattedNumber, ufpositer : *mut UFieldPositionIterator, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumf_resultNextFieldPosition(uresult : *const UFormattedNumber, ufpos : *mut UFieldPosition, ec : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unumf_resultToString(uresult : *const UFormattedNumber, buffer : *mut u16, buffercapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_close(unumsys : *mut UNumberingSystem) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_getDescription(unumsys : *const UNumberingSystem, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_getName(unumsys : *const UNumberingSystem) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_getRadix(unumsys : *const UNumberingSystem) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_isAlgorithmic(unumsys : *const UNumberingSystem) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_open(locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UNumberingSystem);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_openAvailableNames(status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn unumsys_openByName(name : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UNumberingSystem);
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_close(uplrules : *mut UPluralRules) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_getKeywords(uplrules : *const UPluralRules, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_open(locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UPluralRules);
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_openForType(locale : ::windows_sys::core::PCSTR, r#type : UPluralType, status : *mut UErrorCode) -> *mut UPluralRules);
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_select(uplrules : *const UPluralRules, number : f64, keyword : *mut u16, capacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uplrules_selectFormatted(uplrules : *const UPluralRules, number : *const UFormattedNumber, keyword : *mut u16, capacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_appendReplacement(regexp : *mut URegularExpression, replacementtext : *const u16, replacementlength : i32, destbuf : *mut *mut u16, destcapacity : *mut i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_appendReplacementUText(regexp : *mut URegularExpression, replacementtext : *mut UText, dest : *mut UText, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_appendTail(regexp : *mut URegularExpression, destbuf : *mut *mut u16, destcapacity : *mut i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_appendTailUText(regexp : *mut URegularExpression, dest : *mut UText, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_clone(regexp : *const URegularExpression, status : *mut UErrorCode) -> *mut URegularExpression);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_close(regexp : *mut URegularExpression) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_end(regexp : *mut URegularExpression, groupnum : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_end64(regexp : *mut URegularExpression, groupnum : i32, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_find(regexp : *mut URegularExpression, startindex : i32, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_find64(regexp : *mut URegularExpression, startindex : i64, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_findNext(regexp : *mut URegularExpression, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_flags(regexp : *const URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getFindProgressCallback(regexp : *const URegularExpression, callback : *mut URegexFindProgressCallback, context : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getMatchCallback(regexp : *const URegularExpression, callback : *mut URegexMatchCallback, context : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getStackLimit(regexp : *const URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getText(regexp : *mut URegularExpression, textlength : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getTimeLimit(regexp : *const URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_getUText(regexp : *mut URegularExpression, dest : *mut UText, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_group(regexp : *mut URegularExpression, groupnum : i32, dest : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_groupCount(regexp : *mut URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_groupNumberFromCName(regexp : *mut URegularExpression, groupname : ::windows_sys::core::PCSTR, namelength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_groupNumberFromName(regexp : *mut URegularExpression, groupname : *const u16, namelength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_groupUText(regexp : *mut URegularExpression, groupnum : i32, dest : *mut UText, grouplength : *mut i64, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_hasAnchoringBounds(regexp : *const URegularExpression, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_hasTransparentBounds(regexp : *const URegularExpression, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_hitEnd(regexp : *const URegularExpression, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_lookingAt(regexp : *mut URegularExpression, startindex : i32, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_lookingAt64(regexp : *mut URegularExpression, startindex : i64, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_matches(regexp : *mut URegularExpression, startindex : i32, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_matches64(regexp : *mut URegularExpression, startindex : i64, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_open(pattern : *const u16, patternlength : i32, flags : u32, pe : *mut UParseError, status : *mut UErrorCode) -> *mut URegularExpression);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_openC(pattern : ::windows_sys::core::PCSTR, flags : u32, pe : *mut UParseError, status : *mut UErrorCode) -> *mut URegularExpression);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_openUText(pattern : *mut UText, flags : u32, pe : *mut UParseError, status : *mut UErrorCode) -> *mut URegularExpression);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_pattern(regexp : *const URegularExpression, patlength : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_patternUText(regexp : *const URegularExpression, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_refreshUText(regexp : *mut URegularExpression, text : *mut UText, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_regionEnd(regexp : *const URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_regionEnd64(regexp : *const URegularExpression, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_regionStart(regexp : *const URegularExpression, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_regionStart64(regexp : *const URegularExpression, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_replaceAll(regexp : *mut URegularExpression, replacementtext : *const u16, replacementlength : i32, destbuf : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_replaceAllUText(regexp : *mut URegularExpression, replacement : *mut UText, dest : *mut UText, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_replaceFirst(regexp : *mut URegularExpression, replacementtext : *const u16, replacementlength : i32, destbuf : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_replaceFirstUText(regexp : *mut URegularExpression, replacement : *mut UText, dest : *mut UText, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_requireEnd(regexp : *const URegularExpression, status : *mut UErrorCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_reset(regexp : *mut URegularExpression, index : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_reset64(regexp : *mut URegularExpression, index : i64, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setFindProgressCallback(regexp : *mut URegularExpression, callback : URegexFindProgressCallback, context : *const ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setMatchCallback(regexp : *mut URegularExpression, callback : URegexMatchCallback, context : *const ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setRegion(regexp : *mut URegularExpression, regionstart : i32, regionlimit : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setRegion64(regexp : *mut URegularExpression, regionstart : i64, regionlimit : i64, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setRegionAndStart(regexp : *mut URegularExpression, regionstart : i64, regionlimit : i64, startindex : i64, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setStackLimit(regexp : *mut URegularExpression, limit : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setText(regexp : *mut URegularExpression, text : *const u16, textlength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setTimeLimit(regexp : *mut URegularExpression, limit : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_setUText(regexp : *mut URegularExpression, text : *mut UText, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_split(regexp : *mut URegularExpression, destbuf : *mut u16, destcapacity : i32, requiredcapacity : *mut i32, destfields : *mut *mut u16, destfieldscapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_splitUText(regexp : *mut URegularExpression, destfields : *mut *mut UText, destfieldscapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_start(regexp : *mut URegularExpression, groupnum : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_start64(regexp : *mut URegularExpression, groupnum : i32, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn uregex_useAnchoringBounds(regexp : *mut URegularExpression, b : i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregex_useTransparentBounds(regexp : *mut URegularExpression, b : i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uregion_areEqual(uregion : *const URegion, otherregion : *const URegion) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_contains(uregion : *const URegion, otherregion : *const URegion) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getAvailable(r#type : URegionType, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getContainedRegions(uregion : *const URegion, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getContainedRegionsOfType(uregion : *const URegion, r#type : URegionType, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getContainingRegion(uregion : *const URegion) -> *mut URegion);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getContainingRegionOfType(uregion : *const URegion, r#type : URegionType) -> *mut URegion);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getNumericCode(uregion : *const URegion) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getPreferredValues(uregion : *const URegion, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getRegionCode(uregion : *const URegion) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getRegionFromCode(regioncode : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut URegion);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getRegionFromNumericCode(code : i32, status : *mut UErrorCode) -> *mut URegion);
::windows_targets::link!("icu.dll" "cdecl" fn uregion_getType(uregion : *const URegion) -> URegionType);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_close(reldatefmt : *mut URelativeDateTimeFormatter) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_closeResult(ufrdt : *mut UFormattedRelativeDateTime) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_combineDateAndTime(reldatefmt : *const URelativeDateTimeFormatter, relativedatestring : *const u16, relativedatestringlen : i32, timestring : *const u16, timestringlen : i32, result : *mut u16, resultcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_format(reldatefmt : *const URelativeDateTimeFormatter, offset : f64, unit : URelativeDateTimeUnit, result : *mut u16, resultcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_formatNumeric(reldatefmt : *const URelativeDateTimeFormatter, offset : f64, unit : URelativeDateTimeUnit, result : *mut u16, resultcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_formatNumericToResult(reldatefmt : *const URelativeDateTimeFormatter, offset : f64, unit : URelativeDateTimeUnit, result : *mut UFormattedRelativeDateTime, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_formatToResult(reldatefmt : *const URelativeDateTimeFormatter, offset : f64, unit : URelativeDateTimeUnit, result : *mut UFormattedRelativeDateTime, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_open(locale : ::windows_sys::core::PCSTR, nftoadopt : *mut *mut ::core::ffi::c_void, width : UDateRelativeDateTimeFormatterStyle, capitalizationcontext : UDisplayContext, status : *mut UErrorCode) -> *mut URelativeDateTimeFormatter);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_openResult(ec : *mut UErrorCode) -> *mut UFormattedRelativeDateTime);
::windows_targets::link!("icu.dll" "cdecl" fn ureldatefmt_resultAsValue(ufrdt : *const UFormattedRelativeDateTime, ec : *mut UErrorCode) -> *mut UFormattedValue);
::windows_targets::link!("icu.dll" "cdecl" fn ures_close(resourcebundle : *mut UResourceBundle) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ures_getBinary(resourcebundle : *const UResourceBundle, len : *mut i32, status : *mut UErrorCode) -> *mut u8);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getByIndex(resourcebundle : *const UResourceBundle, indexr : i32, fillin : *mut UResourceBundle, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getByKey(resourcebundle : *const UResourceBundle, key : ::windows_sys::core::PCSTR, fillin : *mut UResourceBundle, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getInt(resourcebundle : *const UResourceBundle, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getIntVector(resourcebundle : *const UResourceBundle, len : *mut i32, status : *mut UErrorCode) -> *mut i32);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getKey(resourcebundle : *const UResourceBundle) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getLocaleByType(resourcebundle : *const UResourceBundle, r#type : ULocDataLocaleType, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getNextResource(resourcebundle : *mut UResourceBundle, fillin : *mut UResourceBundle, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getNextString(resourcebundle : *mut UResourceBundle, len : *mut i32, key : *const *const i8, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getSize(resourcebundle : *const UResourceBundle) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getString(resourcebundle : *const UResourceBundle, len : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getStringByIndex(resourcebundle : *const UResourceBundle, indexs : i32, len : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getStringByKey(resb : *const UResourceBundle, key : ::windows_sys::core::PCSTR, len : *mut i32, status : *mut UErrorCode) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getType(resourcebundle : *const UResourceBundle) -> UResType);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getUInt(resourcebundle : *const UResourceBundle, status : *mut UErrorCode) -> u32);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getUTF8String(resb : *const UResourceBundle, dest : ::windows_sys::core::PCSTR, length : *mut i32, forcecopy : i8, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getUTF8StringByIndex(resb : *const UResourceBundle, stringindex : i32, dest : ::windows_sys::core::PCSTR, plength : *mut i32, forcecopy : i8, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getUTF8StringByKey(resb : *const UResourceBundle, key : ::windows_sys::core::PCSTR, dest : ::windows_sys::core::PCSTR, plength : *mut i32, forcecopy : i8, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn ures_getVersion(resb : *const UResourceBundle, versioninfo : *mut u8) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn ures_hasNext(resourcebundle : *const UResourceBundle) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn ures_open(packagename : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_openAvailableLocales(packagename : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn ures_openDirect(packagename : ::windows_sys::core::PCSTR, locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_openU(packagename : *const u16, locale : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UResourceBundle);
::windows_targets::link!("icu.dll" "cdecl" fn ures_resetIterator(resourcebundle : *mut UResourceBundle) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uscript_breaksBetweenLetters(script : UScriptCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getCode(nameorabbrorlocale : ::windows_sys::core::PCSTR, fillin : *mut UScriptCode, capacity : i32, err : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getName(scriptcode : UScriptCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getSampleString(script : UScriptCode, dest : *mut u16, capacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getScript(codepoint : i32, err : *mut UErrorCode) -> UScriptCode);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getScriptExtensions(c : i32, scripts : *mut UScriptCode, capacity : i32, errorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getShortName(scriptcode : UScriptCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_getUsage(script : UScriptCode) -> UScriptUsage);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_hasScript(c : i32, sc : UScriptCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_isCased(script : UScriptCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uscript_isRightToLeft(script : UScriptCode) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_close(searchiter : *mut UStringSearch) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_first(strsrch : *mut UStringSearch, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_following(strsrch : *mut UStringSearch, position : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getAttribute(strsrch : *const UStringSearch, attribute : USearchAttribute) -> USearchAttributeValue);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getBreakIterator(strsrch : *const UStringSearch) -> *mut UBreakIterator);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getCollator(strsrch : *const UStringSearch) -> *mut UCollator);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getMatchedLength(strsrch : *const UStringSearch) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getMatchedStart(strsrch : *const UStringSearch) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getMatchedText(strsrch : *const UStringSearch, result : *mut u16, resultcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getOffset(strsrch : *const UStringSearch) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getPattern(strsrch : *const UStringSearch, length : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_getText(strsrch : *const UStringSearch, length : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_last(strsrch : *mut UStringSearch, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_next(strsrch : *mut UStringSearch, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_open(pattern : *const u16, patternlength : i32, text : *const u16, textlength : i32, locale : ::windows_sys::core::PCSTR, breakiter : *mut UBreakIterator, status : *mut UErrorCode) -> *mut UStringSearch);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_openFromCollator(pattern : *const u16, patternlength : i32, text : *const u16, textlength : i32, collator : *const UCollator, breakiter : *mut UBreakIterator, status : *mut UErrorCode) -> *mut UStringSearch);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_preceding(strsrch : *mut UStringSearch, position : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_previous(strsrch : *mut UStringSearch, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn usearch_reset(strsrch : *mut UStringSearch) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setAttribute(strsrch : *mut UStringSearch, attribute : USearchAttribute, value : USearchAttributeValue, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setBreakIterator(strsrch : *mut UStringSearch, breakiter : *mut UBreakIterator, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setCollator(strsrch : *mut UStringSearch, collator : *const UCollator, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setOffset(strsrch : *mut UStringSearch, position : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setPattern(strsrch : *mut UStringSearch, pattern : *const u16, patternlength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usearch_setText(strsrch : *mut UStringSearch, text : *const u16, textlength : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_add(set : *mut USet, c : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_addAll(set : *mut USet, additionalset : *const USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_addAllCodePoints(set : *mut USet, str : *const u16, strlen : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_addRange(set : *mut USet, start : i32, end : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_addString(set : *mut USet, str : *const u16, strlen : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_applyIntPropertyValue(set : *mut USet, prop : UProperty, value : i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_applyPattern(set : *mut USet, pattern : *const u16, patternlength : i32, options : u32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_applyPropertyAlias(set : *mut USet, prop : *const u16, proplength : i32, value : *const u16, valuelength : i32, ec : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_charAt(set : *const USet, charindex : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_clear(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_clone(set : *const USet) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_cloneAsThawed(set : *const USet) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_close(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_closeOver(set : *mut USet, attributes : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_compact(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_complement(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_complementAll(set : *mut USet, complement : *const USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_contains(set : *const USet, c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsAll(set1 : *const USet, set2 : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsAllCodePoints(set : *const USet, str : *const u16, strlen : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsNone(set1 : *const USet, set2 : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsRange(set : *const USet, start : i32, end : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsSome(set1 : *const USet, set2 : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_containsString(set : *const USet, str : *const u16, strlen : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_equals(set1 : *const USet, set2 : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_freeze(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_getItem(set : *const USet, itemindex : i32, start : *mut i32, end : *mut i32, str : *mut u16, strcapacity : i32, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_getItemCount(set : *const USet) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_getSerializedRange(set : *const USerializedSet, rangeindex : i32, pstart : *mut i32, pend : *mut i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_getSerializedRangeCount(set : *const USerializedSet) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_getSerializedSet(fillset : *mut USerializedSet, src : *const u16, srclength : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_indexOf(set : *const USet, c : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_isEmpty(set : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_isFrozen(set : *const USet) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_open(start : i32, end : i32) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_openEmpty() -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_openPattern(pattern : *const u16, patternlength : i32, ec : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_openPatternOptions(pattern : *const u16, patternlength : i32, options : u32, ec : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uset_remove(set : *mut USet, c : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_removeAll(set : *mut USet, removeset : *const USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_removeAllStrings(set : *mut USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_removeRange(set : *mut USet, start : i32, end : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_removeString(set : *mut USet, str : *const u16, strlen : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_resemblesPattern(pattern : *const u16, patternlength : i32, pos : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_retain(set : *mut USet, start : i32, end : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_retainAll(set : *mut USet, retain : *const USet) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_serialize(set : *const USet, dest : *mut u16, destcapacity : i32, perrorcode : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_serializedContains(set : *const USerializedSet, c : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn uset_set(set : *mut USet, start : i32, end : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_setSerializedToOne(fillset : *mut USerializedSet, c : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uset_size(set : *const USet) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_span(set : *const USet, s : *const u16, length : i32, spancondition : USetSpanCondition) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_spanBack(set : *const USet, s : *const u16, length : i32, spancondition : USetSpanCondition) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_spanBackUTF8(set : *const USet, s : ::windows_sys::core::PCSTR, length : i32, spancondition : USetSpanCondition) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_spanUTF8(set : *const USet, s : ::windows_sys::core::PCSTR, length : i32, spancondition : USetSpanCondition) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uset_toPattern(set : *const USet, result : *mut u16, resultcapacity : i32, escapeunprintable : i8, ec : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_areConfusable(sc : *const USpoofChecker, id1 : *const u16, length1 : i32, id2 : *const u16, length2 : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_areConfusableUTF8(sc : *const USpoofChecker, id1 : ::windows_sys::core::PCSTR, length1 : i32, id2 : ::windows_sys::core::PCSTR, length2 : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_check(sc : *const USpoofChecker, id : *const u16, length : i32, position : *mut i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_check2(sc : *const USpoofChecker, id : *const u16, length : i32, checkresult : *mut USpoofCheckResult, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_check2UTF8(sc : *const USpoofChecker, id : ::windows_sys::core::PCSTR, length : i32, checkresult : *mut USpoofCheckResult, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_checkUTF8(sc : *const USpoofChecker, id : ::windows_sys::core::PCSTR, length : i32, position : *mut i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_clone(sc : *const USpoofChecker, status : *mut UErrorCode) -> *mut USpoofChecker);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_close(sc : *mut USpoofChecker) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_closeCheckResult(checkresult : *mut USpoofCheckResult) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getAllowedChars(sc : *const USpoofChecker, status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getAllowedLocales(sc : *mut USpoofChecker, status : *mut UErrorCode) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getCheckResultChecks(checkresult : *const USpoofCheckResult, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getCheckResultNumerics(checkresult : *const USpoofCheckResult, status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getCheckResultRestrictionLevel(checkresult : *const USpoofCheckResult, status : *mut UErrorCode) -> URestrictionLevel);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getChecks(sc : *const USpoofChecker, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getInclusionSet(status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getRecommendedSet(status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getRestrictionLevel(sc : *const USpoofChecker) -> URestrictionLevel);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getSkeleton(sc : *const USpoofChecker, r#type : u32, id : *const u16, length : i32, dest : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_getSkeletonUTF8(sc : *const USpoofChecker, r#type : u32, id : ::windows_sys::core::PCSTR, length : i32, dest : ::windows_sys::core::PCSTR, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_open(status : *mut UErrorCode) -> *mut USpoofChecker);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_openCheckResult(status : *mut UErrorCode) -> *mut USpoofCheckResult);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_openFromSerialized(data : *const ::core::ffi::c_void, length : i32, pactuallength : *mut i32, perrorcode : *mut UErrorCode) -> *mut USpoofChecker);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_openFromSource(confusables : ::windows_sys::core::PCSTR, confusableslen : i32, confusableswholescript : ::windows_sys::core::PCSTR, confusableswholescriptlen : i32, errtype : *mut i32, pe : *mut UParseError, status : *mut UErrorCode) -> *mut USpoofChecker);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_serialize(sc : *mut USpoofChecker, data : *mut ::core::ffi::c_void, capacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_setAllowedChars(sc : *mut USpoofChecker, chars : *const USet, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_setAllowedLocales(sc : *mut USpoofChecker, localeslist : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_setChecks(sc : *mut USpoofChecker, checks : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn uspoof_setRestrictionLevel(sc : *mut USpoofChecker, restrictionlevel : URestrictionLevel) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usprep_close(profile : *mut UStringPrepProfile) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn usprep_open(path : ::windows_sys::core::PCSTR, filename : ::windows_sys::core::PCSTR, status : *mut UErrorCode) -> *mut UStringPrepProfile);
::windows_targets::link!("icu.dll" "cdecl" fn usprep_openByType(r#type : UStringPrepProfileType, status : *mut UErrorCode) -> *mut UStringPrepProfile);
::windows_targets::link!("icu.dll" "cdecl" fn usprep_prepare(prep : *const UStringPrepProfile, src : *const u16, srclength : i32, dest : *mut u16, destcapacity : i32, options : i32, parseerror : *mut UParseError, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_char32At(ut : *mut UText, nativeindex : i64) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_clone(dest : *mut UText, src : *const UText, deep : i8, readonly : i8, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn utext_close(ut : *mut UText) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn utext_copy(ut : *mut UText, nativestart : i64, nativelimit : i64, destindex : i64, r#move : i8, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utext_current32(ut : *mut UText) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_equals(a : *const UText, b : *const UText) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn utext_extract(ut : *mut UText, nativestart : i64, nativelimit : i64, dest : *mut u16, destcapacity : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_freeze(ut : *mut UText) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utext_getNativeIndex(ut : *const UText) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utext_getPreviousNativeIndex(ut : *mut UText) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utext_hasMetaData(ut : *const UText) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn utext_isLengthExpensive(ut : *const UText) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn utext_isWritable(ut : *const UText) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn utext_moveIndex32(ut : *mut UText, delta : i32) -> i8);
::windows_targets::link!("icu.dll" "cdecl" fn utext_nativeLength(ut : *mut UText) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utext_next32(ut : *mut UText) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_next32From(ut : *mut UText, nativeindex : i64) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_openUChars(ut : *mut UText, s : *const u16, length : i64, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn utext_openUTF8(ut : *mut UText, s : ::windows_sys::core::PCSTR, length : i64, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn utext_previous32(ut : *mut UText) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_previous32From(ut : *mut UText, nativeindex : i64) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_replace(ut : *mut UText, nativestart : i64, nativelimit : i64, replacementtext : *const u16, replacementlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utext_setNativeIndex(ut : *mut UText, nativeindex : i64) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utext_setup(ut : *mut UText, extraspace : i32, status : *mut UErrorCode) -> *mut UText);
::windows_targets::link!("icu.dll" "cdecl" fn utf8_appendCharSafeBody(s : *mut u8, i : i32, length : i32, c : i32, piserror : *mut i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utf8_back1SafeBody(s : *const u8, start : i32, i : i32) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utf8_nextCharSafeBody(s : *const u8, pi : *mut i32, length : i32, c : i32, strict : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utf8_prevCharSafeBody(s : *const u8, start : i32, pi : *mut i32, c : i32, strict : i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utmscale_fromInt64(othertime : i64, timescale : UDateTimeScale, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utmscale_getTimeScaleValue(timescale : UDateTimeScale, value : UTimeScaleValue, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utmscale_toInt64(universaltime : i64, timescale : UDateTimeScale, status : *mut UErrorCode) -> i64);
::windows_targets::link!("icu.dll" "cdecl" fn utrace_format(outbuf : ::windows_sys::core::PCSTR, capacity : i32, indent : i32, fmt : ::windows_sys::core::PCSTR, ...) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utrace_functionName(fnnumber : i32) -> ::windows_sys::core::PCSTR);
::windows_targets::link!("icu.dll" "cdecl" fn utrace_getFunctions(context : *const *const ::core::ffi::c_void, e : *mut UTraceEntry, x : *mut UTraceExit, d : *mut UTraceData) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrace_getLevel() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utrace_setFunctions(context : *const ::core::ffi::c_void, e : UTraceEntry, x : UTraceExit, d : UTraceData) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrace_setLevel(tracelevel : i32) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrace_vformat(outbuf : ::windows_sys::core::PCSTR, capacity : i32, indent : i32, fmt : ::windows_sys::core::PCSTR, args : *mut i8) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_clone(trans : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_close(trans : *mut *mut ::core::ffi::c_void) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_countAvailableIDs() -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_getSourceSet(trans : *const *const ::core::ffi::c_void, ignorefilter : i8, fillin : *mut USet, status : *mut UErrorCode) -> *mut USet);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_getUnicodeID(trans : *const *const ::core::ffi::c_void, resultlength : *mut i32) -> *mut u16);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_openIDs(perrorcode : *mut UErrorCode) -> *mut UEnumeration);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_openInverse(trans : *const *const ::core::ffi::c_void, status : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_openU(id : *const u16, idlength : i32, dir : UTransDirection, rules : *const u16, ruleslength : i32, parseerror : *mut UParseError, perrorcode : *mut UErrorCode) -> *mut *mut ::core::ffi::c_void);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_register(adoptedtrans : *mut *mut ::core::ffi::c_void, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_setFilter(trans : *mut *mut ::core::ffi::c_void, filterpattern : *const u16, filterpatternlen : i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_toRules(trans : *const *const ::core::ffi::c_void, escapeunprintable : i8, result : *mut u16, resultlength : i32, status : *mut UErrorCode) -> i32);
::windows_targets::link!("icu.dll" "cdecl" fn utrans_trans(trans : *const *const ::core::ffi::c_void, rep : *mut *mut ::core::ffi::c_void, repfunc : *const UReplaceableCallbacks, start : i32, limit : *mut i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_transIncremental(trans : *const *const ::core::ffi::c_void, rep : *mut *mut ::core::ffi::c_void, repfunc : *const UReplaceableCallbacks, pos : *mut UTransPosition, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_transIncrementalUChars(trans : *const *const ::core::ffi::c_void, text : *mut u16, textlength : *mut i32, textcapacity : i32, pos : *mut UTransPosition, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_transUChars(trans : *const *const ::core::ffi::c_void, text : *mut u16, textlength : *mut i32, textcapacity : i32, start : i32, limit : *mut i32, status : *mut UErrorCode) -> ());
::windows_targets::link!("icu.dll" "cdecl" fn utrans_unregisterID(id : *const u16, idlength : i32) -> ());
pub type IComprehensiveSpellCheckProvider = *mut ::core::ffi::c_void;
pub type IEnumCodePage = *mut ::core::ffi::c_void;
pub type IEnumRfc1766 = *mut ::core::ffi::c_void;
pub type IEnumScript = *mut ::core::ffi::c_void;
pub type IEnumSpellingError = *mut ::core::ffi::c_void;
pub type IMLangCodePages = *mut ::core::ffi::c_void;
pub type IMLangConvertCharset = *mut ::core::ffi::c_void;
pub type IMLangFontLink = *mut ::core::ffi::c_void;
pub type IMLangFontLink2 = *mut ::core::ffi::c_void;
pub type IMLangLineBreakConsole = *mut ::core::ffi::c_void;
pub type IMLangString = *mut ::core::ffi::c_void;
pub type IMLangStringAStr = *mut ::core::ffi::c_void;
pub type IMLangStringBufA = *mut ::core::ffi::c_void;
pub type IMLangStringBufW = *mut ::core::ffi::c_void;
pub type IMLangStringWStr = *mut ::core::ffi::c_void;
pub type IMultiLanguage = *mut ::core::ffi::c_void;
pub type IMultiLanguage2 = *mut ::core::ffi::c_void;
pub type IMultiLanguage3 = *mut ::core::ffi::c_void;
pub type IOptionDescription = *mut ::core::ffi::c_void;
pub type ISpellCheckProvider = *mut ::core::ffi::c_void;
pub type ISpellCheckProviderFactory = *mut ::core::ffi::c_void;
pub type ISpellChecker = *mut ::core::ffi::c_void;
pub type ISpellChecker2 = *mut ::core::ffi::c_void;
pub type ISpellCheckerChangedEventHandler = *mut ::core::ffi::c_void;
pub type ISpellCheckerFactory = *mut ::core::ffi::c_void;
pub type ISpellingError = *mut ::core::ffi::c_void;
pub type IUserDictionariesRegistrar = *mut ::core::ffi::c_void;
pub const ALL_SERVICES: u32 = 0u32;
pub const ALL_SERVICE_TYPES: u32 = 0u32;
pub const C1_ALPHA: u32 = 256u32;
pub const C1_BLANK: u32 = 64u32;
pub const C1_CNTRL: u32 = 32u32;
pub const C1_DEFINED: u32 = 512u32;
pub const C1_DIGIT: u32 = 4u32;
pub const C1_LOWER: u32 = 2u32;
pub const C1_PUNCT: u32 = 16u32;
pub const C1_SPACE: u32 = 8u32;
pub const C1_UPPER: u32 = 1u32;
pub const C1_XDIGIT: u32 = 128u32;
pub const C2_ARABICNUMBER: u32 = 6u32;
pub const C2_BLOCKSEPARATOR: u32 = 8u32;
pub const C2_COMMONSEPARATOR: u32 = 7u32;
pub const C2_EUROPENUMBER: u32 = 3u32;
pub const C2_EUROPESEPARATOR: u32 = 4u32;
pub const C2_EUROPETERMINATOR: u32 = 5u32;
pub const C2_LEFTTORIGHT: u32 = 1u32;
pub const C2_NOTAPPLICABLE: u32 = 0u32;
pub const C2_OTHERNEUTRAL: u32 = 11u32;
pub const C2_RIGHTTOLEFT: u32 = 2u32;
pub const C2_SEGMENTSEPARATOR: u32 = 9u32;
pub const C2_WHITESPACE: u32 = 10u32;
pub const C3_ALPHA: u32 = 32768u32;
pub const C3_DIACRITIC: u32 = 2u32;
pub const C3_FULLWIDTH: u32 = 128u32;
pub const C3_HALFWIDTH: u32 = 64u32;
pub const C3_HIGHSURROGATE: u32 = 2048u32;
pub const C3_HIRAGANA: u32 = 32u32;
pub const C3_IDEOGRAPH: u32 = 256u32;
pub const C3_KASHIDA: u32 = 512u32;
pub const C3_KATAKANA: u32 = 16u32;
pub const C3_LEXICAL: u32 = 1024u32;
pub const C3_LOWSURROGATE: u32 = 4096u32;
pub const C3_NONSPACING: u32 = 1u32;
pub const C3_NOTAPPLICABLE: u32 = 0u32;
pub const C3_SYMBOL: u32 = 8u32;
pub const C3_VOWELMARK: u32 = 4u32;
pub const CAL_GREGORIAN: u32 = 1u32;
pub const CAL_GREGORIAN_ARABIC: u32 = 10u32;
pub const CAL_GREGORIAN_ME_FRENCH: u32 = 9u32;
pub const CAL_GREGORIAN_US: u32 = 2u32;
pub const CAL_GREGORIAN_XLIT_ENGLISH: u32 = 11u32;
pub const CAL_GREGORIAN_XLIT_FRENCH: u32 = 12u32;
pub const CAL_HEBREW: u32 = 8u32;
pub const CAL_HIJRI: u32 = 6u32;
pub const CAL_ICALINTVALUE: u32 = 1u32;
pub const CAL_ITWODIGITYEARMAX: u32 = 48u32;
pub const CAL_IYEAROFFSETRANGE: u32 = 3u32;
pub const CAL_JAPAN: u32 = 3u32;
pub const CAL_KOREA: u32 = 5u32;
pub const CAL_NOUSEROVERRIDE: u32 = 2147483648u32;
pub const CAL_PERSIAN: u32 = 22u32;
pub const CAL_RETURN_GENITIVE_NAMES: u32 = 268435456u32;
pub const CAL_RETURN_NUMBER: u32 = 536870912u32;
pub const CAL_SABBREVDAYNAME1: u32 = 14u32;
pub const CAL_SABBREVDAYNAME2: u32 = 15u32;
pub const CAL_SABBREVDAYNAME3: u32 = 16u32;
pub const CAL_SABBREVDAYNAME4: u32 = 17u32;
pub const CAL_SABBREVDAYNAME5: u32 = 18u32;
pub const CAL_SABBREVDAYNAME6: u32 = 19u32;
pub const CAL_SABBREVDAYNAME7: u32 = 20u32;
pub const CAL_SABBREVERASTRING: u32 = 57u32;
pub const CAL_SABBREVMONTHNAME1: u32 = 34u32;
pub const CAL_SABBREVMONTHNAME10: u32 = 43u32;
pub const CAL_SABBREVMONTHNAME11: u32 = 44u32;
pub const CAL_SABBREVMONTHNAME12: u32 = 45u32;
pub const CAL_SABBREVMONTHNAME13: u32 = 46u32;
pub const CAL_SABBREVMONTHNAME2: u32 = 35u32;
pub const CAL_SABBREVMONTHNAME3: u32 = 36u32;
pub const CAL_SABBREVMONTHNAME4: u32 = 37u32;
pub const CAL_SABBREVMONTHNAME5: u32 = 38u32;
pub const CAL_SABBREVMONTHNAME6: u32 = 39u32;
pub const CAL_SABBREVMONTHNAME7: u32 = 40u32;
pub const CAL_SABBREVMONTHNAME8: u32 = 41u32;
pub const CAL_SABBREVMONTHNAME9: u32 = 42u32;
pub const CAL_SCALNAME: u32 = 2u32;
pub const CAL_SDAYNAME1: u32 = 7u32;
pub const CAL_SDAYNAME2: u32 = 8u32;
pub const CAL_SDAYNAME3: u32 = 9u32;
pub const CAL_SDAYNAME4: u32 = 10u32;
pub const CAL_SDAYNAME5: u32 = 11u32;
pub const CAL_SDAYNAME6: u32 = 12u32;
pub const CAL_SDAYNAME7: u32 = 13u32;
pub const CAL_SENGLISHABBREVERANAME: u32 = 60u32;
pub const CAL_SENGLISHERANAME: u32 = 59u32;
pub const CAL_SERASTRING: u32 = 4u32;
pub const CAL_SJAPANESEERAFIRSTYEAR: u32 = 61u32;
pub const CAL_SLONGDATE: u32 = 6u32;
pub const CAL_SMONTHDAY: u32 = 56u32;
pub const CAL_SMONTHNAME1: u32 = 21u32;
pub const CAL_SMONTHNAME10: u32 = 30u32;
pub const CAL_SMONTHNAME11: u32 = 31u32;
pub const CAL_SMONTHNAME12: u32 = 32u32;
pub const CAL_SMONTHNAME13: u32 = 33u32;
pub const CAL_SMONTHNAME2: u32 = 22u32;
pub const CAL_SMONTHNAME3: u32 = 23u32;
pub const CAL_SMONTHNAME4: u32 = 24u32;
pub const CAL_SMONTHNAME5: u32 = 25u32;
pub const CAL_SMONTHNAME6: u32 = 26u32;
pub const CAL_SMONTHNAME7: u32 = 27u32;
pub const CAL_SMONTHNAME8: u32 = 28u32;
pub const CAL_SMONTHNAME9: u32 = 29u32;
pub const CAL_SRELATIVELONGDATE: u32 = 58u32;
pub const CAL_SSHORTDATE: u32 = 5u32;
pub const CAL_SSHORTESTDAYNAME1: u32 = 49u32;
pub const CAL_SSHORTESTDAYNAME2: u32 = 50u32;
pub const CAL_SSHORTESTDAYNAME3: u32 = 51u32;
pub const CAL_SSHORTESTDAYNAME4: u32 = 52u32;
pub const CAL_SSHORTESTDAYNAME5: u32 = 53u32;
pub const CAL_SSHORTESTDAYNAME6: u32 = 54u32;
pub const CAL_SSHORTESTDAYNAME7: u32 = 55u32;
pub const CAL_SYEARMONTH: u32 = 47u32;
pub const CAL_TAIWAN: u32 = 4u32;
pub const CAL_THAI: u32 = 7u32;
pub const CAL_UMALQURA: u32 = 23u32;
pub const CAL_USE_CP_ACP: u32 = 1073741824u32;
pub const CANITER_SKIP_ZEROES: u32 = 1u32;
pub const CMLangConvertCharset: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd66d6f99_cdaa_11d0_b822_00c04fc9b31f);
pub const CMLangString: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc04d65cf_b70d_11d0_b188_00aa0038c969);
pub const CMultiLanguage: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x275c23e2_3747_11d0_9fea_00aa003f8646);
pub const COMPARE_STRING: SYSNLS_FUNCTION = 1i32;
pub const CORRECTIVE_ACTION_DELETE: CORRECTIVE_ACTION = 3i32;
pub const CORRECTIVE_ACTION_GET_SUGGESTIONS: CORRECTIVE_ACTION = 1i32;
pub const CORRECTIVE_ACTION_NONE: CORRECTIVE_ACTION = 0i32;
pub const CORRECTIVE_ACTION_REPLACE: CORRECTIVE_ACTION = 2i32;
pub const CPIOD_FORCE_PROMPT: i32 = -2147483648i32;
pub const CPIOD_PEEK: i32 = 1073741824i32;
pub const CP_ACP: u32 = 0u32;
pub const CP_INSTALLED: ENUM_SYSTEM_CODE_PAGES_FLAGS = 1u32;
pub const CP_MACCP: u32 = 2u32;
pub const CP_OEMCP: u32 = 1u32;
pub const CP_SUPPORTED: ENUM_SYSTEM_CODE_PAGES_FLAGS = 2u32;
pub const CP_SYMBOL: u32 = 42u32;
pub const CP_THREAD_ACP: u32 = 3u32;
pub const CP_UTF7: u32 = 65000u32;
pub const CP_UTF8: u32 = 65001u32;
pub const CSTR_EQUAL: COMPARESTRING_RESULT = 2i32;
pub const CSTR_GREATER_THAN: COMPARESTRING_RESULT = 3i32;
pub const CSTR_LESS_THAN: COMPARESTRING_RESULT = 1i32;
pub const CTRY_ALBANIA: u32 = 355u32;
pub const CTRY_ALGERIA: u32 = 213u32;
pub const CTRY_ARGENTINA: u32 = 54u32;
pub const CTRY_ARMENIA: u32 = 374u32;
pub const CTRY_AUSTRALIA: u32 = 61u32;
pub const CTRY_AUSTRIA: u32 = 43u32;
pub const CTRY_AZERBAIJAN: u32 = 994u32;
pub const CTRY_BAHRAIN: u32 = 973u32;
pub const CTRY_BELARUS: u32 = 375u32;
pub const CTRY_BELGIUM: u32 = 32u32;
pub const CTRY_BELIZE: u32 = 501u32;
pub const CTRY_BOLIVIA: u32 = 591u32;
pub const CTRY_BRAZIL: u32 = 55u32;
pub const CTRY_BRUNEI_DARUSSALAM: u32 = 673u32;
pub const CTRY_BULGARIA: u32 = 359u32;
pub const CTRY_CANADA: u32 = 2u32;
pub const CTRY_CARIBBEAN: u32 = 1u32;
pub const CTRY_CHILE: u32 = 56u32;
pub const CTRY_COLOMBIA: u32 = 57u32;
pub const CTRY_COSTA_RICA: u32 = 506u32;
pub const CTRY_CROATIA: u32 = 385u32;
pub const CTRY_CZECH: u32 = 420u32;
pub const CTRY_DEFAULT: u32 = 0u32;
pub const CTRY_DENMARK: u32 = 45u32;
pub const CTRY_DOMINICAN_REPUBLIC: u32 = 1u32;
pub const CTRY_ECUADOR: u32 = 593u32;
pub const CTRY_EGYPT: u32 = 20u32;
pub const CTRY_EL_SALVADOR: u32 = 503u32;
pub const CTRY_ESTONIA: u32 = 372u32;
pub const CTRY_FAEROE_ISLANDS: u32 = 298u32;
pub const CTRY_FINLAND: u32 = 358u32;
pub const CTRY_FRANCE: u32 = 33u32;
pub const CTRY_GEORGIA: u32 = 995u32;
pub const CTRY_GERMANY: u32 = 49u32;
pub const CTRY_GREECE: u32 = 30u32;
pub const CTRY_GUATEMALA: u32 = 502u32;
pub const CTRY_HONDURAS: u32 = 504u32;
pub const CTRY_HONG_KONG: u32 = 852u32;
pub const CTRY_HUNGARY: u32 = 36u32;
pub const CTRY_ICELAND: u32 = 354u32;
pub const CTRY_INDIA: u32 = 91u32;
pub const CTRY_INDONESIA: u32 = 62u32;
pub const CTRY_IRAN: u32 = 981u32;
pub const CTRY_IRAQ: u32 = 964u32;
pub const CTRY_IRELAND: u32 = 353u32;
pub const CTRY_ISRAEL: u32 = 972u32;
pub const CTRY_ITALY: u32 = 39u32;
pub const CTRY_JAMAICA: u32 = 1u32;
pub const CTRY_JAPAN: u32 = 81u32;
pub const CTRY_JORDAN: u32 = 962u32;
pub const CTRY_KAZAKSTAN: u32 = 7u32;
pub const CTRY_KENYA: u32 = 254u32;
pub const CTRY_KUWAIT: u32 = 965u32;
pub const CTRY_KYRGYZSTAN: u32 = 996u32;
pub const CTRY_LATVIA: u32 = 371u32;
pub const CTRY_LEBANON: u32 = 961u32;
pub const CTRY_LIBYA: u32 = 218u32;
pub const CTRY_LIECHTENSTEIN: u32 = 41u32;
pub const CTRY_LITHUANIA: u32 = 370u32;
pub const CTRY_LUXEMBOURG: u32 = 352u32;
pub const CTRY_MACAU: u32 = 853u32;
pub const CTRY_MACEDONIA: u32 = 389u32;
pub const CTRY_MALAYSIA: u32 = 60u32;
pub const CTRY_MALDIVES: u32 = 960u32;
pub const CTRY_MEXICO: u32 = 52u32;
pub const CTRY_MONACO: u32 = 33u32;
pub const CTRY_MONGOLIA: u32 = 976u32;
pub const CTRY_MOROCCO: u32 = 212u32;
pub const CTRY_NETHERLANDS: u32 = 31u32;
pub const CTRY_NEW_ZEALAND: u32 = 64u32;
pub const CTRY_NICARAGUA: u32 = 505u32;
pub const CTRY_NORWAY: u32 = 47u32;
pub const CTRY_OMAN: u32 = 968u32;
pub const CTRY_PAKISTAN: u32 = 92u32;
pub const CTRY_PANAMA: u32 = 507u32;
pub const CTRY_PARAGUAY: u32 = 595u32;
pub const CTRY_PERU: u32 = 51u32;
pub const CTRY_PHILIPPINES: u32 = 63u32;
pub const CTRY_POLAND: u32 = 48u32;
pub const CTRY_PORTUGAL: u32 = 351u32;
pub const CTRY_PRCHINA: u32 = 86u32;
pub const CTRY_PUERTO_RICO: u32 = 1u32;
pub const CTRY_QATAR: u32 = 974u32;
pub const CTRY_ROMANIA: u32 = 40u32;
pub const CTRY_RUSSIA: u32 = 7u32;
pub const CTRY_SAUDI_ARABIA: u32 = 966u32;
pub const CTRY_SERBIA: u32 = 381u32;
pub const CTRY_SINGAPORE: u32 = 65u32;
pub const CTRY_SLOVAK: u32 = 421u32;
pub const CTRY_SLOVENIA: u32 = 386u32;
pub const CTRY_SOUTH_AFRICA: u32 = 27u32;
pub const CTRY_SOUTH_KOREA: u32 = 82u32;
pub const CTRY_SPAIN: u32 = 34u32;
pub const CTRY_SWEDEN: u32 = 46u32;
pub const CTRY_SWITZERLAND: u32 = 41u32;
pub const CTRY_SYRIA: u32 = 963u32;
pub const CTRY_TAIWAN: u32 = 886u32;
pub const CTRY_TATARSTAN: u32 = 7u32;
pub const CTRY_THAILAND: u32 = 66u32;
pub const CTRY_TRINIDAD_Y_TOBAGO: u32 = 1u32;
pub const CTRY_TUNISIA: u32 = 216u32;
pub const CTRY_TURKEY: u32 = 90u32;
pub const CTRY_UAE: u32 = 971u32;
pub const CTRY_UKRAINE: u32 = 380u32;
pub const CTRY_UNITED_KINGDOM: u32 = 44u32;
pub const CTRY_UNITED_STATES: u32 = 1u32;
pub const CTRY_URUGUAY: u32 = 598u32;
pub const CTRY_UZBEKISTAN: u32 = 7u32;
pub const CTRY_VENEZUELA: u32 = 58u32;
pub const CTRY_VIET_NAM: u32 = 84u32;
pub const CTRY_YEMEN: u32 = 967u32;
pub const CTRY_ZIMBABWE: u32 = 263u32;
pub const CT_CTYPE1: u32 = 1u32;
pub const CT_CTYPE2: u32 = 2u32;
pub const CT_CTYPE3: u32 = 4u32;
pub const DATE_AUTOLAYOUT: ENUM_DATE_FORMATS_FLAGS = 64u32;
pub const DATE_LONGDATE: ENUM_DATE_FORMATS_FLAGS = 2u32;
pub const DATE_LTRREADING: ENUM_DATE_FORMATS_FLAGS = 16u32;
pub const DATE_MONTHDAY: ENUM_DATE_FORMATS_FLAGS = 128u32;
pub const DATE_RTLREADING: ENUM_DATE_FORMATS_FLAGS = 32u32;
pub const DATE_SHORTDATE: ENUM_DATE_FORMATS_FLAGS = 1u32;
pub const DATE_USE_ALT_CALENDAR: ENUM_DATE_FORMATS_FLAGS = 4u32;
pub const DATE_YEARMONTH: ENUM_DATE_FORMATS_FLAGS = 8u32;
pub const ELS_GUID_LANGUAGE_DETECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xcf7e00b1_909b_4d95_a8f4_611f7c377702);
pub const ELS_GUID_SCRIPT_DETECTION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x2d64b439_6caf_4f6b_b688_e5d0f4faa7d7);
pub const ELS_GUID_TRANSLITERATION_BENGALI_TO_LATIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xf4dfd825_91a4_489f_855e_9ad9bee55727);
pub const ELS_GUID_TRANSLITERATION_CYRILLIC_TO_LATIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3dd12a98_5afd_4903_a13f_e17e6c0bfe01);
pub const ELS_GUID_TRANSLITERATION_DEVANAGARI_TO_LATIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xc4a4dcfe_2661_4d02_9835_f48187109803);
pub const ELS_GUID_TRANSLITERATION_HANGUL_DECOMPOSITION: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x4ba2a721_e43d_41b7_b330_536ae1e48863);
pub const ELS_GUID_TRANSLITERATION_HANS_TO_HANT: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x3caccdc8_5590_42dc_9a7b_b5a6b5b3b63b);
pub const ELS_GUID_TRANSLITERATION_HANT_TO_HANS: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xa3a8333b_f4fc_42f6_a0c4_0462fe7317cb);
pub const ELS_GUID_TRANSLITERATION_MALAYALAM_TO_LATIN: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0xd8b983b1_f8bf_4a2b_bcd5_5b5ea20613e1);
pub const ENUM_ALL_CALENDARS: u32 = 4294967295u32;
pub const FIND_ENDSWITH: u32 = 2097152u32;
pub const FIND_FROMEND: u32 = 8388608u32;
pub const FIND_FROMSTART: u32 = 4194304u32;
pub const FIND_STARTSWITH: u32 = 1048576u32;
pub const GEOCLASS_ALL: SYSGEOCLASS = 0i32;
pub const GEOCLASS_NATION: SYSGEOCLASS = 16i32;
pub const GEOCLASS_REGION: SYSGEOCLASS = 14i32;
pub const GEOID_NOT_AVAILABLE: i32 = -1i32;
pub const GEO_CURRENCYCODE: SYSGEOTYPE = 15i32;
pub const GEO_CURRENCYSYMBOL: SYSGEOTYPE = 16i32;
pub const GEO_DIALINGCODE: SYSGEOTYPE = 14i32;
pub const GEO_FRIENDLYNAME: SYSGEOTYPE = 8i32;
pub const GEO_ID: SYSGEOTYPE = 18i32;
pub const GEO_ISO2: SYSGEOTYPE = 4i32;
pub const GEO_ISO3: SYSGEOTYPE = 5i32;
pub const GEO_ISO_UN_NUMBER: SYSGEOTYPE = 12i32;
pub const GEO_LATITUDE: SYSGEOTYPE = 2i32;
pub const GEO_LCID: SYSGEOTYPE = 7i32;
pub const GEO_LONGITUDE: SYSGEOTYPE = 3i32;
pub const GEO_NAME: SYSGEOTYPE = 17i32;
pub const GEO_NATION: SYSGEOTYPE = 1i32;
pub const GEO_OFFICIALLANGUAGES: SYSGEOTYPE = 11i32;
pub const GEO_OFFICIALNAME: SYSGEOTYPE = 9i32;
pub const GEO_PARENT: SYSGEOTYPE = 13i32;
pub const GEO_RFC1766: SYSGEOTYPE = 6i32;
pub const GEO_TIMEZONES: SYSGEOTYPE = 10i32;
pub const GSS_ALLOW_INHERITED_COMMON: u32 = 1u32;
pub const HIGHLEVEL_SERVICE_TYPES: u32 = 1u32;
pub const HIGH_SURROGATE_END: u32 = 56319u32;
pub const HIGH_SURROGATE_START: u32 = 55296u32;
pub const IDN_ALLOW_UNASSIGNED: u32 = 1u32;
pub const IDN_EMAIL_ADDRESS: u32 = 4u32;
pub const IDN_RAW_PUNYCODE: u32 = 8u32;
pub const IDN_USE_STD3_ASCII_RULES: u32 = 2u32;
pub const IS_TEXT_UNICODE_ASCII16: IS_TEXT_UNICODE_RESULT = 1u32;
pub const IS_TEXT_UNICODE_CONTROLS: IS_TEXT_UNICODE_RESULT = 4u32;
pub const IS_TEXT_UNICODE_ILLEGAL_CHARS: IS_TEXT_UNICODE_RESULT = 256u32;
pub const IS_TEXT_UNICODE_NOT_ASCII_MASK: IS_TEXT_UNICODE_RESULT = 61440u32;
pub const IS_TEXT_UNICODE_NOT_UNICODE_MASK: IS_TEXT_UNICODE_RESULT = 3840u32;
pub const IS_TEXT_UNICODE_NULL_BYTES: IS_TEXT_UNICODE_RESULT = 4096u32;
pub const IS_TEXT_UNICODE_ODD_LENGTH: IS_TEXT_UNICODE_RESULT = 512u32;
pub const IS_TEXT_UNICODE_REVERSE_ASCII16: IS_TEXT_UNICODE_RESULT = 16u32;
pub const IS_TEXT_UNICODE_REVERSE_CONTROLS: IS_TEXT_UNICODE_RESULT = 64u32;
pub const IS_TEXT_UNICODE_REVERSE_MASK: IS_TEXT_UNICODE_RESULT = 240u32;
pub const IS_TEXT_UNICODE_REVERSE_SIGNATURE: IS_TEXT_UNICODE_RESULT = 128u32;
pub const IS_TEXT_UNICODE_REVERSE_STATISTICS: IS_TEXT_UNICODE_RESULT = 32u32;
pub const IS_TEXT_UNICODE_SIGNATURE: IS_TEXT_UNICODE_RESULT = 8u32;
pub const IS_TEXT_UNICODE_STATISTICS: IS_TEXT_UNICODE_RESULT = 2u32;
pub const IS_TEXT_UNICODE_UNICODE_MASK: IS_TEXT_UNICODE_RESULT = 15u32;
pub const LCID_ALTERNATE_SORTS: u32 = 4u32;
pub const LCID_INSTALLED: IS_VALID_LOCALE_FLAGS = 1u32;
pub const LCID_SUPPORTED: IS_VALID_LOCALE_FLAGS = 2u32;
pub const LCMAP_BYTEREV: u32 = 2048u32;
pub const LCMAP_FULLWIDTH: u32 = 8388608u32;
pub const LCMAP_HALFWIDTH: u32 = 4194304u32;
pub const LCMAP_HASH: u32 = 262144u32;
pub const LCMAP_HIRAGANA: u32 = 1048576u32;
pub const LCMAP_KATAKANA: u32 = 2097152u32;
pub const LCMAP_LINGUISTIC_CASING: u32 = 16777216u32;
pub const LCMAP_LOWERCASE: u32 = 256u32;
pub const LCMAP_SIMPLIFIED_CHINESE: u32 = 33554432u32;
pub const LCMAP_SORTHANDLE: u32 = 536870912u32;
pub const LCMAP_SORTKEY: u32 = 1024u32;
pub const LCMAP_TITLECASE: u32 = 768u32;
pub const LCMAP_TRADITIONAL_CHINESE: u32 = 67108864u32;
pub const LCMAP_UPPERCASE: u32 = 512u32;
pub const LGRPID_ARABIC: u32 = 13u32;
pub const LGRPID_ARMENIAN: u32 = 17u32;
pub const LGRPID_BALTIC: u32 = 3u32;
pub const LGRPID_CENTRAL_EUROPE: u32 = 2u32;
pub const LGRPID_CYRILLIC: u32 = 5u32;
pub const LGRPID_GEORGIAN: u32 = 16u32;
pub const LGRPID_GREEK: u32 = 4u32;
pub const LGRPID_HEBREW: u32 = 12u32;
pub const LGRPID_INDIC: u32 = 15u32;
pub const LGRPID_INSTALLED: ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS = 1u32;
pub const LGRPID_JAPANESE: u32 = 7u32;
pub const LGRPID_KOREAN: u32 = 8u32;
pub const LGRPID_SIMPLIFIED_CHINESE: u32 = 10u32;
pub const LGRPID_SUPPORTED: ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS = 2u32;
pub const LGRPID_THAI: u32 = 11u32;
pub const LGRPID_TRADITIONAL_CHINESE: u32 = 9u32;
pub const LGRPID_TURKIC: u32 = 6u32;
pub const LGRPID_TURKISH: u32 = 6u32;
pub const LGRPID_VIETNAMESE: u32 = 14u32;
pub const LGRPID_WESTERN_EUROPE: u32 = 1u32;
pub const LINGUISTIC_IGNORECASE: COMPARE_STRING_FLAGS = 16u32;
pub const LINGUISTIC_IGNOREDIACRITIC: COMPARE_STRING_FLAGS = 32u32;
pub const LOCALE_ALL: u32 = 0u32;
pub const LOCALE_ALLOW_NEUTRAL_NAMES: u32 = 134217728u32;
pub const LOCALE_ALTERNATE_SORTS: u32 = 4u32;
pub const LOCALE_FONTSIGNATURE: u32 = 88u32;
pub const LOCALE_ICALENDARTYPE: u32 = 4105u32;
pub const LOCALE_ICENTURY: u32 = 36u32;
pub const LOCALE_ICONSTRUCTEDLOCALE: u32 = 125u32;
pub const LOCALE_ICOUNTRY: u32 = 5u32;
pub const LOCALE_ICURRDIGITS: u32 = 25u32;
pub const LOCALE_ICURRENCY: u32 = 27u32;
pub const LOCALE_IDATE: u32 = 33u32;
pub const LOCALE_IDAYLZERO: u32 = 38u32;
pub const LOCALE_IDEFAULTANSICODEPAGE: u32 = 4100u32;
pub const LOCALE_IDEFAULTCODEPAGE: u32 = 11u32;
pub const LOCALE_IDEFAULTCOUNTRY: u32 = 10u32;
pub const LOCALE_IDEFAULTEBCDICCODEPAGE: u32 = 4114u32;
pub const LOCALE_IDEFAULTLANGUAGE: u32 = 9u32;
pub const LOCALE_IDEFAULTMACCODEPAGE: u32 = 4113u32;
pub const LOCALE_IDIALINGCODE: u32 = 5u32;
pub const LOCALE_IDIGITS: u32 = 17u32;
pub const LOCALE_IDIGITSUBSTITUTION: u32 = 4116u32;
pub const LOCALE_IFIRSTDAYOFWEEK: u32 = 4108u32;
pub const LOCALE_IFIRSTWEEKOFYEAR: u32 = 4109u32;
pub const LOCALE_IGEOID: u32 = 91u32;
pub const LOCALE_IINTLCURRDIGITS: u32 = 26u32;
pub const LOCALE_ILANGUAGE: u32 = 1u32;
pub const LOCALE_ILDATE: u32 = 34u32;
pub const LOCALE_ILZERO: u32 = 18u32;
pub const LOCALE_IMEASURE: u32 = 13u32;
pub const LOCALE_IMONLZERO: u32 = 39u32;
pub const LOCALE_INEGATIVEPERCENT: u32 = 116u32;
pub const LOCALE_INEGCURR: u32 = 28u32;
pub const LOCALE_INEGNUMBER: u32 = 4112u32;
pub const LOCALE_INEGSEPBYSPACE: u32 = 87u32;
pub const LOCALE_INEGSIGNPOSN: u32 = 83u32;
pub const LOCALE_INEGSYMPRECEDES: u32 = 86u32;
pub const LOCALE_INEUTRAL: u32 = 113u32;
pub const LOCALE_IOPTIONALCALENDAR: u32 = 4107u32;
pub const LOCALE_IPAPERSIZE: u32 = 4106u32;
pub const LOCALE_IPOSITIVEPERCENT: u32 = 117u32;
pub const LOCALE_IPOSSEPBYSPACE: u32 = 85u32;
pub const LOCALE_IPOSSIGNPOSN: u32 = 82u32;
pub const LOCALE_IPOSSYMPRECEDES: u32 = 84u32;
pub const LOCALE_IREADINGLAYOUT: u32 = 112u32;
pub const LOCALE_ITIME: u32 = 35u32;
pub const LOCALE_ITIMEMARKPOSN: u32 = 4101u32;
pub const LOCALE_ITLZERO: u32 = 37u32;
pub const LOCALE_IUSEUTF8LEGACYACP: u32 = 1638u32;
pub const LOCALE_IUSEUTF8LEGACYOEMCP: u32 = 2457u32;
pub const LOCALE_NAME_INVARIANT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("");
pub const LOCALE_NAME_SYSTEM_DEFAULT: ::windows_sys::core::PCWSTR = ::windows_sys::core::w!("!x-sys-default-locale");
pub const LOCALE_NEUTRALDATA: u32 = 16u32;
pub const LOCALE_NOUSEROVERRIDE: u32 = 2147483648u32;
pub const LOCALE_REPLACEMENT: u32 = 8u32;
pub const LOCALE_RETURN_GENITIVE_NAMES: u32 = 268435456u32;
pub const LOCALE_RETURN_NUMBER: u32 = 536870912u32;
pub const LOCALE_S1159: u32 = 40u32;
pub const LOCALE_S2359: u32 = 41u32;
pub const LOCALE_SABBREVCTRYNAME: u32 = 7u32;
pub const LOCALE_SABBREVDAYNAME1: u32 = 49u32;
pub const LOCALE_SABBREVDAYNAME2: u32 = 50u32;
pub const LOCALE_SABBREVDAYNAME3: u32 = 51u32;
pub const LOCALE_SABBREVDAYNAME4: u32 = 52u32;
pub const LOCALE_SABBREVDAYNAME5: u32 = 53u32;
pub const LOCALE_SABBREVDAYNAME6: u32 = 54u32;
pub const LOCALE_SABBREVDAYNAME7: u32 = 55u32;
pub const LOCALE_SABBREVLANGNAME: u32 = 3u32;
pub const LOCALE_SABBREVMONTHNAME1: u32 = 68u32;
pub const LOCALE_SABBREVMONTHNAME10: u32 = 77u32;
pub const LOCALE_SABBREVMONTHNAME11: u32 = 78u32;
pub const LOCALE_SABBREVMONTHNAME12: u32 = 79u32;
pub const LOCALE_SABBREVMONTHNAME13: u32 = 4111u32;
pub const LOCALE_SABBREVMONTHNAME2: u32 = 69u32;
pub const LOCALE_SABBREVMONTHNAME3: u32 = 70u32;
pub const LOCALE_SABBREVMONTHNAME4: u32 = 71u32;
pub const LOCALE_SABBREVMONTHNAME5: u32 = 72u32;
pub const LOCALE_SABBREVMONTHNAME6: u32 = 73u32;
pub const LOCALE_SABBREVMONTHNAME7: u32 = 74u32;
pub const LOCALE_SABBREVMONTHNAME8: u32 = 75u32;
pub const LOCALE_SABBREVMONTHNAME9: u32 = 76u32;
pub const LOCALE_SAM: u32 = 40u32;
pub const LOCALE_SCONSOLEFALLBACKNAME: u32 = 110u32;
pub const LOCALE_SCOUNTRY: u32 = 6u32;
pub const LOCALE_SCURRENCY: u32 = 20u32;
pub const LOCALE_SDATE: u32 = 29u32;
pub const LOCALE_SDAYNAME1: u32 = 42u32;
pub const LOCALE_SDAYNAME2: u32 = 43u32;
pub const LOCALE_SDAYNAME3: u32 = 44u32;
pub const LOCALE_SDAYNAME4: u32 = 45u32;
pub const LOCALE_SDAYNAME5: u32 = 46u32;
pub const LOCALE_SDAYNAME6: u32 = 47u32;
pub const LOCALE_SDAYNAME7: u32 = 48u32;
pub const LOCALE_SDECIMAL: u32 = 14u32;
pub const LOCALE_SDURATION: u32 = 93u32;
pub const LOCALE_SENGCOUNTRY: u32 = 4098u32;
pub const LOCALE_SENGCURRNAME: u32 = 4103u32;
pub const LOCALE_SENGLANGUAGE: u32 = 4097u32;
pub const LOCALE_SENGLISHCOUNTRYNAME: u32 = 4098u32;
pub const LOCALE_SENGLISHDISPLAYNAME: u32 = 114u32;
pub const LOCALE_SENGLISHLANGUAGENAME: u32 = 4097u32;
pub const LOCALE_SGROUPING: u32 = 16u32;
pub const LOCALE_SINTLSYMBOL: u32 = 21u32;
pub const LOCALE_SISO3166CTRYNAME: u32 = 90u32;
pub const LOCALE_SISO3166CTRYNAME2: u32 = 104u32;
pub const LOCALE_SISO639LANGNAME: u32 = 89u32;
pub const LOCALE_SISO639LANGNAME2: u32 = 103u32;
pub const LOCALE_SKEYBOARDSTOINSTALL: u32 = 94u32;
pub const LOCALE_SLANGDISPLAYNAME: u32 = 111u32;
pub const LOCALE_SLANGUAGE: u32 = 2u32;
pub const LOCALE_SLIST: u32 = 12u32;
pub const LOCALE_SLOCALIZEDCOUNTRYNAME: u32 = 6u32;
pub const LOCALE_SLOCALIZEDDISPLAYNAME: u32 = 2u32;
pub const LOCALE_SLOCALIZEDLANGUAGENAME: u32 = 111u32;
pub const LOCALE_SLONGDATE: u32 = 32u32;
pub const LOCALE_SMONDECIMALSEP: u32 = 22u32;
pub const LOCALE_SMONGROUPING: u32 = 24u32;
pub const LOCALE_SMONTHDAY: u32 = 120u32;
pub const LOCALE_SMONTHNAME1: u32 = 56u32;
pub const LOCALE_SMONTHNAME10: u32 = 65u32;
pub const LOCALE_SMONTHNAME11: u32 = 66u32;
pub const LOCALE_SMONTHNAME12: u32 = 67u32;
pub const LOCALE_SMONTHNAME13: u32 = 4110u32;
pub const LOCALE_SMONTHNAME2: u32 = 57u32;
pub const LOCALE_SMONTHNAME3: u32 = 58u32;
pub const LOCALE_SMONTHNAME4: u32 = 59u32;
pub const LOCALE_SMONTHNAME5: u32 = 60u32;
pub const LOCALE_SMONTHNAME6: u32 = 61u32;
pub const LOCALE_SMONTHNAME7: u32 = 62u32;
pub const LOCALE_SMONTHNAME8: u32 = 63u32;
pub const LOCALE_SMONTHNAME9: u32 = 64u32;
pub const LOCALE_SMONTHOUSANDSEP: u32 = 23u32;
pub const LOCALE_SNAME: u32 = 92u32;
pub const LOCALE_SNAN: u32 = 105u32;
pub const LOCALE_SNATIVECOUNTRYNAME: u32 = 8u32;
pub const LOCALE_SNATIVECTRYNAME: u32 = 8u32;
pub const LOCALE_SNATIVECURRNAME: u32 = 4104u32;
pub const LOCALE_SNATIVEDIGITS: u32 = 19u32;
pub const LOCALE_SNATIVEDISPLAYNAME: u32 = 115u32;
pub const LOCALE_SNATIVELANGNAME: u32 = 4u32;
pub const LOCALE_SNATIVELANGUAGENAME: u32 = 4u32;
pub const LOCALE_SNEGATIVESIGN: u32 = 81u32;
pub const LOCALE_SNEGINFINITY: u32 = 107u32;
pub const LOCALE_SOPENTYPELANGUAGETAG: u32 = 122u32;
pub const LOCALE_SPARENT: u32 = 109u32;
pub const LOCALE_SPECIFICDATA: u32 = 32u32;
pub const LOCALE_SPERCENT: u32 = 118u32;
pub const LOCALE_SPERMILLE: u32 = 119u32;
pub const LOCALE_SPM: u32 = 41u32;
pub const LOCALE_SPOSINFINITY: u32 = 106u32;
pub const LOCALE_SPOSITIVESIGN: u32 = 80u32;
pub const LOCALE_SRELATIVELONGDATE: u32 = 124u32;
pub const LOCALE_SSCRIPTS: u32 = 108u32;
pub const LOCALE_SSHORTDATE: u32 = 31u32;
pub const LOCALE_SSHORTESTAM: u32 = 126u32;
pub const LOCALE_SSHORTESTDAYNAME1: u32 = 96u32;
pub const LOCALE_SSHORTESTDAYNAME2: u32 = 97u32;
pub const LOCALE_SSHORTESTDAYNAME3: u32 = 98u32;
pub const LOCALE_SSHORTESTDAYNAME4: u32 = 99u32;
pub const LOCALE_SSHORTESTDAYNAME5: u32 = 100u32;
pub const LOCALE_SSHORTESTDAYNAME6: u32 = 101u32;
pub const LOCALE_SSHORTESTDAYNAME7: u32 = 102u32;
pub const LOCALE_SSHORTESTPM: u32 = 127u32;
pub const LOCALE_SSHORTTIME: u32 = 121u32;
pub const LOCALE_SSORTLOCALE: u32 = 123u32;
pub const LOCALE_SSORTNAME: u32 = 4115u32;
pub const LOCALE_STHOUSAND: u32 = 15u32;
pub const LOCALE_STIME: u32 = 30u32;
pub const LOCALE_STIMEFORMAT: u32 = 4099u32;
pub const LOCALE_SUPPLEMENTAL: u32 = 2u32;
pub const LOCALE_SYEARMONTH: u32 = 4102u32;
pub const LOCALE_USE_CP_ACP: u32 = 1073741824u32;
pub const LOCALE_WINDOWS: u32 = 1u32;
pub const LOWLEVEL_SERVICE_TYPES: u32 = 2u32;
pub const LOW_SURROGATE_END: u32 = 57343u32;
pub const LOW_SURROGATE_START: u32 = 56320u32;
pub const MAP_COMPOSITE: FOLD_STRING_MAP_FLAGS = 64u32;
pub const MAP_EXPAND_LIGATURES: FOLD_STRING_MAP_FLAGS = 8192u32;
pub const MAP_FOLDCZONE: FOLD_STRING_MAP_FLAGS = 16u32;
pub const MAP_FOLDDIGITS: FOLD_STRING_MAP_FLAGS = 128u32;
pub const MAP_PRECOMPOSED: FOLD_STRING_MAP_FLAGS = 32u32;
pub const MAX_DEFAULTCHAR: u32 = 2u32;
pub const MAX_LEADBYTES: u32 = 12u32;
pub const MAX_LOCALE_NAME: u32 = 32u32;
pub const MAX_MIMECP_NAME: u32 = 64u32;
pub const MAX_MIMECSET_NAME: u32 = 50u32;
pub const MAX_MIMEFACE_NAME: u32 = 32u32;
pub const MAX_RFC1766_NAME: u32 = 6u32;
pub const MAX_SCRIPT_NAME: u32 = 48u32;
pub const MB_COMPOSITE: MULTI_BYTE_TO_WIDE_CHAR_FLAGS = 2u32;
pub const MB_ERR_INVALID_CHARS: MULTI_BYTE_TO_WIDE_CHAR_FLAGS = 8u32;
pub const MB_PRECOMPOSED: MULTI_BYTE_TO_WIDE_CHAR_FLAGS = 1u32;
pub const MB_USEGLYPHCHARS: MULTI_BYTE_TO_WIDE_CHAR_FLAGS = 4u32;
pub const MIMECONTF_BROWSER: MIMECONTF = 2i32;
pub const MIMECONTF_EXPORT: MIMECONTF = 1024i32;
pub const MIMECONTF_IMPORT: MIMECONTF = 8i32;
pub const MIMECONTF_MAILNEWS: MIMECONTF = 1i32;
pub const MIMECONTF_MIME_IE4: MIMECONTF = 268435456i32;
pub const MIMECONTF_MIME_LATEST: MIMECONTF = 536870912i32;
pub const MIMECONTF_MIME_REGISTRY: MIMECONTF = 1073741824i32;
pub const MIMECONTF_MINIMAL: MIMECONTF = 4i32;
pub const MIMECONTF_PRIVCONVERTER: MIMECONTF = 65536i32;
pub const MIMECONTF_SAVABLE_BROWSER: MIMECONTF = 512i32;
pub const MIMECONTF_SAVABLE_MAILNEWS: MIMECONTF = 256i32;
pub const MIMECONTF_VALID: MIMECONTF = 131072i32;
pub const MIMECONTF_VALID_NLS: MIMECONTF = 262144i32;
pub const MIN_SPELLING_NTDDI: u32 = 100794368u32;
pub const MLCONVCHARF_AUTODETECT: MLCONVCHAR = 1i32;
pub const MLCONVCHARF_DETECTJPN: MLCONVCHAR = 32i32;
pub const MLCONVCHARF_ENTITIZE: MLCONVCHAR = 2i32;
pub const MLCONVCHARF_NAME_ENTITIZE: MLCONVCHAR = 4i32;
pub const MLCONVCHARF_NCR_ENTITIZE: MLCONVCHAR = 2i32;
pub const MLCONVCHARF_NOBESTFITCHARS: MLCONVCHAR = 16i32;
pub const MLCONVCHARF_USEDEFCHAR: MLCONVCHAR = 8i32;
pub const MLDETECTCP_7BIT: MLDETECTCP = 1i32;
pub const MLDETECTCP_8BIT: MLDETECTCP = 2i32;
pub const MLDETECTCP_DBCS: MLDETECTCP = 4i32;
pub const MLDETECTCP_HTML: MLDETECTCP = 8i32;
pub const MLDETECTCP_NONE: MLDETECTCP = 0i32;
pub const MLDETECTCP_NUMBER: MLDETECTCP = 16i32;
pub const MLDETECTF_BROWSER: MLCP = 2i32;
pub const MLDETECTF_EURO_UTF8: MLCP = 128i32;
pub const MLDETECTF_FILTER_SPECIALCHAR: MLCP = 64i32;
pub const MLDETECTF_MAILNEWS: MLCP = 1i32;
pub const MLDETECTF_PREFERRED_ONLY: MLCP = 32i32;
pub const MLDETECTF_PRESERVE_ORDER: MLCP = 16i32;
pub const MLDETECTF_VALID: MLCP = 4i32;
pub const MLDETECTF_VALID_NLS: MLCP = 8i32;
pub const MLSTR_READ: MLSTR_FLAGS = 1i32;
pub const MLSTR_WRITE: MLSTR_FLAGS = 2i32;
pub const MUI_COMPLEX_SCRIPT_FILTER: u32 = 512u32;
pub const MUI_CONSOLE_FILTER: u32 = 256u32;
pub const MUI_FILEINFO_VERSION: u32 = 1u32;
pub const MUI_FILETYPE_LANGUAGE_NEUTRAL_MAIN: u32 = 2u32;
pub const MUI_FILETYPE_LANGUAGE_NEUTRAL_MUI: u32 = 4u32;
pub const MUI_FILETYPE_NOT_LANGUAGE_NEUTRAL: u32 = 1u32;
pub const MUI_FORMAT_INF_COMPAT: u32 = 2u32;
pub const MUI_FORMAT_REG_COMPAT: u32 = 1u32;
pub const MUI_FULL_LANGUAGE: u32 = 1u32;
pub const MUI_IMMUTABLE_LOOKUP: u32 = 16u32;
pub const MUI_LANGUAGE_EXACT: u32 = 16u32;
pub const MUI_LANGUAGE_ID: u32 = 4u32;
pub const MUI_LANGUAGE_INSTALLED: u32 = 32u32;
pub const MUI_LANGUAGE_LICENSED: u32 = 64u32;
pub const MUI_LANGUAGE_NAME: u32 = 8u32;
pub const MUI_LANG_NEUTRAL_PE_FILE: u32 = 256u32;
pub const MUI_LIP_LANGUAGE: u32 = 4u32;
pub const MUI_MACHINE_LANGUAGE_SETTINGS: u32 = 1024u32;
pub const MUI_MERGE_SYSTEM_FALLBACK: u32 = 16u32;
pub const MUI_MERGE_USER_FALLBACK: u32 = 32u32;
pub const MUI_NON_LANG_NEUTRAL_FILE: u32 = 512u32;
pub const MUI_PARTIAL_LANGUAGE: u32 = 2u32;
pub const MUI_QUERY_CHECKSUM: u32 = 2u32;
pub const MUI_QUERY_LANGUAGE_NAME: u32 = 4u32;
pub const MUI_QUERY_RESOURCE_TYPES: u32 = 8u32;
pub const MUI_QUERY_TYPE: u32 = 1u32;
pub const MUI_RESET_FILTERS: u32 = 1u32;
pub const MUI_SKIP_STRING_CACHE: u32 = 8u32;
pub const MUI_THREAD_LANGUAGES: u32 = 64u32;
pub const MUI_USER_PREFERRED_UI_LANGUAGES: u32 = 16u32;
pub const MUI_USE_INSTALLED_LANGUAGES: u32 = 32u32;
pub const MUI_USE_SEARCH_ALL_LANGUAGES: u32 = 64u32;
pub const MUI_VERIFY_FILE_EXISTS: u32 = 4u32;
pub const NLS_CP_CPINFO: u32 = 268435456u32;
pub const NLS_CP_MBTOWC: u32 = 1073741824u32;
pub const NLS_CP_WCTOMB: u32 = 2147483648u32;
pub const NORM_IGNORECASE: COMPARE_STRING_FLAGS = 1u32;
pub const NORM_IGNOREKANATYPE: COMPARE_STRING_FLAGS = 65536u32;
pub const NORM_IGNORENONSPACE: COMPARE_STRING_FLAGS = 2u32;
pub const NORM_IGNORESYMBOLS: COMPARE_STRING_FLAGS = 4u32;
pub const NORM_IGNOREWIDTH: COMPARE_STRING_FLAGS = 131072u32;
pub const NORM_LINGUISTIC_CASING: COMPARE_STRING_FLAGS = 134217728u32;
pub const NUMSYS_NAME_CAPACITY: u32 = 8u32;
pub const NormalizationC: NORM_FORM = 1i32;
pub const NormalizationD: NORM_FORM = 2i32;
pub const NormalizationKC: NORM_FORM = 5i32;
pub const NormalizationKD: NORM_FORM = 6i32;
pub const NormalizationOther: NORM_FORM = 0i32;
pub const OFFLINE_SERVICES: u32 = 2u32;
pub const ONLINE_SERVICES: u32 = 1u32;
pub const SCRIPTCONTF_FIXED_FONT: SCRIPTFONTCONTF = 1i32;
pub const SCRIPTCONTF_PROPORTIONAL_FONT: SCRIPTFONTCONTF = 2i32;
pub const SCRIPTCONTF_SCRIPT_HIDE: SCRIPTFONTCONTF = 131072i32;
pub const SCRIPTCONTF_SCRIPT_SYSTEM: SCRIPTFONTCONTF = 262144i32;
pub const SCRIPTCONTF_SCRIPT_USER: SCRIPTFONTCONTF = 65536i32;
pub const SCRIPT_DIGITSUBSTITUTE_CONTEXT: u32 = 0u32;
pub const SCRIPT_DIGITSUBSTITUTE_NATIONAL: u32 = 2u32;
pub const SCRIPT_DIGITSUBSTITUTE_NONE: u32 = 1u32;
pub const SCRIPT_DIGITSUBSTITUTE_TRADITIONAL: u32 = 3u32;
pub const SCRIPT_JUSTIFY_ARABIC_ALEF: SCRIPT_JUSTIFY = 9i32;
pub const SCRIPT_JUSTIFY_ARABIC_BA: SCRIPT_JUSTIFY = 12i32;
pub const SCRIPT_JUSTIFY_ARABIC_BARA: SCRIPT_JUSTIFY = 13i32;
pub const SCRIPT_JUSTIFY_ARABIC_BLANK: SCRIPT_JUSTIFY = 1i32;
pub const SCRIPT_JUSTIFY_ARABIC_HA: SCRIPT_JUSTIFY = 10i32;
pub const SCRIPT_JUSTIFY_ARABIC_KASHIDA: SCRIPT_JUSTIFY = 8i32;
pub const SCRIPT_JUSTIFY_ARABIC_NORMAL: SCRIPT_JUSTIFY = 7i32;
pub const SCRIPT_JUSTIFY_ARABIC_RA: SCRIPT_JUSTIFY = 11i32;
pub const SCRIPT_JUSTIFY_ARABIC_SEEN: SCRIPT_JUSTIFY = 14i32;
pub const SCRIPT_JUSTIFY_ARABIC_SEEN_M: SCRIPT_JUSTIFY = 15i32;
pub const SCRIPT_JUSTIFY_BLANK: SCRIPT_JUSTIFY = 4i32;
pub const SCRIPT_JUSTIFY_CHARACTER: SCRIPT_JUSTIFY = 2i32;
pub const SCRIPT_JUSTIFY_NONE: SCRIPT_JUSTIFY = 0i32;
pub const SCRIPT_JUSTIFY_RESERVED1: SCRIPT_JUSTIFY = 3i32;
pub const SCRIPT_JUSTIFY_RESERVED2: SCRIPT_JUSTIFY = 5i32;
pub const SCRIPT_JUSTIFY_RESERVED3: SCRIPT_JUSTIFY = 6i32;
pub const SCRIPT_TAG_UNKNOWN: u32 = 0u32;
pub const SCRIPT_UNDEFINED: u32 = 0u32;
pub const SGCM_RTL: u32 = 1u32;
pub const SIC_ASCIIDIGIT: SCRIPT_IS_COMPLEX_FLAGS = 2u32;
pub const SIC_COMPLEX: SCRIPT_IS_COMPLEX_FLAGS = 1u32;
pub const SIC_NEUTRAL: SCRIPT_IS_COMPLEX_FLAGS = 4u32;
pub const SORTING_PARADIGM_ICU: u32 = 16777216u32;
pub const SORTING_PARADIGM_NLS: u32 = 0u32;
pub const SORT_DIGITSASNUMBERS: COMPARE_STRING_FLAGS = 8u32;
pub const SORT_STRINGSORT: COMPARE_STRING_FLAGS = 4096u32;
pub const SSA_BREAK: u32 = 64u32;
pub const SSA_CLIP: u32 = 4u32;
pub const SSA_DONTGLYPH: u32 = 1073741824u32;
pub const SSA_DZWG: u32 = 16u32;
pub const SSA_FALLBACK: u32 = 32u32;
pub const SSA_FIT: u32 = 8u32;
pub const SSA_FULLMEASURE: u32 = 67108864u32;
pub const SSA_GCP: u32 = 512u32;
pub const SSA_GLYPHS: u32 = 128u32;
pub const SSA_HIDEHOTKEY: u32 = 8192u32;
pub const SSA_HOTKEY: u32 = 1024u32;
pub const SSA_HOTKEYONLY: u32 = 9216u32;
pub const SSA_LAYOUTRTL: u32 = 536870912u32;
pub const SSA_LINK: u32 = 4096u32;
pub const SSA_LPKANSIFALLBACK: u32 = 134217728u32;
pub const SSA_METAFILE: u32 = 2048u32;
pub const SSA_NOKASHIDA: u32 = 2147483648u32;
pub const SSA_PASSWORD: u32 = 1u32;
pub const SSA_PIDX: u32 = 268435456u32;
pub const SSA_RTL: u32 = 256u32;
pub const SSA_TAB: u32 = 2u32;
pub const SpellCheckerFactory: ::windows_sys::core::GUID = ::windows_sys::core::GUID::from_u128(0x7ab36653_1796_484b_bdfa_e74f1db7c1dc);
pub const TCI_SRCCHARSET: TRANSLATE_CHARSET_INFO_FLAGS = 1u32;
pub const TCI_SRCCODEPAGE: TRANSLATE_CHARSET_INFO_FLAGS = 2u32;
pub const TCI_SRCFONTSIG: TRANSLATE_CHARSET_INFO_FLAGS = 3u32;
pub const TCI_SRCLOCALE: TRANSLATE_CHARSET_INFO_FLAGS = 4096u32;
pub const TIME_FORCE24HOURFORMAT: TIME_FORMAT_FLAGS = 8u32;
pub const TIME_NOMINUTESORSECONDS: TIME_FORMAT_FLAGS = 1u32;
pub const TIME_NOSECONDS: TIME_FORMAT_FLAGS = 2u32;
pub const TIME_NOTIMEMARKER: TIME_FORMAT_FLAGS = 4u32;
pub const U16_MAX_LENGTH: u32 = 2u32;
pub const U8_LEAD3_T1_BITS: ::windows_sys::core::PCSTR = ::windows_sys::core::s!(" 000000000000\u{10}00");
pub const U8_LEAD4_T1_BITS: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{0}\u{1e}\u{f}\u{f}\u{f}\u{0}\u{0}\u{0}\u{0}");
pub const U8_MAX_LENGTH: u32 = 4u32;
pub const UBIDI_DEFAULT_LTR: u32 = 254u32;
pub const UBIDI_DEFAULT_RTL: u32 = 255u32;
pub const UBIDI_DO_MIRRORING: u32 = 2u32;
pub const UBIDI_INSERT_LRM_FOR_NUMERIC: u32 = 4u32;
pub const UBIDI_KEEP_BASE_COMBINING: u32 = 1u32;
pub const UBIDI_LEVEL_OVERRIDE: u32 = 128u32;
pub const UBIDI_LOGICAL: UBiDiOrder = 0i32;
pub const UBIDI_LTR: UBiDiDirection = 0i32;
pub const UBIDI_MAP_NOWHERE: i32 = -1i32;
pub const UBIDI_MAX_EXPLICIT_LEVEL: u32 = 125u32;
pub const UBIDI_MIRRORING_OFF: UBiDiMirroring = 0i32;
pub const UBIDI_MIRRORING_ON: UBiDiMirroring = 1i32;
pub const UBIDI_MIXED: UBiDiDirection = 2i32;
pub const UBIDI_NEUTRAL: UBiDiDirection = 3i32;
pub const UBIDI_OPTION_DEFAULT: UBiDiReorderingOption = 0i32;
pub const UBIDI_OPTION_INSERT_MARKS: UBiDiReorderingOption = 1i32;
pub const UBIDI_OPTION_REMOVE_CONTROLS: UBiDiReorderingOption = 2i32;
pub const UBIDI_OPTION_STREAMING: UBiDiReorderingOption = 4i32;
pub const UBIDI_OUTPUT_REVERSE: u32 = 16u32;
pub const UBIDI_REMOVE_BIDI_CONTROLS: u32 = 8u32;
pub const UBIDI_REORDER_DEFAULT: UBiDiReorderingMode = 0i32;
pub const UBIDI_REORDER_GROUP_NUMBERS_WITH_R: UBiDiReorderingMode = 2i32;
pub const UBIDI_REORDER_INVERSE_FOR_NUMBERS_SPECIAL: UBiDiReorderingMode = 6i32;
pub const UBIDI_REORDER_INVERSE_LIKE_DIRECT: UBiDiReorderingMode = 5i32;
pub const UBIDI_REORDER_INVERSE_NUMBERS_AS_L: UBiDiReorderingMode = 4i32;
pub const UBIDI_REORDER_NUMBERS_SPECIAL: UBiDiReorderingMode = 1i32;
pub const UBIDI_REORDER_RUNS_ONLY: UBiDiReorderingMode = 3i32;
pub const UBIDI_RTL: UBiDiDirection = 1i32;
pub const UBIDI_VISUAL: UBiDiOrder = 1i32;
pub const UBLOCK_ADLAM: UBlockCode = 263i32;
pub const UBLOCK_AEGEAN_NUMBERS: UBlockCode = 119i32;
pub const UBLOCK_AHOM: UBlockCode = 253i32;
pub const UBLOCK_ALCHEMICAL_SYMBOLS: UBlockCode = 208i32;
pub const UBLOCK_ALPHABETIC_PRESENTATION_FORMS: UBlockCode = 80i32;
pub const UBLOCK_ANATOLIAN_HIEROGLYPHS: UBlockCode = 254i32;
pub const UBLOCK_ANCIENT_GREEK_MUSICAL_NOTATION: UBlockCode = 126i32;
pub const UBLOCK_ANCIENT_GREEK_NUMBERS: UBlockCode = 127i32;
pub const UBLOCK_ANCIENT_SYMBOLS: UBlockCode = 165i32;
pub const UBLOCK_ARABIC: UBlockCode = 12i32;
pub const UBLOCK_ARABIC_EXTENDED_A: UBlockCode = 210i32;
pub const UBLOCK_ARABIC_MATHEMATICAL_ALPHABETIC_SYMBOLS: UBlockCode = 211i32;
pub const UBLOCK_ARABIC_PRESENTATION_FORMS_A: UBlockCode = 81i32;
pub const UBLOCK_ARABIC_PRESENTATION_FORMS_B: UBlockCode = 85i32;
pub const UBLOCK_ARABIC_SUPPLEMENT: UBlockCode = 128i32;
pub const UBLOCK_ARMENIAN: UBlockCode = 10i32;
pub const UBLOCK_ARROWS: UBlockCode = 46i32;
pub const UBLOCK_AVESTAN: UBlockCode = 188i32;
pub const UBLOCK_BALINESE: UBlockCode = 147i32;
pub const UBLOCK_BAMUM: UBlockCode = 177i32;
pub const UBLOCK_BAMUM_SUPPLEMENT: UBlockCode = 202i32;
pub const UBLOCK_BASIC_LATIN: UBlockCode = 1i32;
pub const UBLOCK_BASSA_VAH: UBlockCode = 221i32;
pub const UBLOCK_BATAK: UBlockCode = 199i32;
pub const UBLOCK_BENGALI: UBlockCode = 16i32;
pub const UBLOCK_BHAIKSUKI: UBlockCode = 264i32;
pub const UBLOCK_BLOCK_ELEMENTS: UBlockCode = 53i32;
pub const UBLOCK_BOPOMOFO: UBlockCode = 64i32;
pub const UBLOCK_BOPOMOFO_EXTENDED: UBlockCode = 67i32;
pub const UBLOCK_BOX_DRAWING: UBlockCode = 52i32;
pub const UBLOCK_BRAHMI: UBlockCode = 201i32;
pub const UBLOCK_BRAILLE_PATTERNS: UBlockCode = 57i32;
pub const UBLOCK_BUGINESE: UBlockCode = 129i32;
pub const UBLOCK_BUHID: UBlockCode = 100i32;
pub const UBLOCK_BYZANTINE_MUSICAL_SYMBOLS: UBlockCode = 91i32;
pub const UBLOCK_CARIAN: UBlockCode = 168i32;
pub const UBLOCK_CAUCASIAN_ALBANIAN: UBlockCode = 222i32;
pub const UBLOCK_CHAKMA: UBlockCode = 212i32;
pub const UBLOCK_CHAM: UBlockCode = 164i32;
pub const UBLOCK_CHEROKEE: UBlockCode = 32i32;
pub const UBLOCK_CHEROKEE_SUPPLEMENT: UBlockCode = 255i32;
pub const UBLOCK_CHESS_SYMBOLS: UBlockCode = 281i32;
pub const UBLOCK_CHORASMIAN: UBlockCode = 301i32;
pub const UBLOCK_CJK_COMPATIBILITY: UBlockCode = 69i32;
pub const UBLOCK_CJK_COMPATIBILITY_FORMS: UBlockCode = 83i32;
pub const UBLOCK_CJK_COMPATIBILITY_IDEOGRAPHS: UBlockCode = 79i32;
pub const UBLOCK_CJK_COMPATIBILITY_IDEOGRAPHS_SUPPLEMENT: UBlockCode = 95i32;
pub const UBLOCK_CJK_RADICALS_SUPPLEMENT: UBlockCode = 58i32;
pub const UBLOCK_CJK_STROKES: UBlockCode = 130i32;
pub const UBLOCK_CJK_SYMBOLS_AND_PUNCTUATION: UBlockCode = 61i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS: UBlockCode = 71i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_A: UBlockCode = 70i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_B: UBlockCode = 94i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_C: UBlockCode = 197i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_D: UBlockCode = 209i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_E: UBlockCode = 256i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_F: UBlockCode = 274i32;
pub const UBLOCK_CJK_UNIFIED_IDEOGRAPHS_EXTENSION_G: UBlockCode = 302i32;
pub const UBLOCK_COMBINING_DIACRITICAL_MARKS: UBlockCode = 7i32;
pub const UBLOCK_COMBINING_DIACRITICAL_MARKS_EXTENDED: UBlockCode = 224i32;
pub const UBLOCK_COMBINING_DIACRITICAL_MARKS_SUPPLEMENT: UBlockCode = 131i32;
pub const UBLOCK_COMBINING_HALF_MARKS: UBlockCode = 82i32;
pub const UBLOCK_COMBINING_MARKS_FOR_SYMBOLS: UBlockCode = 43i32;
pub const UBLOCK_COMMON_INDIC_NUMBER_FORMS: UBlockCode = 178i32;
pub const UBLOCK_CONTROL_PICTURES: UBlockCode = 49i32;
pub const UBLOCK_COPTIC: UBlockCode = 132i32;
pub const UBLOCK_COPTIC_EPACT_NUMBERS: UBlockCode = 223i32;
pub const UBLOCK_COUNTING_ROD_NUMERALS: UBlockCode = 154i32;
pub const UBLOCK_CUNEIFORM: UBlockCode = 152i32;
pub const UBLOCK_CUNEIFORM_NUMBERS_AND_PUNCTUATION: UBlockCode = 153i32;
pub const UBLOCK_CURRENCY_SYMBOLS: UBlockCode = 42i32;
pub const UBLOCK_CYPRIOT_SYLLABARY: UBlockCode = 123i32;
pub const UBLOCK_CYRILLIC: UBlockCode = 9i32;
pub const UBLOCK_CYRILLIC_EXTENDED_A: UBlockCode = 158i32;
pub const UBLOCK_CYRILLIC_EXTENDED_B: UBlockCode = 160i32;
pub const UBLOCK_CYRILLIC_EXTENDED_C: UBlockCode = 265i32;
pub const UBLOCK_CYRILLIC_SUPPLEMENT: UBlockCode = 97i32;
pub const UBLOCK_CYRILLIC_SUPPLEMENTARY: UBlockCode = 97i32;
pub const UBLOCK_DESERET: UBlockCode = 90i32;
pub const UBLOCK_DEVANAGARI: UBlockCode = 15i32;
pub const UBLOCK_DEVANAGARI_EXTENDED: UBlockCode = 179i32;
pub const UBLOCK_DINGBATS: UBlockCode = 56i32;
pub const UBLOCK_DIVES_AKURU: UBlockCode = 303i32;
pub const UBLOCK_DOGRA: UBlockCode = 282i32;
pub const UBLOCK_DOMINO_TILES: UBlockCode = 171i32;
pub const UBLOCK_DUPLOYAN: UBlockCode = 225i32;
pub const UBLOCK_EARLY_DYNASTIC_CUNEIFORM: UBlockCode = 257i32;
pub const UBLOCK_EGYPTIAN_HIEROGLYPHS: UBlockCode = 194i32;
pub const UBLOCK_EGYPTIAN_HIEROGLYPH_FORMAT_CONTROLS: UBlockCode = 292i32;
pub const UBLOCK_ELBASAN: UBlockCode = 226i32;
pub const UBLOCK_ELYMAIC: UBlockCode = 293i32;
pub const UBLOCK_EMOTICONS: UBlockCode = 206i32;
pub const UBLOCK_ENCLOSED_ALPHANUMERICS: UBlockCode = 51i32;
pub const UBLOCK_ENCLOSED_ALPHANUMERIC_SUPPLEMENT: UBlockCode = 195i32;
pub const UBLOCK_ENCLOSED_CJK_LETTERS_AND_MONTHS: UBlockCode = 68i32;
pub const UBLOCK_ENCLOSED_IDEOGRAPHIC_SUPPLEMENT: UBlockCode = 196i32;
pub const UBLOCK_ETHIOPIC: UBlockCode = 31i32;
pub const UBLOCK_ETHIOPIC_EXTENDED: UBlockCode = 133i32;
pub const UBLOCK_ETHIOPIC_EXTENDED_A: UBlockCode = 200i32;
pub const UBLOCK_ETHIOPIC_SUPPLEMENT: UBlockCode = 134i32;
pub const UBLOCK_GENERAL_PUNCTUATION: UBlockCode = 40i32;
pub const UBLOCK_GEOMETRIC_SHAPES: UBlockCode = 54i32;
pub const UBLOCK_GEOMETRIC_SHAPES_EXTENDED: UBlockCode = 227i32;
pub const UBLOCK_GEORGIAN: UBlockCode = 29i32;
pub const UBLOCK_GEORGIAN_EXTENDED: UBlockCode = 283i32;
pub const UBLOCK_GEORGIAN_SUPPLEMENT: UBlockCode = 135i32;
pub const UBLOCK_GLAGOLITIC: UBlockCode = 136i32;
pub const UBLOCK_GLAGOLITIC_SUPPLEMENT: UBlockCode = 266i32;
pub const UBLOCK_GOTHIC: UBlockCode = 89i32;
pub const UBLOCK_GRANTHA: UBlockCode = 228i32;
pub const UBLOCK_GREEK: UBlockCode = 8i32;
pub const UBLOCK_GREEK_EXTENDED: UBlockCode = 39i32;
pub const UBLOCK_GUJARATI: UBlockCode = 18i32;
pub const UBLOCK_GUNJALA_GONDI: UBlockCode = 284i32;
pub const UBLOCK_GURMUKHI: UBlockCode = 17i32;
pub const UBLOCK_HALFWIDTH_AND_FULLWIDTH_FORMS: UBlockCode = 87i32;
pub const UBLOCK_HANGUL_COMPATIBILITY_JAMO: UBlockCode = 65i32;
pub const UBLOCK_HANGUL_JAMO: UBlockCode = 30i32;
pub const UBLOCK_HANGUL_JAMO_EXTENDED_A: UBlockCode = 180i32;
pub const UBLOCK_HANGUL_JAMO_EXTENDED_B: UBlockCode = 185i32;
pub const UBLOCK_HANGUL_SYLLABLES: UBlockCode = 74i32;
pub const UBLOCK_HANIFI_ROHINGYA: UBlockCode = 285i32;
pub const UBLOCK_HANUNOO: UBlockCode = 99i32;
pub const UBLOCK_HATRAN: UBlockCode = 258i32;
pub const UBLOCK_HEBREW: UBlockCode = 11i32;
pub const UBLOCK_HIGH_PRIVATE_USE_SURROGATES: UBlockCode = 76i32;
pub const UBLOCK_HIGH_SURROGATES: UBlockCode = 75i32;
pub const UBLOCK_HIRAGANA: UBlockCode = 62i32;
pub const UBLOCK_IDEOGRAPHIC_DESCRIPTION_CHARACTERS: UBlockCode = 60i32;
pub const UBLOCK_IDEOGRAPHIC_SYMBOLS_AND_PUNCTUATION: UBlockCode = 267i32;
pub const UBLOCK_IMPERIAL_ARAMAIC: UBlockCode = 186i32;
pub const UBLOCK_INDIC_SIYAQ_NUMBERS: UBlockCode = 286i32;
pub const UBLOCK_INSCRIPTIONAL_PAHLAVI: UBlockCode = 190i32;
pub const UBLOCK_INSCRIPTIONAL_PARTHIAN: UBlockCode = 189i32;
pub const UBLOCK_INVALID_CODE: UBlockCode = -1i32;
pub const UBLOCK_IPA_EXTENSIONS: UBlockCode = 5i32;
pub const UBLOCK_JAVANESE: UBlockCode = 181i32;
pub const UBLOCK_KAITHI: UBlockCode = 193i32;
pub const UBLOCK_KANA_EXTENDED_A: UBlockCode = 275i32;
pub const UBLOCK_KANA_SUPPLEMENT: UBlockCode = 203i32;
pub const UBLOCK_KANBUN: UBlockCode = 66i32;
pub const UBLOCK_KANGXI_RADICALS: UBlockCode = 59i32;
pub const UBLOCK_KANNADA: UBlockCode = 22i32;
pub const UBLOCK_KATAKANA: UBlockCode = 63i32;
pub const UBLOCK_KATAKANA_PHONETIC_EXTENSIONS: UBlockCode = 107i32;
pub const UBLOCK_KAYAH_LI: UBlockCode = 162i32;
pub const UBLOCK_KHAROSHTHI: UBlockCode = 137i32;
pub const UBLOCK_KHITAN_SMALL_SCRIPT: UBlockCode = 304i32;
pub const UBLOCK_KHMER: UBlockCode = 36i32;
pub const UBLOCK_KHMER_SYMBOLS: UBlockCode = 113i32;
pub const UBLOCK_KHOJKI: UBlockCode = 229i32;
pub const UBLOCK_KHUDAWADI: UBlockCode = 230i32;
pub const UBLOCK_LAO: UBlockCode = 26i32;
pub const UBLOCK_LATIN_1_SUPPLEMENT: UBlockCode = 2i32;
pub const UBLOCK_LATIN_EXTENDED_A: UBlockCode = 3i32;
pub const UBLOCK_LATIN_EXTENDED_ADDITIONAL: UBlockCode = 38i32;
pub const UBLOCK_LATIN_EXTENDED_B: UBlockCode = 4i32;
pub const UBLOCK_LATIN_EXTENDED_C: UBlockCode = 148i32;
pub const UBLOCK_LATIN_EXTENDED_D: UBlockCode = 149i32;
pub const UBLOCK_LATIN_EXTENDED_E: UBlockCode = 231i32;
pub const UBLOCK_LEPCHA: UBlockCode = 156i32;
pub const UBLOCK_LETTERLIKE_SYMBOLS: UBlockCode = 44i32;
pub const UBLOCK_LIMBU: UBlockCode = 111i32;
pub const UBLOCK_LINEAR_A: UBlockCode = 232i32;
pub const UBLOCK_LINEAR_B_IDEOGRAMS: UBlockCode = 118i32;
pub const UBLOCK_LINEAR_B_SYLLABARY: UBlockCode = 117i32;
pub const UBLOCK_LISU: UBlockCode = 176i32;
pub const UBLOCK_LISU_SUPPLEMENT: UBlockCode = 305i32;
pub const UBLOCK_LOW_SURROGATES: UBlockCode = 77i32;
pub const UBLOCK_LYCIAN: UBlockCode = 167i32;
pub const UBLOCK_LYDIAN: UBlockCode = 169i32;
pub const UBLOCK_MAHAJANI: UBlockCode = 233i32;
pub const UBLOCK_MAHJONG_TILES: UBlockCode = 170i32;
pub const UBLOCK_MAKASAR: UBlockCode = 287i32;
pub const UBLOCK_MALAYALAM: UBlockCode = 23i32;
pub const UBLOCK_MANDAIC: UBlockCode = 198i32;
pub const UBLOCK_MANICHAEAN: UBlockCode = 234i32;
pub const UBLOCK_MARCHEN: UBlockCode = 268i32;
pub const UBLOCK_MASARAM_GONDI: UBlockCode = 276i32;
pub const UBLOCK_MATHEMATICAL_ALPHANUMERIC_SYMBOLS: UBlockCode = 93i32;
pub const UBLOCK_MATHEMATICAL_OPERATORS: UBlockCode = 47i32;
pub const UBLOCK_MAYAN_NUMERALS: UBlockCode = 288i32;
pub const UBLOCK_MEDEFAIDRIN: UBlockCode = 289i32;
pub const UBLOCK_MEETEI_MAYEK: UBlockCode = 184i32;
pub const UBLOCK_MEETEI_MAYEK_EXTENSIONS: UBlockCode = 213i32;
pub const UBLOCK_MENDE_KIKAKUI: UBlockCode = 235i32;
pub const UBLOCK_MEROITIC_CURSIVE: UBlockCode = 214i32;
pub const UBLOCK_MEROITIC_HIEROGLYPHS: UBlockCode = 215i32;
pub const UBLOCK_MIAO: UBlockCode = 216i32;
pub const UBLOCK_MISCELLANEOUS_MATHEMATICAL_SYMBOLS_A: UBlockCode = 102i32;
pub const UBLOCK_MISCELLANEOUS_MATHEMATICAL_SYMBOLS_B: UBlockCode = 105i32;
pub const UBLOCK_MISCELLANEOUS_SYMBOLS: UBlockCode = 55i32;
pub const UBLOCK_MISCELLANEOUS_SYMBOLS_AND_ARROWS: UBlockCode = 115i32;
pub const UBLOCK_MISCELLANEOUS_SYMBOLS_AND_PICTOGRAPHS: UBlockCode = 205i32;
pub const UBLOCK_MISCELLANEOUS_TECHNICAL: UBlockCode = 48i32;
pub const UBLOCK_MODI: UBlockCode = 236i32;
pub const UBLOCK_MODIFIER_TONE_LETTERS: UBlockCode = 138i32;
pub const UBLOCK_MONGOLIAN: UBlockCode = 37i32;
pub const UBLOCK_MONGOLIAN_SUPPLEMENT: UBlockCode = 269i32;
pub const UBLOCK_MRO: UBlockCode = 237i32;
pub const UBLOCK_MULTANI: UBlockCode = 259i32;
pub const UBLOCK_MUSICAL_SYMBOLS: UBlockCode = 92i32;
pub const UBLOCK_MYANMAR: UBlockCode = 28i32;
pub const UBLOCK_MYANMAR_EXTENDED_A: UBlockCode = 182i32;
pub const UBLOCK_MYANMAR_EXTENDED_B: UBlockCode = 238i32;
pub const UBLOCK_NABATAEAN: UBlockCode = 239i32;
pub const UBLOCK_NANDINAGARI: UBlockCode = 294i32;
pub const UBLOCK_NEWA: UBlockCode = 270i32;
pub const UBLOCK_NEW_TAI_LUE: UBlockCode = 139i32;
pub const UBLOCK_NKO: UBlockCode = 146i32;
pub const UBLOCK_NO_BLOCK: UBlockCode = 0i32;
pub const UBLOCK_NUMBER_FORMS: UBlockCode = 45i32;
pub const UBLOCK_NUSHU: UBlockCode = 277i32;
pub const UBLOCK_NYIAKENG_PUACHUE_HMONG: UBlockCode = 295i32;
pub const UBLOCK_OGHAM: UBlockCode = 34i32;
pub const UBLOCK_OLD_HUNGARIAN: UBlockCode = 260i32;
pub const UBLOCK_OLD_ITALIC: UBlockCode = 88i32;
pub const UBLOCK_OLD_NORTH_ARABIAN: UBlockCode = 240i32;
pub const UBLOCK_OLD_PERMIC: UBlockCode = 241i32;
pub const UBLOCK_OLD_PERSIAN: UBlockCode = 140i32;
pub const UBLOCK_OLD_SOGDIAN: UBlockCode = 290i32;
pub const UBLOCK_OLD_SOUTH_ARABIAN: UBlockCode = 187i32;
pub const UBLOCK_OLD_TURKIC: UBlockCode = 191i32;
pub const UBLOCK_OL_CHIKI: UBlockCode = 157i32;
pub const UBLOCK_OPTICAL_CHARACTER_RECOGNITION: UBlockCode = 50i32;
pub const UBLOCK_ORIYA: UBlockCode = 19i32;
pub const UBLOCK_ORNAMENTAL_DINGBATS: UBlockCode = 242i32;
pub const UBLOCK_OSAGE: UBlockCode = 271i32;
pub const UBLOCK_OSMANYA: UBlockCode = 122i32;
pub const UBLOCK_OTTOMAN_SIYAQ_NUMBERS: UBlockCode = 296i32;
pub const UBLOCK_PAHAWH_HMONG: UBlockCode = 243i32;
pub const UBLOCK_PALMYRENE: UBlockCode = 244i32;
pub const UBLOCK_PAU_CIN_HAU: UBlockCode = 245i32;
pub const UBLOCK_PHAGS_PA: UBlockCode = 150i32;
pub const UBLOCK_PHAISTOS_DISC: UBlockCode = 166i32;
pub const UBLOCK_PHOENICIAN: UBlockCode = 151i32;
pub const UBLOCK_PHONETIC_EXTENSIONS: UBlockCode = 114i32;
pub const UBLOCK_PHONETIC_EXTENSIONS_SUPPLEMENT: UBlockCode = 141i32;
pub const UBLOCK_PLAYING_CARDS: UBlockCode = 204i32;
pub const UBLOCK_PRIVATE_USE: UBlockCode = 78i32;
pub const UBLOCK_PRIVATE_USE_AREA: UBlockCode = 78i32;
pub const UBLOCK_PSALTER_PAHLAVI: UBlockCode = 246i32;
pub const UBLOCK_REJANG: UBlockCode = 163i32;
pub const UBLOCK_RUMI_NUMERAL_SYMBOLS: UBlockCode = 192i32;
pub const UBLOCK_RUNIC: UBlockCode = 35i32;
pub const UBLOCK_SAMARITAN: UBlockCode = 172i32;
pub const UBLOCK_SAURASHTRA: UBlockCode = 161i32;
pub const UBLOCK_SHARADA: UBlockCode = 217i32;
pub const UBLOCK_SHAVIAN: UBlockCode = 121i32;
pub const UBLOCK_SHORTHAND_FORMAT_CONTROLS: UBlockCode = 247i32;
pub const UBLOCK_SIDDHAM: UBlockCode = 248i32;
pub const UBLOCK_SINHALA: UBlockCode = 24i32;
pub const UBLOCK_SINHALA_ARCHAIC_NUMBERS: UBlockCode = 249i32;
pub const UBLOCK_SMALL_FORM_VARIANTS: UBlockCode = 84i32;
pub const UBLOCK_SMALL_KANA_EXTENSION: UBlockCode = 297i32;
pub const UBLOCK_SOGDIAN: UBlockCode = 291i32;
pub const UBLOCK_SORA_SOMPENG: UBlockCode = 218i32;
pub const UBLOCK_SOYOMBO: UBlockCode = 278i32;
pub const UBLOCK_SPACING_MODIFIER_LETTERS: UBlockCode = 6i32;
pub const UBLOCK_SPECIALS: UBlockCode = 86i32;
pub const UBLOCK_SUNDANESE: UBlockCode = 155i32;
pub const UBLOCK_SUNDANESE_SUPPLEMENT: UBlockCode = 219i32;
pub const UBLOCK_SUPERSCRIPTS_AND_SUBSCRIPTS: UBlockCode = 41i32;
pub const UBLOCK_SUPPLEMENTAL_ARROWS_A: UBlockCode = 103i32;
pub const UBLOCK_SUPPLEMENTAL_ARROWS_B: UBlockCode = 104i32;
pub const UBLOCK_SUPPLEMENTAL_ARROWS_C: UBlockCode = 250i32;
pub const UBLOCK_SUPPLEMENTAL_MATHEMATICAL_OPERATORS: UBlockCode = 106i32;
pub const UBLOCK_SUPPLEMENTAL_PUNCTUATION: UBlockCode = 142i32;
pub const UBLOCK_SUPPLEMENTAL_SYMBOLS_AND_PICTOGRAPHS: UBlockCode = 261i32;
pub const UBLOCK_SUPPLEMENTARY_PRIVATE_USE_AREA_A: UBlockCode = 109i32;
pub const UBLOCK_SUPPLEMENTARY_PRIVATE_USE_AREA_B: UBlockCode = 110i32;
pub const UBLOCK_SUTTON_SIGNWRITING: UBlockCode = 262i32;
pub const UBLOCK_SYLOTI_NAGRI: UBlockCode = 143i32;
pub const UBLOCK_SYMBOLS_AND_PICTOGRAPHS_EXTENDED_A: UBlockCode = 298i32;
pub const UBLOCK_SYMBOLS_FOR_LEGACY_COMPUTING: UBlockCode = 306i32;
pub const UBLOCK_SYRIAC: UBlockCode = 13i32;
pub const UBLOCK_SYRIAC_SUPPLEMENT: UBlockCode = 279i32;
pub const UBLOCK_TAGALOG: UBlockCode = 98i32;
pub const UBLOCK_TAGBANWA: UBlockCode = 101i32;
pub const UBLOCK_TAGS: UBlockCode = 96i32;
pub const UBLOCK_TAI_LE: UBlockCode = 112i32;
pub const UBLOCK_TAI_THAM: UBlockCode = 174i32;
pub const UBLOCK_TAI_VIET: UBlockCode = 183i32;
pub const UBLOCK_TAI_XUAN_JING_SYMBOLS: UBlockCode = 124i32;
pub const UBLOCK_TAKRI: UBlockCode = 220i32;
pub const UBLOCK_TAMIL: UBlockCode = 20i32;
pub const UBLOCK_TAMIL_SUPPLEMENT: UBlockCode = 299i32;
pub const UBLOCK_TANGUT: UBlockCode = 272i32;
pub const UBLOCK_TANGUT_COMPONENTS: UBlockCode = 273i32;
pub const UBLOCK_TANGUT_SUPPLEMENT: UBlockCode = 307i32;
pub const UBLOCK_TELUGU: UBlockCode = 21i32;
pub const UBLOCK_THAANA: UBlockCode = 14i32;
pub const UBLOCK_THAI: UBlockCode = 25i32;
pub const UBLOCK_TIBETAN: UBlockCode = 27i32;
pub const UBLOCK_TIFINAGH: UBlockCode = 144i32;
pub const UBLOCK_TIRHUTA: UBlockCode = 251i32;
pub const UBLOCK_TRANSPORT_AND_MAP_SYMBOLS: UBlockCode = 207i32;
pub const UBLOCK_UGARITIC: UBlockCode = 120i32;
pub const UBLOCK_UNIFIED_CANADIAN_ABORIGINAL_SYLLABICS: UBlockCode = 33i32;
pub const UBLOCK_UNIFIED_CANADIAN_ABORIGINAL_SYLLABICS_EXTENDED: UBlockCode = 173i32;
pub const UBLOCK_VAI: UBlockCode = 159i32;
pub const UBLOCK_VARIATION_SELECTORS: UBlockCode = 108i32;
pub const UBLOCK_VARIATION_SELECTORS_SUPPLEMENT: UBlockCode = 125i32;
pub const UBLOCK_VEDIC_EXTENSIONS: UBlockCode = 175i32;
pub const UBLOCK_VERTICAL_FORMS: UBlockCode = 145i32;
pub const UBLOCK_WANCHO: UBlockCode = 300i32;
pub const UBLOCK_WARANG_CITI: UBlockCode = 252i32;
pub const UBLOCK_YEZIDI: UBlockCode = 308i32;
pub const UBLOCK_YIJING_HEXAGRAM_SYMBOLS: UBlockCode = 116i32;
pub const UBLOCK_YI_RADICALS: UBlockCode = 73i32;
pub const UBLOCK_YI_SYLLABLES: UBlockCode = 72i32;
pub const UBLOCK_ZANABAZAR_SQUARE: UBlockCode = 280i32;
pub const UBRK_CHARACTER: UBreakIteratorType = 0i32;
pub const UBRK_LINE: UBreakIteratorType = 2i32;
pub const UBRK_LINE_HARD: ULineBreakTag = 100i32;
pub const UBRK_LINE_HARD_LIMIT: ULineBreakTag = 200i32;
pub const UBRK_LINE_SOFT: ULineBreakTag = 0i32;
pub const UBRK_LINE_SOFT_LIMIT: ULineBreakTag = 100i32;
pub const UBRK_SENTENCE: UBreakIteratorType = 3i32;
pub const UBRK_SENTENCE_SEP: USentenceBreakTag = 100i32;
pub const UBRK_SENTENCE_SEP_LIMIT: USentenceBreakTag = 200i32;
pub const UBRK_SENTENCE_TERM: USentenceBreakTag = 0i32;
pub const UBRK_SENTENCE_TERM_LIMIT: USentenceBreakTag = 100i32;
pub const UBRK_WORD: UBreakIteratorType = 1i32;
pub const UBRK_WORD_IDEO: UWordBreak = 400i32;
pub const UBRK_WORD_IDEO_LIMIT: UWordBreak = 500i32;
pub const UBRK_WORD_KANA: UWordBreak = 300i32;
pub const UBRK_WORD_KANA_LIMIT: UWordBreak = 400i32;
pub const UBRK_WORD_LETTER: UWordBreak = 200i32;
pub const UBRK_WORD_LETTER_LIMIT: UWordBreak = 300i32;
pub const UBRK_WORD_NONE: UWordBreak = 0i32;
pub const UBRK_WORD_NONE_LIMIT: UWordBreak = 100i32;
pub const UBRK_WORD_NUMBER: UWordBreak = 100i32;
pub const UBRK_WORD_NUMBER_LIMIT: UWordBreak = 200i32;
pub const UCAL_ACTUAL_MAXIMUM: UCalendarLimitType = 5i32;
pub const UCAL_ACTUAL_MINIMUM: UCalendarLimitType = 4i32;
pub const UCAL_AM: UCalendarAMPMs = 0i32;
pub const UCAL_AM_PM: UCalendarDateFields = 9i32;
pub const UCAL_APRIL: UCalendarMonths = 3i32;
pub const UCAL_AUGUST: UCalendarMonths = 7i32;
pub const UCAL_DATE: UCalendarDateFields = 5i32;
pub const UCAL_DAY_OF_MONTH: UCalendarDateFields = 5i32;
pub const UCAL_DAY_OF_WEEK: UCalendarDateFields = 7i32;
pub const UCAL_DAY_OF_WEEK_IN_MONTH: UCalendarDateFields = 8i32;
pub const UCAL_DAY_OF_YEAR: UCalendarDateFields = 6i32;
pub const UCAL_DECEMBER: UCalendarMonths = 11i32;
pub const UCAL_DEFAULT: UCalendarType = 0i32;
pub const UCAL_DOW_LOCAL: UCalendarDateFields = 18i32;
pub const UCAL_DST: UCalendarDisplayNameType = 2i32;
pub const UCAL_DST_OFFSET: UCalendarDateFields = 16i32;
pub const UCAL_ERA: UCalendarDateFields = 0i32;
pub const UCAL_EXTENDED_YEAR: UCalendarDateFields = 19i32;
pub const UCAL_FEBRUARY: UCalendarMonths = 1i32;
pub const UCAL_FIELD_COUNT: UCalendarDateFields = 23i32;
pub const UCAL_FIRST_DAY_OF_WEEK: UCalendarAttribute = 1i32;
pub const UCAL_FRIDAY: UCalendarDaysOfWeek = 6i32;
pub const UCAL_GREATEST_MINIMUM: UCalendarLimitType = 2i32;
pub const UCAL_GREGORIAN: UCalendarType = 1i32;
pub const UCAL_HOUR: UCalendarDateFields = 10i32;
pub const UCAL_HOUR_OF_DAY: UCalendarDateFields = 11i32;
pub const UCAL_IS_LEAP_MONTH: UCalendarDateFields = 22i32;
pub const UCAL_JANUARY: UCalendarMonths = 0i32;
pub const UCAL_JULIAN_DAY: UCalendarDateFields = 20i32;
pub const UCAL_JULY: UCalendarMonths = 6i32;
pub const UCAL_JUNE: UCalendarMonths = 5i32;
pub const UCAL_LEAST_MAXIMUM: UCalendarLimitType = 3i32;
pub const UCAL_LENIENT: UCalendarAttribute = 0i32;
pub const UCAL_MARCH: UCalendarMonths = 2i32;
pub const UCAL_MAXIMUM: UCalendarLimitType = 1i32;
pub const UCAL_MAY: UCalendarMonths = 4i32;
pub const UCAL_MILLISECOND: UCalendarDateFields = 14i32;
pub const UCAL_MILLISECONDS_IN_DAY: UCalendarDateFields = 21i32;
pub const UCAL_MINIMAL_DAYS_IN_FIRST_WEEK: UCalendarAttribute = 2i32;
pub const UCAL_MINIMUM: UCalendarLimitType = 0i32;
pub const UCAL_MINUTE: UCalendarDateFields = 12i32;
pub const UCAL_MONDAY: UCalendarDaysOfWeek = 2i32;
pub const UCAL_MONTH: UCalendarDateFields = 2i32;
pub const UCAL_NOVEMBER: UCalendarMonths = 10i32;
pub const UCAL_OCTOBER: UCalendarMonths = 9i32;
pub const UCAL_PM: UCalendarAMPMs = 1i32;
pub const UCAL_REPEATED_WALL_TIME: UCalendarAttribute = 3i32;
pub const UCAL_SATURDAY: UCalendarDaysOfWeek = 7i32;
pub const UCAL_SECOND: UCalendarDateFields = 13i32;
pub const UCAL_SEPTEMBER: UCalendarMonths = 8i32;
pub const UCAL_SHORT_DST: UCalendarDisplayNameType = 3i32;
pub const UCAL_SHORT_STANDARD: UCalendarDisplayNameType = 1i32;
pub const UCAL_SKIPPED_WALL_TIME: UCalendarAttribute = 4i32;
pub const UCAL_STANDARD: UCalendarDisplayNameType = 0i32;
pub const UCAL_SUNDAY: UCalendarDaysOfWeek = 1i32;
pub const UCAL_THURSDAY: UCalendarDaysOfWeek = 5i32;
pub const UCAL_TRADITIONAL: UCalendarType = 0i32;
pub const UCAL_TUESDAY: UCalendarDaysOfWeek = 3i32;
pub const UCAL_TZ_TRANSITION_NEXT: UTimeZoneTransitionType = 0i32;
pub const UCAL_TZ_TRANSITION_NEXT_INCLUSIVE: UTimeZoneTransitionType = 1i32;
pub const UCAL_TZ_TRANSITION_PREVIOUS: UTimeZoneTransitionType = 2i32;
pub const UCAL_TZ_TRANSITION_PREVIOUS_INCLUSIVE: UTimeZoneTransitionType = 3i32;
pub const UCAL_UNDECIMBER: UCalendarMonths = 12i32;
pub const UCAL_UNKNOWN_ZONE_ID: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Etc/Unknown");
pub const UCAL_WALLTIME_FIRST: UCalendarWallTimeOption = 1i32;
pub const UCAL_WALLTIME_LAST: UCalendarWallTimeOption = 0i32;
pub const UCAL_WALLTIME_NEXT_VALID: UCalendarWallTimeOption = 2i32;
pub const UCAL_WEDNESDAY: UCalendarDaysOfWeek = 4i32;
pub const UCAL_WEEKDAY: UCalendarWeekdayType = 0i32;
pub const UCAL_WEEKEND: UCalendarWeekdayType = 1i32;
pub const UCAL_WEEKEND_CEASE: UCalendarWeekdayType = 3i32;
pub const UCAL_WEEKEND_ONSET: UCalendarWeekdayType = 2i32;
pub const UCAL_WEEK_OF_MONTH: UCalendarDateFields = 4i32;
pub const UCAL_WEEK_OF_YEAR: UCalendarDateFields = 3i32;
pub const UCAL_YEAR: UCalendarDateFields = 1i32;
pub const UCAL_YEAR_WOY: UCalendarDateFields = 17i32;
pub const UCAL_ZONE_OFFSET: UCalendarDateFields = 15i32;
pub const UCAL_ZONE_TYPE_ANY: USystemTimeZoneType = 0i32;
pub const UCAL_ZONE_TYPE_CANONICAL: USystemTimeZoneType = 1i32;
pub const UCAL_ZONE_TYPE_CANONICAL_LOCATION: USystemTimeZoneType = 2i32;
pub const UCHAR_AGE: UProperty = 16384i32;
pub const UCHAR_ALPHABETIC: UProperty = 0i32;
pub const UCHAR_ASCII_HEX_DIGIT: UProperty = 1i32;
pub const UCHAR_BIDI_CLASS: UProperty = 4096i32;
pub const UCHAR_BIDI_CONTROL: UProperty = 2i32;
pub const UCHAR_BIDI_MIRRORED: UProperty = 3i32;
pub const UCHAR_BIDI_MIRRORING_GLYPH: UProperty = 16385i32;
pub const UCHAR_BIDI_PAIRED_BRACKET: UProperty = 16397i32;
pub const UCHAR_BIDI_PAIRED_BRACKET_TYPE: UProperty = 4117i32;
pub const UCHAR_BINARY_START: UProperty = 0i32;
pub const UCHAR_BLOCK: UProperty = 4097i32;
pub const UCHAR_CANONICAL_COMBINING_CLASS: UProperty = 4098i32;
pub const UCHAR_CASED: UProperty = 49i32;
pub const UCHAR_CASE_FOLDING: UProperty = 16386i32;
pub const UCHAR_CASE_IGNORABLE: UProperty = 50i32;
pub const UCHAR_CASE_SENSITIVE: UProperty = 34i32;
pub const UCHAR_CHANGES_WHEN_CASEFOLDED: UProperty = 54i32;
pub const UCHAR_CHANGES_WHEN_CASEMAPPED: UProperty = 55i32;
pub const UCHAR_CHANGES_WHEN_LOWERCASED: UProperty = 51i32;
pub const UCHAR_CHANGES_WHEN_NFKC_CASEFOLDED: UProperty = 56i32;
pub const UCHAR_CHANGES_WHEN_TITLECASED: UProperty = 53i32;
pub const UCHAR_CHANGES_WHEN_UPPERCASED: UProperty = 52i32;
pub const UCHAR_DASH: UProperty = 4i32;
pub const UCHAR_DECOMPOSITION_TYPE: UProperty = 4099i32;
pub const UCHAR_DEFAULT_IGNORABLE_CODE_POINT: UProperty = 5i32;
pub const UCHAR_DEPRECATED: UProperty = 6i32;
pub const UCHAR_DIACRITIC: UProperty = 7i32;
pub const UCHAR_DOUBLE_START: UProperty = 12288i32;
pub const UCHAR_EAST_ASIAN_WIDTH: UProperty = 4100i32;
pub const UCHAR_EMOJI: UProperty = 57i32;
pub const UCHAR_EMOJI_COMPONENT: UProperty = 61i32;
pub const UCHAR_EMOJI_MODIFIER: UProperty = 59i32;
pub const UCHAR_EMOJI_MODIFIER_BASE: UProperty = 60i32;
pub const UCHAR_EMOJI_PRESENTATION: UProperty = 58i32;
pub const UCHAR_EXTENDED_PICTOGRAPHIC: UProperty = 64i32;
pub const UCHAR_EXTENDER: UProperty = 8i32;
pub const UCHAR_FULL_COMPOSITION_EXCLUSION: UProperty = 9i32;
pub const UCHAR_GENERAL_CATEGORY: UProperty = 4101i32;
pub const UCHAR_GENERAL_CATEGORY_MASK: UProperty = 8192i32;
pub const UCHAR_GRAPHEME_BASE: UProperty = 10i32;
pub const UCHAR_GRAPHEME_CLUSTER_BREAK: UProperty = 4114i32;
pub const UCHAR_GRAPHEME_EXTEND: UProperty = 11i32;
pub const UCHAR_GRAPHEME_LINK: UProperty = 12i32;
pub const UCHAR_HANGUL_SYLLABLE_TYPE: UProperty = 4107i32;
pub const UCHAR_HEX_DIGIT: UProperty = 13i32;
pub const UCHAR_HYPHEN: UProperty = 14i32;
pub const UCHAR_IDEOGRAPHIC: UProperty = 17i32;
pub const UCHAR_IDS_BINARY_OPERATOR: UProperty = 18i32;
pub const UCHAR_IDS_TRINARY_OPERATOR: UProperty = 19i32;
pub const UCHAR_ID_CONTINUE: UProperty = 15i32;
pub const UCHAR_ID_START: UProperty = 16i32;
pub const UCHAR_INDIC_POSITIONAL_CATEGORY: UProperty = 4118i32;
pub const UCHAR_INDIC_SYLLABIC_CATEGORY: UProperty = 4119i32;
pub const UCHAR_INT_START: UProperty = 4096i32;
pub const UCHAR_INVALID_CODE: UProperty = -1i32;
pub const UCHAR_JOINING_GROUP: UProperty = 4102i32;
pub const UCHAR_JOINING_TYPE: UProperty = 4103i32;
pub const UCHAR_JOIN_CONTROL: UProperty = 20i32;
pub const UCHAR_LEAD_CANONICAL_COMBINING_CLASS: UProperty = 4112i32;
pub const UCHAR_LINE_BREAK: UProperty = 4104i32;
pub const UCHAR_LOGICAL_ORDER_EXCEPTION: UProperty = 21i32;
pub const UCHAR_LOWERCASE: UProperty = 22i32;
pub const UCHAR_LOWERCASE_MAPPING: UProperty = 16388i32;
pub const UCHAR_MASK_START: UProperty = 8192i32;
pub const UCHAR_MATH: UProperty = 23i32;
pub const UCHAR_MAX_VALUE: u32 = 1114111u32;
pub const UCHAR_MIN_VALUE: u32 = 0u32;
pub const UCHAR_NAME: UProperty = 16389i32;
pub const UCHAR_NFC_INERT: UProperty = 39i32;
pub const UCHAR_NFC_QUICK_CHECK: UProperty = 4110i32;
pub const UCHAR_NFD_INERT: UProperty = 37i32;
pub const UCHAR_NFD_QUICK_CHECK: UProperty = 4108i32;
pub const UCHAR_NFKC_INERT: UProperty = 40i32;
pub const UCHAR_NFKC_QUICK_CHECK: UProperty = 4111i32;
pub const UCHAR_NFKD_INERT: UProperty = 38i32;
pub const UCHAR_NFKD_QUICK_CHECK: UProperty = 4109i32;
pub const UCHAR_NONCHARACTER_CODE_POINT: UProperty = 24i32;
pub const UCHAR_NUMERIC_TYPE: UProperty = 4105i32;
pub const UCHAR_NUMERIC_VALUE: UProperty = 12288i32;
pub const UCHAR_OTHER_PROPERTY_START: UProperty = 28672i32;
pub const UCHAR_PATTERN_SYNTAX: UProperty = 42i32;
pub const UCHAR_PATTERN_WHITE_SPACE: UProperty = 43i32;
pub const UCHAR_POSIX_ALNUM: UProperty = 44i32;
pub const UCHAR_POSIX_BLANK: UProperty = 45i32;
pub const UCHAR_POSIX_GRAPH: UProperty = 46i32;
pub const UCHAR_POSIX_PRINT: UProperty = 47i32;
pub const UCHAR_POSIX_XDIGIT: UProperty = 48i32;
pub const UCHAR_PREPENDED_CONCATENATION_MARK: UProperty = 63i32;
pub const UCHAR_QUOTATION_MARK: UProperty = 25i32;
pub const UCHAR_RADICAL: UProperty = 26i32;
pub const UCHAR_REGIONAL_INDICATOR: UProperty = 62i32;
pub const UCHAR_SCRIPT: UProperty = 4106i32;
pub const UCHAR_SCRIPT_EXTENSIONS: UProperty = 28672i32;
pub const UCHAR_SEGMENT_STARTER: UProperty = 41i32;
pub const UCHAR_SENTENCE_BREAK: UProperty = 4115i32;
pub const UCHAR_SIMPLE_CASE_FOLDING: UProperty = 16390i32;
pub const UCHAR_SIMPLE_LOWERCASE_MAPPING: UProperty = 16391i32;
pub const UCHAR_SIMPLE_TITLECASE_MAPPING: UProperty = 16392i32;
pub const UCHAR_SIMPLE_UPPERCASE_MAPPING: UProperty = 16393i32;
pub const UCHAR_SOFT_DOTTED: UProperty = 27i32;
pub const UCHAR_STRING_START: UProperty = 16384i32;
pub const UCHAR_S_TERM: UProperty = 35i32;
pub const UCHAR_TERMINAL_PUNCTUATION: UProperty = 28i32;
pub const UCHAR_TITLECASE_MAPPING: UProperty = 16394i32;
pub const UCHAR_TRAIL_CANONICAL_COMBINING_CLASS: UProperty = 4113i32;
pub const UCHAR_UNIFIED_IDEOGRAPH: UProperty = 29i32;
pub const UCHAR_UPPERCASE: UProperty = 30i32;
pub const UCHAR_UPPERCASE_MAPPING: UProperty = 16396i32;
pub const UCHAR_VARIATION_SELECTOR: UProperty = 36i32;
pub const UCHAR_VERTICAL_ORIENTATION: UProperty = 4120i32;
pub const UCHAR_WHITE_SPACE: UProperty = 31i32;
pub const UCHAR_WORD_BREAK: UProperty = 4116i32;
pub const UCHAR_XID_CONTINUE: UProperty = 32i32;
pub const UCHAR_XID_START: UProperty = 33i32;
pub const UCLN_NO_AUTO_CLEANUP: u32 = 1u32;
pub const UCNV_BOCU1: UConverterType = 28i32;
pub const UCNV_CESU8: UConverterType = 31i32;
pub const UCNV_CLONE: UConverterCallbackReason = 5i32;
pub const UCNV_CLOSE: UConverterCallbackReason = 4i32;
pub const UCNV_COMPOUND_TEXT: UConverterType = 33i32;
pub const UCNV_DBCS: UConverterType = 1i32;
pub const UCNV_EBCDIC_STATEFUL: UConverterType = 9i32;
pub const UCNV_ESCAPE_C: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("C");
pub const UCNV_ESCAPE_CSS2: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("S");
pub const UCNV_ESCAPE_JAVA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("J");
pub const UCNV_ESCAPE_UNICODE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("U");
pub const UCNV_ESCAPE_XML_DEC: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("D");
pub const UCNV_ESCAPE_XML_HEX: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("X");
pub const UCNV_HZ: UConverterType = 23i32;
pub const UCNV_IBM: UConverterPlatform = 0i32;
pub const UCNV_ILLEGAL: UConverterCallbackReason = 1i32;
pub const UCNV_IMAP_MAILBOX: UConverterType = 32i32;
pub const UCNV_IRREGULAR: UConverterCallbackReason = 2i32;
pub const UCNV_ISCII: UConverterType = 25i32;
pub const UCNV_ISO_2022: UConverterType = 10i32;
pub const UCNV_LATIN_1: UConverterType = 3i32;
pub const UCNV_LMBCS_1: UConverterType = 11i32;
pub const UCNV_LMBCS_11: UConverterType = 18i32;
pub const UCNV_LMBCS_16: UConverterType = 19i32;
pub const UCNV_LMBCS_17: UConverterType = 20i32;
pub const UCNV_LMBCS_18: UConverterType = 21i32;
pub const UCNV_LMBCS_19: UConverterType = 22i32;
pub const UCNV_LMBCS_2: UConverterType = 12i32;
pub const UCNV_LMBCS_3: UConverterType = 13i32;
pub const UCNV_LMBCS_4: UConverterType = 14i32;
pub const UCNV_LMBCS_5: UConverterType = 15i32;
pub const UCNV_LMBCS_6: UConverterType = 16i32;
pub const UCNV_LMBCS_8: UConverterType = 17i32;
pub const UCNV_LMBCS_LAST: UConverterType = 22i32;
pub const UCNV_LOCALE_OPTION_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!(",locale=");
pub const UCNV_MAX_CONVERTER_NAME_LENGTH: u32 = 60u32;
pub const UCNV_MBCS: UConverterType = 2i32;
pub const UCNV_NUMBER_OF_SUPPORTED_CONVERTER_TYPES: UConverterType = 34i32;
pub const UCNV_OPTION_SEP_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!(",");
pub const UCNV_RESET: UConverterCallbackReason = 3i32;
pub const UCNV_ROUNDTRIP_AND_FALLBACK_SET: UConverterUnicodeSet = 1i32;
pub const UCNV_ROUNDTRIP_SET: UConverterUnicodeSet = 0i32;
pub const UCNV_SBCS: UConverterType = 0i32;
pub const UCNV_SCSU: UConverterType = 24i32;
pub const UCNV_SI: u32 = 15u32;
pub const UCNV_SKIP_STOP_ON_ILLEGAL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("i");
pub const UCNV_SO: u32 = 14u32;
pub const UCNV_SUB_STOP_ON_ILLEGAL: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("i");
pub const UCNV_SWAP_LFNL_OPTION_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!(",swaplfnl");
pub const UCNV_UNASSIGNED: UConverterCallbackReason = 0i32;
pub const UCNV_UNKNOWN: UConverterPlatform = -1i32;
pub const UCNV_UNSUPPORTED_CONVERTER: UConverterType = -1i32;
pub const UCNV_US_ASCII: UConverterType = 26i32;
pub const UCNV_UTF16: UConverterType = 29i32;
pub const UCNV_UTF16_BigEndian: UConverterType = 5i32;
pub const UCNV_UTF16_LittleEndian: UConverterType = 6i32;
pub const UCNV_UTF32: UConverterType = 30i32;
pub const UCNV_UTF32_BigEndian: UConverterType = 7i32;
pub const UCNV_UTF32_LittleEndian: UConverterType = 8i32;
pub const UCNV_UTF7: UConverterType = 27i32;
pub const UCNV_UTF8: UConverterType = 4i32;
pub const UCNV_VALUE_SEP_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("=");
pub const UCNV_VERSION_OPTION_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!(",version=");
pub const UCOL_ALTERNATE_HANDLING: UColAttribute = 1i32;
pub const UCOL_ATTRIBUTE_COUNT: UColAttribute = 8i32;
pub const UCOL_BOUND_LOWER: UColBoundMode = 0i32;
pub const UCOL_BOUND_UPPER: UColBoundMode = 1i32;
pub const UCOL_BOUND_UPPER_LONG: UColBoundMode = 2i32;
pub const UCOL_CASE_FIRST: UColAttribute = 2i32;
pub const UCOL_CASE_LEVEL: UColAttribute = 3i32;
pub const UCOL_CE_STRENGTH_LIMIT: UColAttributeValue = 3i32;
pub const UCOL_DECOMPOSITION_MODE: UColAttribute = 4i32;
pub const UCOL_DEFAULT: UColAttributeValue = -1i32;
pub const UCOL_DEFAULT_STRENGTH: UColAttributeValue = 2i32;
pub const UCOL_EQUAL: UCollationResult = 0i32;
pub const UCOL_FRENCH_COLLATION: UColAttribute = 0i32;
pub const UCOL_FULL_RULES: UColRuleOption = 1i32;
pub const UCOL_GREATER: UCollationResult = 1i32;
pub const UCOL_IDENTICAL: UColAttributeValue = 15i32;
pub const UCOL_LESS: UCollationResult = -1i32;
pub const UCOL_LOWER_FIRST: UColAttributeValue = 24i32;
pub const UCOL_NON_IGNORABLE: UColAttributeValue = 21i32;
pub const UCOL_NORMALIZATION_MODE: UColAttribute = 4i32;
pub const UCOL_NUMERIC_COLLATION: UColAttribute = 7i32;
pub const UCOL_OFF: UColAttributeValue = 16i32;
pub const UCOL_ON: UColAttributeValue = 17i32;
pub const UCOL_PRIMARY: UColAttributeValue = 0i32;
pub const UCOL_QUATERNARY: UColAttributeValue = 3i32;
pub const UCOL_REORDER_CODE_CURRENCY: UColReorderCode = 4099i32;
pub const UCOL_REORDER_CODE_DEFAULT: UColReorderCode = -1i32;
pub const UCOL_REORDER_CODE_DIGIT: UColReorderCode = 4100i32;
pub const UCOL_REORDER_CODE_FIRST: UColReorderCode = 4096i32;
pub const UCOL_REORDER_CODE_NONE: UColReorderCode = 103i32;
pub const UCOL_REORDER_CODE_OTHERS: UColReorderCode = 103i32;
pub const UCOL_REORDER_CODE_PUNCTUATION: UColReorderCode = 4097i32;
pub const UCOL_REORDER_CODE_SPACE: UColReorderCode = 4096i32;
pub const UCOL_REORDER_CODE_SYMBOL: UColReorderCode = 4098i32;
pub const UCOL_SECONDARY: UColAttributeValue = 1i32;
pub const UCOL_SHIFTED: UColAttributeValue = 20i32;
pub const UCOL_STRENGTH: UColAttribute = 5i32;
pub const UCOL_STRENGTH_LIMIT: UColAttributeValue = 16i32;
pub const UCOL_TAILORING_ONLY: UColRuleOption = 0i32;
pub const UCOL_TERTIARY: UColAttributeValue = 2i32;
pub const UCOL_UPPER_FIRST: UColAttributeValue = 25i32;
pub const UCONFIG_ENABLE_PLUGINS: u32 = 0u32;
pub const UCONFIG_FORMAT_FASTPATHS_49: u32 = 1u32;
pub const UCONFIG_HAVE_PARSEALLINPUT: u32 = 1u32;
pub const UCONFIG_NO_BREAK_ITERATION: u32 = 1u32;
pub const UCONFIG_NO_COLLATION: u32 = 1u32;
pub const UCONFIG_NO_CONVERSION: u32 = 0u32;
pub const UCONFIG_NO_FILE_IO: u32 = 0u32;
pub const UCONFIG_NO_FILTERED_BREAK_ITERATION: u32 = 0u32;
pub const UCONFIG_NO_FORMATTING: u32 = 1u32;
pub const UCONFIG_NO_IDNA: u32 = 1u32;
pub const UCONFIG_NO_LEGACY_CONVERSION: u32 = 1u32;
pub const UCONFIG_NO_NORMALIZATION: u32 = 0u32;
pub const UCONFIG_NO_REGULAR_EXPRESSIONS: u32 = 1u32;
pub const UCONFIG_NO_SERVICE: u32 = 0u32;
pub const UCONFIG_NO_TRANSLITERATION: u32 = 1u32;
pub const UCONFIG_ONLY_COLLATION: u32 = 0u32;
pub const UCONFIG_ONLY_HTML_CONVERSION: u32 = 0u32;
pub const UCPMAP_RANGE_FIXED_ALL_SURROGATES: UCPMapRangeOption = 2i32;
pub const UCPMAP_RANGE_FIXED_LEAD_SURROGATES: UCPMapRangeOption = 1i32;
pub const UCPMAP_RANGE_NORMAL: UCPMapRangeOption = 0i32;
pub const UCPTRIE_ERROR_VALUE_NEG_DATA_OFFSET: i32 = 1i32;
pub const UCPTRIE_FAST_DATA_BLOCK_LENGTH: i32 = 64i32;
pub const UCPTRIE_FAST_DATA_MASK: i32 = 63i32;
pub const UCPTRIE_FAST_SHIFT: i32 = 6i32;
pub const UCPTRIE_HIGH_VALUE_NEG_DATA_OFFSET: i32 = 2i32;
pub const UCPTRIE_SMALL_MAX: i32 = 4095i32;
pub const UCPTRIE_TYPE_ANY: UCPTrieType = -1i32;
pub const UCPTRIE_TYPE_FAST: UCPTrieType = 0i32;
pub const UCPTRIE_TYPE_SMALL: UCPTrieType = 1i32;
pub const UCPTRIE_VALUE_BITS_16: UCPTrieValueWidth = 0i32;
pub const UCPTRIE_VALUE_BITS_32: UCPTrieValueWidth = 1i32;
pub const UCPTRIE_VALUE_BITS_8: UCPTrieValueWidth = 2i32;
pub const UCPTRIE_VALUE_BITS_ANY: UCPTrieValueWidth = -1i32;
pub const UCURR_ALL: UCurrCurrencyType = 2147483647i32;
pub const UCURR_COMMON: UCurrCurrencyType = 1i32;
pub const UCURR_DEPRECATED: UCurrCurrencyType = 4i32;
pub const UCURR_LONG_NAME: UCurrNameStyle = 1i32;
pub const UCURR_NARROW_SYMBOL_NAME: UCurrNameStyle = 2i32;
pub const UCURR_NON_DEPRECATED: UCurrCurrencyType = 8i32;
pub const UCURR_SYMBOL_NAME: UCurrNameStyle = 0i32;
pub const UCURR_UNCOMMON: UCurrCurrencyType = 2i32;
pub const UCURR_USAGE_CASH: UCurrencyUsage = 1i32;
pub const UCURR_USAGE_STANDARD: UCurrencyUsage = 0i32;
pub const UDATPG_ABBREVIATED: UDateTimePGDisplayWidth = 1i32;
pub const UDATPG_BASE_CONFLICT: UDateTimePatternConflict = 1i32;
pub const UDATPG_CONFLICT: UDateTimePatternConflict = 2i32;
pub const UDATPG_DAYPERIOD_FIELD: UDateTimePatternField = 10i32;
pub const UDATPG_DAY_FIELD: UDateTimePatternField = 9i32;
pub const UDATPG_DAY_OF_WEEK_IN_MONTH_FIELD: UDateTimePatternField = 8i32;
pub const UDATPG_DAY_OF_YEAR_FIELD: UDateTimePatternField = 7i32;
pub const UDATPG_ERA_FIELD: UDateTimePatternField = 0i32;
pub const UDATPG_FIELD_COUNT: UDateTimePatternField = 16i32;
pub const UDATPG_FRACTIONAL_SECOND_FIELD: UDateTimePatternField = 14i32;
pub const UDATPG_HOUR_FIELD: UDateTimePatternField = 11i32;
pub const UDATPG_MATCH_ALL_FIELDS_LENGTH: UDateTimePatternMatchOptions = 65535i32;
pub const UDATPG_MATCH_HOUR_FIELD_LENGTH: UDateTimePatternMatchOptions = 2048i32;
pub const UDATPG_MATCH_NO_OPTIONS: UDateTimePatternMatchOptions = 0i32;
pub const UDATPG_MINUTE_FIELD: UDateTimePatternField = 12i32;
pub const UDATPG_MONTH_FIELD: UDateTimePatternField = 3i32;
pub const UDATPG_NARROW: UDateTimePGDisplayWidth = 2i32;
pub const UDATPG_NO_CONFLICT: UDateTimePatternConflict = 0i32;
pub const UDATPG_QUARTER_FIELD: UDateTimePatternField = 2i32;
pub const UDATPG_SECOND_FIELD: UDateTimePatternField = 13i32;
pub const UDATPG_WEEKDAY_FIELD: UDateTimePatternField = 6i32;
pub const UDATPG_WEEK_OF_MONTH_FIELD: UDateTimePatternField = 5i32;
pub const UDATPG_WEEK_OF_YEAR_FIELD: UDateTimePatternField = 4i32;
pub const UDATPG_WIDE: UDateTimePGDisplayWidth = 0i32;
pub const UDATPG_YEAR_FIELD: UDateTimePatternField = 1i32;
pub const UDATPG_ZONE_FIELD: UDateTimePatternField = 15i32;
pub const UDAT_ABBR_GENERIC_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("v");
pub const UDAT_ABBR_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMM");
pub const UDAT_ABBR_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMMd");
pub const UDAT_ABBR_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMMEd");
pub const UDAT_ABBR_QUARTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("QQQ");
pub const UDAT_ABBR_SPECIFIC_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("z");
pub const UDAT_ABBR_UTC_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ZZZZ");
pub const UDAT_ABBR_WEEKDAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("E");
pub const UDAT_ABSOLUTE_DAY: UDateAbsoluteUnit = 7i32;
pub const UDAT_ABSOLUTE_FRIDAY: UDateAbsoluteUnit = 5i32;
pub const UDAT_ABSOLUTE_MONDAY: UDateAbsoluteUnit = 1i32;
pub const UDAT_ABSOLUTE_MONTH: UDateAbsoluteUnit = 9i32;
pub const UDAT_ABSOLUTE_NOW: UDateAbsoluteUnit = 11i32;
pub const UDAT_ABSOLUTE_SATURDAY: UDateAbsoluteUnit = 6i32;
pub const UDAT_ABSOLUTE_SUNDAY: UDateAbsoluteUnit = 0i32;
pub const UDAT_ABSOLUTE_THURSDAY: UDateAbsoluteUnit = 4i32;
pub const UDAT_ABSOLUTE_TUESDAY: UDateAbsoluteUnit = 2i32;
pub const UDAT_ABSOLUTE_UNIT_COUNT: UDateAbsoluteUnit = 12i32;
pub const UDAT_ABSOLUTE_WEDNESDAY: UDateAbsoluteUnit = 3i32;
pub const UDAT_ABSOLUTE_WEEK: UDateAbsoluteUnit = 8i32;
pub const UDAT_ABSOLUTE_YEAR: UDateAbsoluteUnit = 10i32;
pub const UDAT_AM_PMS: UDateFormatSymbolType = 5i32;
pub const UDAT_AM_PM_FIELD: UDateFormatField = 14i32;
pub const UDAT_AM_PM_MIDNIGHT_NOON_FIELD: UDateFormatField = 35i32;
pub const UDAT_BOOLEAN_ATTRIBUTE_COUNT: UDateFormatBooleanAttribute = 4i32;
pub const UDAT_CYCLIC_YEARS_ABBREVIATED: UDateFormatSymbolType = 23i32;
pub const UDAT_CYCLIC_YEARS_NARROW: UDateFormatSymbolType = 24i32;
pub const UDAT_CYCLIC_YEARS_WIDE: UDateFormatSymbolType = 22i32;
pub const UDAT_DATE_FIELD: UDateFormatField = 3i32;
pub const UDAT_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("d");
pub const UDAT_DAY_OF_WEEK_FIELD: UDateFormatField = 9i32;
pub const UDAT_DAY_OF_WEEK_IN_MONTH_FIELD: UDateFormatField = 11i32;
pub const UDAT_DAY_OF_YEAR_FIELD: UDateFormatField = 10i32;
pub const UDAT_DEFAULT: UDateFormatStyle = 2i32;
pub const UDAT_DIRECTION_COUNT: UDateDirection = 6i32;
pub const UDAT_DIRECTION_LAST: UDateDirection = 1i32;
pub const UDAT_DIRECTION_LAST_2: UDateDirection = 0i32;
pub const UDAT_DIRECTION_NEXT: UDateDirection = 3i32;
pub const UDAT_DIRECTION_NEXT_2: UDateDirection = 4i32;
pub const UDAT_DIRECTION_PLAIN: UDateDirection = 5i32;
pub const UDAT_DIRECTION_THIS: UDateDirection = 2i32;
pub const UDAT_DOW_LOCAL_FIELD: UDateFormatField = 19i32;
pub const UDAT_ERAS: UDateFormatSymbolType = 0i32;
pub const UDAT_ERA_FIELD: UDateFormatField = 0i32;
pub const UDAT_ERA_NAMES: UDateFormatSymbolType = 7i32;
pub const UDAT_EXTENDED_YEAR_FIELD: UDateFormatField = 20i32;
pub const UDAT_FLEXIBLE_DAY_PERIOD_FIELD: UDateFormatField = 36i32;
pub const UDAT_FRACTIONAL_SECOND_FIELD: UDateFormatField = 8i32;
pub const UDAT_FULL: UDateFormatStyle = 0i32;
pub const UDAT_FULL_RELATIVE: UDateFormatStyle = 128i32;
pub const UDAT_GENERIC_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("vvvv");
pub const UDAT_HOUR: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("j");
pub const UDAT_HOUR0_FIELD: UDateFormatField = 16i32;
pub const UDAT_HOUR1_FIELD: UDateFormatField = 15i32;
pub const UDAT_HOUR24: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("H");
pub const UDAT_HOUR24_MINUTE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Hm");
pub const UDAT_HOUR24_MINUTE_SECOND: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Hms");
pub const UDAT_HOUR_MINUTE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("jm");
pub const UDAT_HOUR_MINUTE_SECOND: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("jms");
pub const UDAT_HOUR_OF_DAY0_FIELD: UDateFormatField = 5i32;
pub const UDAT_HOUR_OF_DAY1_FIELD: UDateFormatField = 4i32;
pub const UDAT_JULIAN_DAY_FIELD: UDateFormatField = 21i32;
pub const UDAT_LOCALIZED_CHARS: UDateFormatSymbolType = 6i32;
pub const UDAT_LOCATION_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("VVVV");
pub const UDAT_LONG: UDateFormatStyle = 1i32;
pub const UDAT_LONG_RELATIVE: UDateFormatStyle = 129i32;
pub const UDAT_MEDIUM: UDateFormatStyle = 2i32;
pub const UDAT_MEDIUM_RELATIVE: UDateFormatStyle = 130i32;
pub const UDAT_MILLISECONDS_IN_DAY_FIELD: UDateFormatField = 22i32;
pub const UDAT_MINUTE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("m");
pub const UDAT_MINUTE_FIELD: UDateFormatField = 6i32;
pub const UDAT_MINUTE_SECOND: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ms");
pub const UDAT_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMMM");
pub const UDAT_MONTHS: UDateFormatSymbolType = 1i32;
pub const UDAT_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMMMd");
pub const UDAT_MONTH_FIELD: UDateFormatField = 2i32;
pub const UDAT_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MMMMEEEEd");
pub const UDAT_NARROW_MONTHS: UDateFormatSymbolType = 8i32;
pub const UDAT_NARROW_WEEKDAYS: UDateFormatSymbolType = 9i32;
pub const UDAT_NONE: UDateFormatStyle = -1i32;
pub const UDAT_NUM_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("M");
pub const UDAT_NUM_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("Md");
pub const UDAT_NUM_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("MEd");
pub const UDAT_PARSE_ALLOW_NUMERIC: UDateFormatBooleanAttribute = 1i32;
pub const UDAT_PARSE_ALLOW_WHITESPACE: UDateFormatBooleanAttribute = 0i32;
pub const UDAT_PARSE_MULTIPLE_PATTERNS_FOR_MATCH: UDateFormatBooleanAttribute = 3i32;
pub const UDAT_PARSE_PARTIAL_LITERAL_MATCH: UDateFormatBooleanAttribute = 2i32;
pub const UDAT_PATTERN: UDateFormatStyle = -2i32;
pub const UDAT_QUARTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("QQQQ");
pub const UDAT_QUARTERS: UDateFormatSymbolType = 16i32;
pub const UDAT_QUARTER_FIELD: UDateFormatField = 27i32;
pub const UDAT_RELATIVE: UDateFormatStyle = 128i32;
pub const UDAT_RELATIVE_DAYS: UDateRelativeUnit = 3i32;
pub const UDAT_RELATIVE_HOURS: UDateRelativeUnit = 2i32;
pub const UDAT_RELATIVE_MINUTES: UDateRelativeUnit = 1i32;
pub const UDAT_RELATIVE_MONTHS: UDateRelativeUnit = 5i32;
pub const UDAT_RELATIVE_SECONDS: UDateRelativeUnit = 0i32;
pub const UDAT_RELATIVE_UNIT_COUNT: UDateRelativeUnit = 7i32;
pub const UDAT_RELATIVE_WEEKS: UDateRelativeUnit = 4i32;
pub const UDAT_RELATIVE_YEARS: UDateRelativeUnit = 6i32;
pub const UDAT_REL_LITERAL_FIELD: URelativeDateTimeFormatterField = 0i32;
pub const UDAT_REL_NUMERIC_FIELD: URelativeDateTimeFormatterField = 1i32;
pub const UDAT_REL_UNIT_DAY: URelativeDateTimeUnit = 4i32;
pub const UDAT_REL_UNIT_FRIDAY: URelativeDateTimeUnit = 13i32;
pub const UDAT_REL_UNIT_HOUR: URelativeDateTimeUnit = 5i32;
pub const UDAT_REL_UNIT_MINUTE: URelativeDateTimeUnit = 6i32;
pub const UDAT_REL_UNIT_MONDAY: URelativeDateTimeUnit = 9i32;
pub const UDAT_REL_UNIT_MONTH: URelativeDateTimeUnit = 2i32;
pub const UDAT_REL_UNIT_QUARTER: URelativeDateTimeUnit = 1i32;
pub const UDAT_REL_UNIT_SATURDAY: URelativeDateTimeUnit = 14i32;
pub const UDAT_REL_UNIT_SECOND: URelativeDateTimeUnit = 7i32;
pub const UDAT_REL_UNIT_SUNDAY: URelativeDateTimeUnit = 8i32;
pub const UDAT_REL_UNIT_THURSDAY: URelativeDateTimeUnit = 12i32;
pub const UDAT_REL_UNIT_TUESDAY: URelativeDateTimeUnit = 10i32;
pub const UDAT_REL_UNIT_WEDNESDAY: URelativeDateTimeUnit = 11i32;
pub const UDAT_REL_UNIT_WEEK: URelativeDateTimeUnit = 3i32;
pub const UDAT_REL_UNIT_YEAR: URelativeDateTimeUnit = 0i32;
pub const UDAT_SECOND: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("s");
pub const UDAT_SECOND_FIELD: UDateFormatField = 7i32;
pub const UDAT_SHORT: UDateFormatStyle = 3i32;
pub const UDAT_SHORTER_WEEKDAYS: UDateFormatSymbolType = 20i32;
pub const UDAT_SHORT_MONTHS: UDateFormatSymbolType = 2i32;
pub const UDAT_SHORT_QUARTERS: UDateFormatSymbolType = 17i32;
pub const UDAT_SHORT_RELATIVE: UDateFormatStyle = 131i32;
pub const UDAT_SHORT_WEEKDAYS: UDateFormatSymbolType = 4i32;
pub const UDAT_SPECIFIC_TZ: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zzzz");
pub const UDAT_STANDALONE_DAY_FIELD: UDateFormatField = 25i32;
pub const UDAT_STANDALONE_MONTHS: UDateFormatSymbolType = 10i32;
pub const UDAT_STANDALONE_MONTH_FIELD: UDateFormatField = 26i32;
pub const UDAT_STANDALONE_NARROW_MONTHS: UDateFormatSymbolType = 12i32;
pub const UDAT_STANDALONE_NARROW_WEEKDAYS: UDateFormatSymbolType = 15i32;
pub const UDAT_STANDALONE_QUARTERS: UDateFormatSymbolType = 18i32;
pub const UDAT_STANDALONE_QUARTER_FIELD: UDateFormatField = 28i32;
pub const UDAT_STANDALONE_SHORTER_WEEKDAYS: UDateFormatSymbolType = 21i32;
pub const UDAT_STANDALONE_SHORT_MONTHS: UDateFormatSymbolType = 11i32;
pub const UDAT_STANDALONE_SHORT_QUARTERS: UDateFormatSymbolType = 19i32;
pub const UDAT_STANDALONE_SHORT_WEEKDAYS: UDateFormatSymbolType = 14i32;
pub const UDAT_STANDALONE_WEEKDAYS: UDateFormatSymbolType = 13i32;
pub const UDAT_STYLE_LONG: UDateRelativeDateTimeFormatterStyle = 0i32;
pub const UDAT_STYLE_NARROW: UDateRelativeDateTimeFormatterStyle = 2i32;
pub const UDAT_STYLE_SHORT: UDateRelativeDateTimeFormatterStyle = 1i32;
pub const UDAT_TIMEZONE_FIELD: UDateFormatField = 17i32;
pub const UDAT_TIMEZONE_GENERIC_FIELD: UDateFormatField = 24i32;
pub const UDAT_TIMEZONE_ISO_FIELD: UDateFormatField = 32i32;
pub const UDAT_TIMEZONE_ISO_LOCAL_FIELD: UDateFormatField = 33i32;
pub const UDAT_TIMEZONE_LOCALIZED_GMT_OFFSET_FIELD: UDateFormatField = 31i32;
pub const UDAT_TIMEZONE_RFC_FIELD: UDateFormatField = 23i32;
pub const UDAT_TIMEZONE_SPECIAL_FIELD: UDateFormatField = 29i32;
pub const UDAT_WEEKDAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("EEEE");
pub const UDAT_WEEKDAYS: UDateFormatSymbolType = 3i32;
pub const UDAT_WEEK_OF_MONTH_FIELD: UDateFormatField = 13i32;
pub const UDAT_WEEK_OF_YEAR_FIELD: UDateFormatField = 12i32;
pub const UDAT_YEAR: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("y");
pub const UDAT_YEAR_ABBR_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMM");
pub const UDAT_YEAR_ABBR_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMMd");
pub const UDAT_YEAR_ABBR_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMMEd");
pub const UDAT_YEAR_ABBR_QUARTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yQQQ");
pub const UDAT_YEAR_FIELD: UDateFormatField = 1i32;
pub const UDAT_YEAR_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMMM");
pub const UDAT_YEAR_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMMMd");
pub const UDAT_YEAR_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMMMMEEEEd");
pub const UDAT_YEAR_NAME_FIELD: UDateFormatField = 30i32;
pub const UDAT_YEAR_NUM_MONTH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yM");
pub const UDAT_YEAR_NUM_MONTH_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMd");
pub const UDAT_YEAR_NUM_MONTH_WEEKDAY_DAY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yMEd");
pub const UDAT_YEAR_QUARTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("yQQQQ");
pub const UDAT_YEAR_WOY_FIELD: UDateFormatField = 18i32;
pub const UDAT_ZODIAC_NAMES_ABBREVIATED: UDateFormatSymbolType = 26i32;
pub const UDAT_ZODIAC_NAMES_NARROW: UDateFormatSymbolType = 27i32;
pub const UDAT_ZODIAC_NAMES_WIDE: UDateFormatSymbolType = 25i32;
pub const UDISPCTX_CAPITALIZATION_FOR_BEGINNING_OF_SENTENCE: UDisplayContext = 258i32;
pub const UDISPCTX_CAPITALIZATION_FOR_MIDDLE_OF_SENTENCE: UDisplayContext = 257i32;
pub const UDISPCTX_CAPITALIZATION_FOR_STANDALONE: UDisplayContext = 260i32;
pub const UDISPCTX_CAPITALIZATION_FOR_UI_LIST_OR_MENU: UDisplayContext = 259i32;
pub const UDISPCTX_CAPITALIZATION_NONE: UDisplayContext = 256i32;
pub const UDISPCTX_DIALECT_NAMES: UDisplayContext = 1i32;
pub const UDISPCTX_LENGTH_FULL: UDisplayContext = 512i32;
pub const UDISPCTX_LENGTH_SHORT: UDisplayContext = 513i32;
pub const UDISPCTX_NO_SUBSTITUTE: UDisplayContext = 769i32;
pub const UDISPCTX_STANDARD_NAMES: UDisplayContext = 0i32;
pub const UDISPCTX_SUBSTITUTE: UDisplayContext = 768i32;
pub const UDISPCTX_TYPE_CAPITALIZATION: UDisplayContextType = 1i32;
pub const UDISPCTX_TYPE_DIALECT_HANDLING: UDisplayContextType = 0i32;
pub const UDISPCTX_TYPE_DISPLAY_LENGTH: UDisplayContextType = 2i32;
pub const UDISPCTX_TYPE_SUBSTITUTE_HANDLING: UDisplayContextType = 3i32;
pub const UDTS_DB2_TIME: UDateTimeScale = 8i32;
pub const UDTS_DOTNET_DATE_TIME: UDateTimeScale = 4i32;
pub const UDTS_EXCEL_TIME: UDateTimeScale = 7i32;
pub const UDTS_ICU4C_TIME: UDateTimeScale = 2i32;
pub const UDTS_JAVA_TIME: UDateTimeScale = 0i32;
pub const UDTS_MAC_OLD_TIME: UDateTimeScale = 5i32;
pub const UDTS_MAC_TIME: UDateTimeScale = 6i32;
pub const UDTS_UNIX_MICROSECONDS_TIME: UDateTimeScale = 9i32;
pub const UDTS_UNIX_TIME: UDateTimeScale = 1i32;
pub const UDTS_WINDOWS_FILE_TIME: UDateTimeScale = 3i32;
pub const UFIELD_CATEGORY_DATE: UFieldCategory = 1i32;
pub const UFIELD_CATEGORY_DATE_INTERVAL: UFieldCategory = 5i32;
pub const UFIELD_CATEGORY_DATE_INTERVAL_SPAN: UFieldCategory = 4101i32;
pub const UFIELD_CATEGORY_LIST: UFieldCategory = 3i32;
pub const UFIELD_CATEGORY_LIST_SPAN: UFieldCategory = 4099i32;
pub const UFIELD_CATEGORY_NUMBER: UFieldCategory = 2i32;
pub const UFIELD_CATEGORY_RELATIVE_DATETIME: UFieldCategory = 4i32;
pub const UFIELD_CATEGORY_UNDEFINED: UFieldCategory = 0i32;
pub const UFMT_ARRAY: UFormattableType = 4i32;
pub const UFMT_DATE: UFormattableType = 0i32;
pub const UFMT_DOUBLE: UFormattableType = 1i32;
pub const UFMT_INT64: UFormattableType = 5i32;
pub const UFMT_LONG: UFormattableType = 2i32;
pub const UFMT_OBJECT: UFormattableType = 6i32;
pub const UFMT_STRING: UFormattableType = 3i32;
pub const UGENDER_FEMALE: UGender = 1i32;
pub const UGENDER_MALE: UGender = 0i32;
pub const UGENDER_OTHER: UGender = 2i32;
pub const UIDNA_CHECK_BIDI: i32 = 4i32;
pub const UIDNA_CHECK_CONTEXTJ: i32 = 8i32;
pub const UIDNA_CHECK_CONTEXTO: i32 = 64i32;
pub const UIDNA_DEFAULT: i32 = 0i32;
pub const UIDNA_ERROR_BIDI: i32 = 2048i32;
pub const UIDNA_ERROR_CONTEXTJ: i32 = 4096i32;
pub const UIDNA_ERROR_CONTEXTO_DIGITS: i32 = 16384i32;
pub const UIDNA_ERROR_CONTEXTO_PUNCTUATION: i32 = 8192i32;
pub const UIDNA_ERROR_DISALLOWED: i32 = 128i32;
pub const UIDNA_ERROR_DOMAIN_NAME_TOO_LONG: i32 = 4i32;
pub const UIDNA_ERROR_EMPTY_LABEL: i32 = 1i32;
pub const UIDNA_ERROR_HYPHEN_3_4: i32 = 32i32;
pub const UIDNA_ERROR_INVALID_ACE_LABEL: i32 = 1024i32;
pub const UIDNA_ERROR_LABEL_HAS_DOT: i32 = 512i32;
pub const UIDNA_ERROR_LABEL_TOO_LONG: i32 = 2i32;
pub const UIDNA_ERROR_LEADING_COMBINING_MARK: i32 = 64i32;
pub const UIDNA_ERROR_LEADING_HYPHEN: i32 = 8i32;
pub const UIDNA_ERROR_PUNYCODE: i32 = 256i32;
pub const UIDNA_ERROR_TRAILING_HYPHEN: i32 = 16i32;
pub const UIDNA_NONTRANSITIONAL_TO_ASCII: i32 = 16i32;
pub const UIDNA_NONTRANSITIONAL_TO_UNICODE: i32 = 32i32;
pub const UIDNA_USE_STD3_RULES: i32 = 2i32;
pub const UITER_CURRENT: UCharIteratorOrigin = 1i32;
pub const UITER_LENGTH: UCharIteratorOrigin = 4i32;
pub const UITER_LIMIT: UCharIteratorOrigin = 2i32;
pub const UITER_START: UCharIteratorOrigin = 0i32;
pub const UITER_UNKNOWN_INDEX: i32 = -2i32;
pub const UITER_ZERO: UCharIteratorOrigin = 3i32;
pub const ULDN_DIALECT_NAMES: UDialectHandling = 1i32;
pub const ULDN_STANDARD_NAMES: UDialectHandling = 0i32;
pub const ULISTFMT_ELEMENT_FIELD: UListFormatterField = 1i32;
pub const ULISTFMT_LITERAL_FIELD: UListFormatterField = 0i32;
pub const ULISTFMT_TYPE_AND: UListFormatterType = 0i32;
pub const ULISTFMT_TYPE_OR: UListFormatterType = 1i32;
pub const ULISTFMT_TYPE_UNITS: UListFormatterType = 2i32;
pub const ULISTFMT_WIDTH_NARROW: UListFormatterWidth = 2i32;
pub const ULISTFMT_WIDTH_SHORT: UListFormatterWidth = 1i32;
pub const ULISTFMT_WIDTH_WIDE: UListFormatterWidth = 0i32;
pub const ULOCDATA_ALT_QUOTATION_END: ULocaleDataDelimiterType = 3i32;
pub const ULOCDATA_ALT_QUOTATION_START: ULocaleDataDelimiterType = 2i32;
pub const ULOCDATA_ES_AUXILIARY: ULocaleDataExemplarSetType = 1i32;
pub const ULOCDATA_ES_INDEX: ULocaleDataExemplarSetType = 2i32;
pub const ULOCDATA_ES_PUNCTUATION: ULocaleDataExemplarSetType = 3i32;
pub const ULOCDATA_ES_STANDARD: ULocaleDataExemplarSetType = 0i32;
pub const ULOCDATA_QUOTATION_END: ULocaleDataDelimiterType = 1i32;
pub const ULOCDATA_QUOTATION_START: ULocaleDataDelimiterType = 0i32;
pub const ULOC_ACCEPT_FAILED: UAcceptResult = 0i32;
pub const ULOC_ACCEPT_FALLBACK: UAcceptResult = 2i32;
pub const ULOC_ACCEPT_VALID: UAcceptResult = 1i32;
pub const ULOC_ACTUAL_LOCALE: ULocDataLocaleType = 0i32;
pub const ULOC_AVAILABLE_DEFAULT: ULocAvailableType = 0i32;
pub const ULOC_AVAILABLE_ONLY_LEGACY_ALIASES: ULocAvailableType = 1i32;
pub const ULOC_AVAILABLE_WITH_LEGACY_ALIASES: ULocAvailableType = 2i32;
pub const ULOC_CANADA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("en_CA");
pub const ULOC_CANADA_FRENCH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("fr_CA");
pub const ULOC_CHINA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh_CN");
pub const ULOC_CHINESE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh");
pub const ULOC_COUNTRY_CAPACITY: u32 = 4u32;
pub const ULOC_ENGLISH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("en");
pub const ULOC_FRANCE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("fr_FR");
pub const ULOC_FRENCH: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("fr");
pub const ULOC_FULLNAME_CAPACITY: u32 = 157u32;
pub const ULOC_GERMAN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("de");
pub const ULOC_GERMANY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("de_DE");
pub const ULOC_ITALIAN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("it");
pub const ULOC_ITALY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("it_IT");
pub const ULOC_JAPAN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ja_JP");
pub const ULOC_JAPANESE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ja");
pub const ULOC_KEYWORDS_CAPACITY: u32 = 96u32;
pub const ULOC_KEYWORD_AND_VALUES_CAPACITY: u32 = 100u32;
pub const ULOC_KEYWORD_ASSIGN_UNICODE: u32 = 61u32;
pub const ULOC_KEYWORD_ITEM_SEPARATOR_UNICODE: u32 = 59u32;
pub const ULOC_KEYWORD_SEPARATOR_UNICODE: u32 = 64u32;
pub const ULOC_KOREA: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ko_KR");
pub const ULOC_KOREAN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("ko");
pub const ULOC_LANG_CAPACITY: u32 = 12u32;
pub const ULOC_LAYOUT_BTT: ULayoutType = 3i32;
pub const ULOC_LAYOUT_LTR: ULayoutType = 0i32;
pub const ULOC_LAYOUT_RTL: ULayoutType = 1i32;
pub const ULOC_LAYOUT_TTB: ULayoutType = 2i32;
pub const ULOC_LAYOUT_UNKNOWN: ULayoutType = 4i32;
pub const ULOC_PRC: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh_CN");
pub const ULOC_SCRIPT_CAPACITY: u32 = 6u32;
pub const ULOC_SIMPLIFIED_CHINESE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh_CN");
pub const ULOC_TAIWAN: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh_TW");
pub const ULOC_TRADITIONAL_CHINESE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("zh_TW");
pub const ULOC_UK: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("en_GB");
pub const ULOC_US: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("en_US");
pub const ULOC_VALID_LOCALE: ULocDataLocaleType = 1i32;
pub const UMEASFMT_WIDTH_COUNT: UMeasureFormatWidth = 4i32;
pub const UMEASFMT_WIDTH_NARROW: UMeasureFormatWidth = 2i32;
pub const UMEASFMT_WIDTH_NUMERIC: UMeasureFormatWidth = 3i32;
pub const UMEASFMT_WIDTH_SHORT: UMeasureFormatWidth = 1i32;
pub const UMEASFMT_WIDTH_WIDE: UMeasureFormatWidth = 0i32;
pub const UMSGPAT_APOS_DOUBLE_OPTIONAL: UMessagePatternApostropheMode = 0i32;
pub const UMSGPAT_APOS_DOUBLE_REQUIRED: UMessagePatternApostropheMode = 1i32;
pub const UMSGPAT_ARG_NAME_NOT_NUMBER: i32 = -1i32;
pub const UMSGPAT_ARG_NAME_NOT_VALID: i32 = -2i32;
pub const UMSGPAT_ARG_TYPE_CHOICE: UMessagePatternArgType = 2i32;
pub const UMSGPAT_ARG_TYPE_NONE: UMessagePatternArgType = 0i32;
pub const UMSGPAT_ARG_TYPE_PLURAL: UMessagePatternArgType = 3i32;
pub const UMSGPAT_ARG_TYPE_SELECT: UMessagePatternArgType = 4i32;
pub const UMSGPAT_ARG_TYPE_SELECTORDINAL: UMessagePatternArgType = 5i32;
pub const UMSGPAT_ARG_TYPE_SIMPLE: UMessagePatternArgType = 1i32;
pub const UMSGPAT_PART_TYPE_ARG_DOUBLE: UMessagePatternPartType = 13i32;
pub const UMSGPAT_PART_TYPE_ARG_INT: UMessagePatternPartType = 12i32;
pub const UMSGPAT_PART_TYPE_ARG_LIMIT: UMessagePatternPartType = 6i32;
pub const UMSGPAT_PART_TYPE_ARG_NAME: UMessagePatternPartType = 8i32;
pub const UMSGPAT_PART_TYPE_ARG_NUMBER: UMessagePatternPartType = 7i32;
pub const UMSGPAT_PART_TYPE_ARG_SELECTOR: UMessagePatternPartType = 11i32;
pub const UMSGPAT_PART_TYPE_ARG_START: UMessagePatternPartType = 5i32;
pub const UMSGPAT_PART_TYPE_ARG_STYLE: UMessagePatternPartType = 10i32;
pub const UMSGPAT_PART_TYPE_ARG_TYPE: UMessagePatternPartType = 9i32;
pub const UMSGPAT_PART_TYPE_INSERT_CHAR: UMessagePatternPartType = 3i32;
pub const UMSGPAT_PART_TYPE_MSG_LIMIT: UMessagePatternPartType = 1i32;
pub const UMSGPAT_PART_TYPE_MSG_START: UMessagePatternPartType = 0i32;
pub const UMSGPAT_PART_TYPE_REPLACE_NUMBER: UMessagePatternPartType = 4i32;
pub const UMSGPAT_PART_TYPE_SKIP_SYNTAX: UMessagePatternPartType = 2i32;
pub const UMS_SI: UMeasurementSystem = 0i32;
pub const UMS_UK: UMeasurementSystem = 2i32;
pub const UMS_US: UMeasurementSystem = 1i32;
pub const UNISCRIBE_OPENTYPE: u32 = 256u32;
pub const UNORM2_COMPOSE: UNormalization2Mode = 0i32;
pub const UNORM2_COMPOSE_CONTIGUOUS: UNormalization2Mode = 3i32;
pub const UNORM2_DECOMPOSE: UNormalization2Mode = 1i32;
pub const UNORM2_FCD: UNormalization2Mode = 2i32;
pub const UNORM_DEFAULT: UNormalizationMode = 4i32;
pub const UNORM_FCD: UNormalizationMode = 6i32;
pub const UNORM_INPUT_IS_FCD: u32 = 131072u32;
pub const UNORM_MAYBE: UNormalizationCheckResult = 2i32;
pub const UNORM_MODE_COUNT: UNormalizationMode = 7i32;
pub const UNORM_NFC: UNormalizationMode = 4i32;
pub const UNORM_NFD: UNormalizationMode = 2i32;
pub const UNORM_NFKC: UNormalizationMode = 5i32;
pub const UNORM_NFKD: UNormalizationMode = 3i32;
pub const UNORM_NO: UNormalizationCheckResult = 0i32;
pub const UNORM_NONE: UNormalizationMode = 1i32;
pub const UNORM_YES: UNormalizationCheckResult = 1i32;
pub const UNUM_CASH_CURRENCY: UNumberFormatStyle = 13i32;
pub const UNUM_COMPACT_FIELD: UNumberFormatFields = 12i32;
pub const UNUM_CURRENCY: UNumberFormatStyle = 2i32;
pub const UNUM_CURRENCY_ACCOUNTING: UNumberFormatStyle = 12i32;
pub const UNUM_CURRENCY_CODE: UNumberFormatTextAttribute = 5i32;
pub const UNUM_CURRENCY_FIELD: UNumberFormatFields = 7i32;
pub const UNUM_CURRENCY_INSERT: UCurrencySpacing = 2i32;
pub const UNUM_CURRENCY_ISO: UNumberFormatStyle = 10i32;
pub const UNUM_CURRENCY_MATCH: UCurrencySpacing = 0i32;
pub const UNUM_CURRENCY_PLURAL: UNumberFormatStyle = 11i32;
pub const UNUM_CURRENCY_SPACING_COUNT: UCurrencySpacing = 3i32;
pub const UNUM_CURRENCY_STANDARD: UNumberFormatStyle = 16i32;
pub const UNUM_CURRENCY_SURROUNDING_MATCH: UCurrencySpacing = 1i32;
pub const UNUM_CURRENCY_SYMBOL: UNumberFormatSymbol = 8i32;
pub const UNUM_CURRENCY_USAGE: UNumberFormatAttribute = 23i32;
pub const UNUM_DECIMAL: UNumberFormatStyle = 1i32;
pub const UNUM_DECIMAL_ALWAYS_SHOWN: UNumberFormatAttribute = 2i32;
pub const UNUM_DECIMAL_COMPACT_LONG: UNumberFormatStyle = 15i32;
pub const UNUM_DECIMAL_COMPACT_SHORT: UNumberFormatStyle = 14i32;
pub const UNUM_DECIMAL_SEPARATOR_ALWAYS: UNumberDecimalSeparatorDisplay = 1i32;
pub const UNUM_DECIMAL_SEPARATOR_AUTO: UNumberDecimalSeparatorDisplay = 0i32;
pub const UNUM_DECIMAL_SEPARATOR_COUNT: UNumberDecimalSeparatorDisplay = 2i32;
pub const UNUM_DECIMAL_SEPARATOR_FIELD: UNumberFormatFields = 2i32;
pub const UNUM_DECIMAL_SEPARATOR_SYMBOL: UNumberFormatSymbol = 0i32;
pub const UNUM_DEFAULT: UNumberFormatStyle = 1i32;
pub const UNUM_DEFAULT_RULESET: UNumberFormatTextAttribute = 6i32;
pub const UNUM_DIGIT_SYMBOL: UNumberFormatSymbol = 5i32;
pub const UNUM_DURATION: UNumberFormatStyle = 7i32;
pub const UNUM_EIGHT_DIGIT_SYMBOL: UNumberFormatSymbol = 25i32;
pub const UNUM_EXPONENTIAL_SYMBOL: UNumberFormatSymbol = 11i32;
pub const UNUM_EXPONENT_FIELD: UNumberFormatFields = 5i32;
pub const UNUM_EXPONENT_MULTIPLICATION_SYMBOL: UNumberFormatSymbol = 27i32;
pub const UNUM_EXPONENT_SIGN_FIELD: UNumberFormatFields = 4i32;
pub const UNUM_EXPONENT_SYMBOL_FIELD: UNumberFormatFields = 3i32;
pub const UNUM_FIVE_DIGIT_SYMBOL: UNumberFormatSymbol = 22i32;
pub const UNUM_FORMAT_ATTRIBUTE_VALUE_HIDDEN: UNumberFormatAttributeValue = 0i32;
pub const UNUM_FORMAT_FAIL_IF_MORE_THAN_MAX_DIGITS: UNumberFormatAttribute = 4096i32;
pub const UNUM_FORMAT_WIDTH: UNumberFormatAttribute = 13i32;
pub const UNUM_FOUR_DIGIT_SYMBOL: UNumberFormatSymbol = 21i32;
pub const UNUM_FRACTION_DIGITS: UNumberFormatAttribute = 8i32;
pub const UNUM_FRACTION_FIELD: UNumberFormatFields = 1i32;
pub const UNUM_GROUPING_AUTO: UNumberGroupingStrategy = 2i32;
pub const UNUM_GROUPING_MIN2: UNumberGroupingStrategy = 1i32;
pub const UNUM_GROUPING_OFF: UNumberGroupingStrategy = 0i32;
pub const UNUM_GROUPING_ON_ALIGNED: UNumberGroupingStrategy = 3i32;
pub const UNUM_GROUPING_SEPARATOR_FIELD: UNumberFormatFields = 6i32;
pub const UNUM_GROUPING_SEPARATOR_SYMBOL: UNumberFormatSymbol = 1i32;
pub const UNUM_GROUPING_SIZE: UNumberFormatAttribute = 10i32;
pub const UNUM_GROUPING_THOUSANDS: UNumberGroupingStrategy = 4i32;
pub const UNUM_GROUPING_USED: UNumberFormatAttribute = 1i32;
pub const UNUM_IDENTITY_FALLBACK_APPROXIMATELY: UNumberRangeIdentityFallback = 2i32;
pub const UNUM_IDENTITY_FALLBACK_APPROXIMATELY_OR_SINGLE_VALUE: UNumberRangeIdentityFallback = 1i32;
pub const UNUM_IDENTITY_FALLBACK_RANGE: UNumberRangeIdentityFallback = 3i32;
pub const UNUM_IDENTITY_FALLBACK_SINGLE_VALUE: UNumberRangeIdentityFallback = 0i32;
pub const UNUM_IDENTITY_RESULT_EQUAL_AFTER_ROUNDING: UNumberRangeIdentityResult = 1i32;
pub const UNUM_IDENTITY_RESULT_EQUAL_BEFORE_ROUNDING: UNumberRangeIdentityResult = 0i32;
pub const UNUM_IDENTITY_RESULT_NOT_EQUAL: UNumberRangeIdentityResult = 2i32;
pub const UNUM_IGNORE: UNumberFormatStyle = 0i32;
pub const UNUM_INFINITY_SYMBOL: UNumberFormatSymbol = 14i32;
pub const UNUM_INTEGER_DIGITS: UNumberFormatAttribute = 5i32;
pub const UNUM_INTEGER_FIELD: UNumberFormatFields = 0i32;
pub const UNUM_INTL_CURRENCY_SYMBOL: UNumberFormatSymbol = 9i32;
pub const UNUM_LENIENT_PARSE: UNumberFormatAttribute = 19i32;
pub const UNUM_LONG: UNumberCompactStyle = 1i32;
pub const UNUM_MAX_FRACTION_DIGITS: UNumberFormatAttribute = 6i32;
pub const UNUM_MAX_INTEGER_DIGITS: UNumberFormatAttribute = 3i32;
pub const UNUM_MAX_SIGNIFICANT_DIGITS: UNumberFormatAttribute = 18i32;
pub const UNUM_MEASURE_UNIT_FIELD: UNumberFormatFields = 11i32;
pub const UNUM_MINIMUM_GROUPING_DIGITS: UNumberFormatAttribute = 22i32;
pub const UNUM_MINUS_SIGN_SYMBOL: UNumberFormatSymbol = 6i32;
pub const UNUM_MIN_FRACTION_DIGITS: UNumberFormatAttribute = 7i32;
pub const UNUM_MIN_INTEGER_DIGITS: UNumberFormatAttribute = 4i32;
pub const UNUM_MIN_SIGNIFICANT_DIGITS: UNumberFormatAttribute = 17i32;
pub const UNUM_MONETARY_GROUPING_SEPARATOR_SYMBOL: UNumberFormatSymbol = 17i32;
pub const UNUM_MONETARY_SEPARATOR_SYMBOL: UNumberFormatSymbol = 10i32;
pub const UNUM_MULTIPLIER: UNumberFormatAttribute = 9i32;
pub const UNUM_NAN_SYMBOL: UNumberFormatSymbol = 15i32;
pub const UNUM_NEGATIVE_PREFIX: UNumberFormatTextAttribute = 2i32;
pub const UNUM_NEGATIVE_SUFFIX: UNumberFormatTextAttribute = 3i32;
pub const UNUM_NINE_DIGIT_SYMBOL: UNumberFormatSymbol = 26i32;
pub const UNUM_NUMBERING_SYSTEM: UNumberFormatStyle = 8i32;
pub const UNUM_ONE_DIGIT_SYMBOL: UNumberFormatSymbol = 18i32;
pub const UNUM_ORDINAL: UNumberFormatStyle = 6i32;
pub const UNUM_PADDING_CHARACTER: UNumberFormatTextAttribute = 4i32;
pub const UNUM_PADDING_POSITION: UNumberFormatAttribute = 14i32;
pub const UNUM_PAD_AFTER_PREFIX: UNumberFormatPadPosition = 1i32;
pub const UNUM_PAD_AFTER_SUFFIX: UNumberFormatPadPosition = 3i32;
pub const UNUM_PAD_BEFORE_PREFIX: UNumberFormatPadPosition = 0i32;
pub const UNUM_PAD_BEFORE_SUFFIX: UNumberFormatPadPosition = 2i32;
pub const UNUM_PAD_ESCAPE_SYMBOL: UNumberFormatSymbol = 13i32;
pub const UNUM_PARSE_ALL_INPUT: UNumberFormatAttribute = 20i32;
pub const UNUM_PARSE_CASE_SENSITIVE: UNumberFormatAttribute = 4099i32;
pub const UNUM_PARSE_DECIMAL_MARK_REQUIRED: UNumberFormatAttribute = 4098i32;
pub const UNUM_PARSE_INT_ONLY: UNumberFormatAttribute = 0i32;
pub const UNUM_PARSE_NO_EXPONENT: UNumberFormatAttribute = 4097i32;
pub const UNUM_PATTERN_DECIMAL: UNumberFormatStyle = 0i32;
pub const UNUM_PATTERN_RULEBASED: UNumberFormatStyle = 9i32;
pub const UNUM_PATTERN_SEPARATOR_SYMBOL: UNumberFormatSymbol = 2i32;
pub const UNUM_PERCENT: UNumberFormatStyle = 3i32;
pub const UNUM_PERCENT_FIELD: UNumberFormatFields = 8i32;
pub const UNUM_PERCENT_SYMBOL: UNumberFormatSymbol = 3i32;
pub const UNUM_PERMILL_FIELD: UNumberFormatFields = 9i32;
pub const UNUM_PERMILL_SYMBOL: UNumberFormatSymbol = 12i32;
pub const UNUM_PLUS_SIGN_SYMBOL: UNumberFormatSymbol = 7i32;
pub const UNUM_POSITIVE_PREFIX: UNumberFormatTextAttribute = 0i32;
pub const UNUM_POSITIVE_SUFFIX: UNumberFormatTextAttribute = 1i32;
pub const UNUM_PUBLIC_RULESETS: UNumberFormatTextAttribute = 7i32;
pub const UNUM_RANGE_COLLAPSE_ALL: UNumberRangeCollapse = 3i32;
pub const UNUM_RANGE_COLLAPSE_AUTO: UNumberRangeCollapse = 0i32;
pub const UNUM_RANGE_COLLAPSE_NONE: UNumberRangeCollapse = 1i32;
pub const UNUM_RANGE_COLLAPSE_UNIT: UNumberRangeCollapse = 2i32;
pub const UNUM_ROUNDING_INCREMENT: UNumberFormatAttribute = 12i32;
pub const UNUM_ROUNDING_MODE: UNumberFormatAttribute = 11i32;
pub const UNUM_ROUND_CEILING: UNumberFormatRoundingMode = 0i32;
pub const UNUM_ROUND_DOWN: UNumberFormatRoundingMode = 2i32;
pub const UNUM_ROUND_FLOOR: UNumberFormatRoundingMode = 1i32;
pub const UNUM_ROUND_HALFDOWN: UNumberFormatRoundingMode = 5i32;
pub const UNUM_ROUND_HALFEVEN: UNumberFormatRoundingMode = 4i32;
pub const UNUM_ROUND_HALFUP: UNumberFormatRoundingMode = 6i32;
pub const UNUM_ROUND_UNNECESSARY: UNumberFormatRoundingMode = 7i32;
pub const UNUM_ROUND_UP: UNumberFormatRoundingMode = 3i32;
pub const UNUM_SCALE: UNumberFormatAttribute = 21i32;
pub const UNUM_SCIENTIFIC: UNumberFormatStyle = 4i32;
pub const UNUM_SECONDARY_GROUPING_SIZE: UNumberFormatAttribute = 15i32;
pub const UNUM_SEVEN_DIGIT_SYMBOL: UNumberFormatSymbol = 24i32;
pub const UNUM_SHORT: UNumberCompactStyle = 0i32;
pub const UNUM_SIGNIFICANT_DIGITS_USED: UNumberFormatAttribute = 16i32;
pub const UNUM_SIGNIFICANT_DIGIT_SYMBOL: UNumberFormatSymbol = 16i32;
pub const UNUM_SIGN_ACCOUNTING: UNumberSignDisplay = 3i32;
pub const UNUM_SIGN_ACCOUNTING_ALWAYS: UNumberSignDisplay = 4i32;
pub const UNUM_SIGN_ACCOUNTING_EXCEPT_ZERO: UNumberSignDisplay = 6i32;
pub const UNUM_SIGN_ALWAYS: UNumberSignDisplay = 1i32;
pub const UNUM_SIGN_ALWAYS_SHOWN: UNumberFormatAttribute = 4100i32;
pub const UNUM_SIGN_AUTO: UNumberSignDisplay = 0i32;
pub const UNUM_SIGN_COUNT: UNumberSignDisplay = 7i32;
pub const UNUM_SIGN_EXCEPT_ZERO: UNumberSignDisplay = 5i32;
pub const UNUM_SIGN_FIELD: UNumberFormatFields = 10i32;
pub const UNUM_SIGN_NEVER: UNumberSignDisplay = 2i32;
pub const UNUM_SIX_DIGIT_SYMBOL: UNumberFormatSymbol = 23i32;
pub const UNUM_SPELLOUT: UNumberFormatStyle = 5i32;
pub const UNUM_THREE_DIGIT_SYMBOL: UNumberFormatSymbol = 20i32;
pub const UNUM_TWO_DIGIT_SYMBOL: UNumberFormatSymbol = 19i32;
pub const UNUM_UNIT_WIDTH_COUNT: UNumberUnitWidth = 5i32;
pub const UNUM_UNIT_WIDTH_FULL_NAME: UNumberUnitWidth = 2i32;
pub const UNUM_UNIT_WIDTH_HIDDEN: UNumberUnitWidth = 4i32;
pub const UNUM_UNIT_WIDTH_ISO_CODE: UNumberUnitWidth = 3i32;
pub const UNUM_UNIT_WIDTH_NARROW: UNumberUnitWidth = 0i32;
pub const UNUM_UNIT_WIDTH_SHORT: UNumberUnitWidth = 1i32;
pub const UNUM_ZERO_DIGIT_SYMBOL: UNumberFormatSymbol = 4i32;
pub const UPLURAL_TYPE_CARDINAL: UPluralType = 0i32;
pub const UPLURAL_TYPE_ORDINAL: UPluralType = 1i32;
pub const UREGEX_CASE_INSENSITIVE: URegexpFlag = 2i32;
pub const UREGEX_COMMENTS: URegexpFlag = 4i32;
pub const UREGEX_DOTALL: URegexpFlag = 32i32;
pub const UREGEX_ERROR_ON_UNKNOWN_ESCAPES: URegexpFlag = 512i32;
pub const UREGEX_LITERAL: URegexpFlag = 16i32;
pub const UREGEX_MULTILINE: URegexpFlag = 8i32;
pub const UREGEX_UNIX_LINES: URegexpFlag = 1i32;
pub const UREGEX_UWORD: URegexpFlag = 256i32;
pub const URES_ALIAS: UResType = 3i32;
pub const URES_ARRAY: UResType = 8i32;
pub const URES_BINARY: UResType = 1i32;
pub const URES_INT: UResType = 7i32;
pub const URES_INT_VECTOR: UResType = 14i32;
pub const URES_NONE: UResType = -1i32;
pub const URES_STRING: UResType = 0i32;
pub const URES_TABLE: UResType = 2i32;
pub const URGN_CONTINENT: URegionType = 3i32;
pub const URGN_DEPRECATED: URegionType = 6i32;
pub const URGN_GROUPING: URegionType = 5i32;
pub const URGN_SUBCONTINENT: URegionType = 4i32;
pub const URGN_TERRITORY: URegionType = 1i32;
pub const URGN_UNKNOWN: URegionType = 0i32;
pub const URGN_WORLD: URegionType = 2i32;
pub const USCRIPT_ADLAM: UScriptCode = 167i32;
pub const USCRIPT_AFAKA: UScriptCode = 147i32;
pub const USCRIPT_AHOM: UScriptCode = 161i32;
pub const USCRIPT_ANATOLIAN_HIEROGLYPHS: UScriptCode = 156i32;
pub const USCRIPT_ARABIC: UScriptCode = 2i32;
pub const USCRIPT_ARMENIAN: UScriptCode = 3i32;
pub const USCRIPT_AVESTAN: UScriptCode = 117i32;
pub const USCRIPT_BALINESE: UScriptCode = 62i32;
pub const USCRIPT_BAMUM: UScriptCode = 130i32;
pub const USCRIPT_BASSA_VAH: UScriptCode = 134i32;
pub const USCRIPT_BATAK: UScriptCode = 63i32;
pub const USCRIPT_BENGALI: UScriptCode = 4i32;
pub const USCRIPT_BHAIKSUKI: UScriptCode = 168i32;
pub const USCRIPT_BLISSYMBOLS: UScriptCode = 64i32;
pub const USCRIPT_BOOK_PAHLAVI: UScriptCode = 124i32;
pub const USCRIPT_BOPOMOFO: UScriptCode = 5i32;
pub const USCRIPT_BRAHMI: UScriptCode = 65i32;
pub const USCRIPT_BRAILLE: UScriptCode = 46i32;
pub const USCRIPT_BUGINESE: UScriptCode = 55i32;
pub const USCRIPT_BUHID: UScriptCode = 44i32;
pub const USCRIPT_CANADIAN_ABORIGINAL: UScriptCode = 40i32;
pub const USCRIPT_CARIAN: UScriptCode = 104i32;
pub const USCRIPT_CAUCASIAN_ALBANIAN: UScriptCode = 159i32;
pub const USCRIPT_CHAKMA: UScriptCode = 118i32;
pub const USCRIPT_CHAM: UScriptCode = 66i32;
pub const USCRIPT_CHEROKEE: UScriptCode = 6i32;
pub const USCRIPT_CHORASMIAN: UScriptCode = 189i32;
pub const USCRIPT_CIRTH: UScriptCode = 67i32;
pub const USCRIPT_COMMON: UScriptCode = 0i32;
pub const USCRIPT_COPTIC: UScriptCode = 7i32;
pub const USCRIPT_CUNEIFORM: UScriptCode = 101i32;
pub const USCRIPT_CYPRIOT: UScriptCode = 47i32;
pub const USCRIPT_CYRILLIC: UScriptCode = 8i32;
pub const USCRIPT_DEMOTIC_EGYPTIAN: UScriptCode = 69i32;
pub const USCRIPT_DESERET: UScriptCode = 9i32;
pub const USCRIPT_DEVANAGARI: UScriptCode = 10i32;
pub const USCRIPT_DIVES_AKURU: UScriptCode = 190i32;
pub const USCRIPT_DOGRA: UScriptCode = 178i32;
pub const USCRIPT_DUPLOYAN: UScriptCode = 135i32;
pub const USCRIPT_EASTERN_SYRIAC: UScriptCode = 97i32;
pub const USCRIPT_EGYPTIAN_HIEROGLYPHS: UScriptCode = 71i32;
pub const USCRIPT_ELBASAN: UScriptCode = 136i32;
pub const USCRIPT_ELYMAIC: UScriptCode = 185i32;
pub const USCRIPT_ESTRANGELO_SYRIAC: UScriptCode = 95i32;
pub const USCRIPT_ETHIOPIC: UScriptCode = 11i32;
pub const USCRIPT_GEORGIAN: UScriptCode = 12i32;
pub const USCRIPT_GLAGOLITIC: UScriptCode = 56i32;
pub const USCRIPT_GOTHIC: UScriptCode = 13i32;
pub const USCRIPT_GRANTHA: UScriptCode = 137i32;
pub const USCRIPT_GREEK: UScriptCode = 14i32;
pub const USCRIPT_GUJARATI: UScriptCode = 15i32;
pub const USCRIPT_GUNJALA_GONDI: UScriptCode = 179i32;
pub const USCRIPT_GURMUKHI: UScriptCode = 16i32;
pub const USCRIPT_HAN: UScriptCode = 17i32;
pub const USCRIPT_HANGUL: UScriptCode = 18i32;
pub const USCRIPT_HANIFI_ROHINGYA: UScriptCode = 182i32;
pub const USCRIPT_HANUNOO: UScriptCode = 43i32;
pub const USCRIPT_HAN_WITH_BOPOMOFO: UScriptCode = 172i32;
pub const USCRIPT_HARAPPAN_INDUS: UScriptCode = 77i32;
pub const USCRIPT_HATRAN: UScriptCode = 162i32;
pub const USCRIPT_HEBREW: UScriptCode = 19i32;
pub const USCRIPT_HIERATIC_EGYPTIAN: UScriptCode = 70i32;
pub const USCRIPT_HIRAGANA: UScriptCode = 20i32;
pub const USCRIPT_IMPERIAL_ARAMAIC: UScriptCode = 116i32;
pub const USCRIPT_INHERITED: UScriptCode = 1i32;
pub const USCRIPT_INSCRIPTIONAL_PAHLAVI: UScriptCode = 122i32;
pub const USCRIPT_INSCRIPTIONAL_PARTHIAN: UScriptCode = 125i32;
pub const USCRIPT_INVALID_CODE: UScriptCode = -1i32;
pub const USCRIPT_JAMO: UScriptCode = 173i32;
pub const USCRIPT_JAPANESE: UScriptCode = 105i32;
pub const USCRIPT_JAVANESE: UScriptCode = 78i32;
pub const USCRIPT_JURCHEN: UScriptCode = 148i32;
pub const USCRIPT_KAITHI: UScriptCode = 120i32;
pub const USCRIPT_KANNADA: UScriptCode = 21i32;
pub const USCRIPT_KATAKANA: UScriptCode = 22i32;
pub const USCRIPT_KATAKANA_OR_HIRAGANA: UScriptCode = 54i32;
pub const USCRIPT_KAYAH_LI: UScriptCode = 79i32;
pub const USCRIPT_KHAROSHTHI: UScriptCode = 57i32;
pub const USCRIPT_KHITAN_SMALL_SCRIPT: UScriptCode = 191i32;
pub const USCRIPT_KHMER: UScriptCode = 23i32;
pub const USCRIPT_KHOJKI: UScriptCode = 157i32;
pub const USCRIPT_KHUDAWADI: UScriptCode = 145i32;
pub const USCRIPT_KHUTSURI: UScriptCode = 72i32;
pub const USCRIPT_KOREAN: UScriptCode = 119i32;
pub const USCRIPT_KPELLE: UScriptCode = 138i32;
pub const USCRIPT_LANNA: UScriptCode = 106i32;
pub const USCRIPT_LAO: UScriptCode = 24i32;
pub const USCRIPT_LATIN: UScriptCode = 25i32;
pub const USCRIPT_LATIN_FRAKTUR: UScriptCode = 80i32;
pub const USCRIPT_LATIN_GAELIC: UScriptCode = 81i32;
pub const USCRIPT_LEPCHA: UScriptCode = 82i32;
pub const USCRIPT_LIMBU: UScriptCode = 48i32;
pub const USCRIPT_LINEAR_A: UScriptCode = 83i32;
pub const USCRIPT_LINEAR_B: UScriptCode = 49i32;
pub const USCRIPT_LISU: UScriptCode = 131i32;
pub const USCRIPT_LOMA: UScriptCode = 139i32;
pub const USCRIPT_LYCIAN: UScriptCode = 107i32;
pub const USCRIPT_LYDIAN: UScriptCode = 108i32;
pub const USCRIPT_MAHAJANI: UScriptCode = 160i32;
pub const USCRIPT_MAKASAR: UScriptCode = 180i32;
pub const USCRIPT_MALAYALAM: UScriptCode = 26i32;
pub const USCRIPT_MANDAEAN: UScriptCode = 84i32;
pub const USCRIPT_MANDAIC: UScriptCode = 84i32;
pub const USCRIPT_MANICHAEAN: UScriptCode = 121i32;
pub const USCRIPT_MARCHEN: UScriptCode = 169i32;
pub const USCRIPT_MASARAM_GONDI: UScriptCode = 175i32;
pub const USCRIPT_MATHEMATICAL_NOTATION: UScriptCode = 128i32;
pub const USCRIPT_MAYAN_HIEROGLYPHS: UScriptCode = 85i32;
pub const USCRIPT_MEDEFAIDRIN: UScriptCode = 181i32;
pub const USCRIPT_MEITEI_MAYEK: UScriptCode = 115i32;
pub const USCRIPT_MENDE: UScriptCode = 140i32;
pub const USCRIPT_MEROITIC: UScriptCode = 86i32;
pub const USCRIPT_MEROITIC_CURSIVE: UScriptCode = 141i32;
pub const USCRIPT_MEROITIC_HIEROGLYPHS: UScriptCode = 86i32;
pub const USCRIPT_MIAO: UScriptCode = 92i32;
pub const USCRIPT_MODI: UScriptCode = 163i32;
pub const USCRIPT_MONGOLIAN: UScriptCode = 27i32;
pub const USCRIPT_MOON: UScriptCode = 114i32;
pub const USCRIPT_MRO: UScriptCode = 149i32;
pub const USCRIPT_MULTANI: UScriptCode = 164i32;
pub const USCRIPT_MYANMAR: UScriptCode = 28i32;
pub const USCRIPT_NABATAEAN: UScriptCode = 143i32;
pub const USCRIPT_NAKHI_GEBA: UScriptCode = 132i32;
pub const USCRIPT_NANDINAGARI: UScriptCode = 187i32;
pub const USCRIPT_NEWA: UScriptCode = 170i32;
pub const USCRIPT_NEW_TAI_LUE: UScriptCode = 59i32;
pub const USCRIPT_NKO: UScriptCode = 87i32;
pub const USCRIPT_NUSHU: UScriptCode = 150i32;
pub const USCRIPT_NYIAKENG_PUACHUE_HMONG: UScriptCode = 186i32;
pub const USCRIPT_OGHAM: UScriptCode = 29i32;
pub const USCRIPT_OLD_CHURCH_SLAVONIC_CYRILLIC: UScriptCode = 68i32;
pub const USCRIPT_OLD_HUNGARIAN: UScriptCode = 76i32;
pub const USCRIPT_OLD_ITALIC: UScriptCode = 30i32;
pub const USCRIPT_OLD_NORTH_ARABIAN: UScriptCode = 142i32;
pub const USCRIPT_OLD_PERMIC: UScriptCode = 89i32;
pub const USCRIPT_OLD_PERSIAN: UScriptCode = 61i32;
pub const USCRIPT_OLD_SOGDIAN: UScriptCode = 184i32;
pub const USCRIPT_OLD_SOUTH_ARABIAN: UScriptCode = 133i32;
pub const USCRIPT_OL_CHIKI: UScriptCode = 109i32;
pub const USCRIPT_ORIYA: UScriptCode = 31i32;
pub const USCRIPT_ORKHON: UScriptCode = 88i32;
pub const USCRIPT_OSAGE: UScriptCode = 171i32;
pub const USCRIPT_OSMANYA: UScriptCode = 50i32;
pub const USCRIPT_PAHAWH_HMONG: UScriptCode = 75i32;
pub const USCRIPT_PALMYRENE: UScriptCode = 144i32;
pub const USCRIPT_PAU_CIN_HAU: UScriptCode = 165i32;
pub const USCRIPT_PHAGS_PA: UScriptCode = 90i32;
pub const USCRIPT_PHOENICIAN: UScriptCode = 91i32;
pub const USCRIPT_PHONETIC_POLLARD: UScriptCode = 92i32;
pub const USCRIPT_PSALTER_PAHLAVI: UScriptCode = 123i32;
pub const USCRIPT_REJANG: UScriptCode = 110i32;
pub const USCRIPT_RONGORONGO: UScriptCode = 93i32;
pub const USCRIPT_RUNIC: UScriptCode = 32i32;
pub const USCRIPT_SAMARITAN: UScriptCode = 126i32;
pub const USCRIPT_SARATI: UScriptCode = 94i32;
pub const USCRIPT_SAURASHTRA: UScriptCode = 111i32;
pub const USCRIPT_SHARADA: UScriptCode = 151i32;
pub const USCRIPT_SHAVIAN: UScriptCode = 51i32;
pub const USCRIPT_SIDDHAM: UScriptCode = 166i32;
pub const USCRIPT_SIGN_WRITING: UScriptCode = 112i32;
pub const USCRIPT_SIMPLIFIED_HAN: UScriptCode = 73i32;
pub const USCRIPT_SINDHI: UScriptCode = 145i32;
pub const USCRIPT_SINHALA: UScriptCode = 33i32;
pub const USCRIPT_SOGDIAN: UScriptCode = 183i32;
pub const USCRIPT_SORA_SOMPENG: UScriptCode = 152i32;
pub const USCRIPT_SOYOMBO: UScriptCode = 176i32;
pub const USCRIPT_SUNDANESE: UScriptCode = 113i32;
pub const USCRIPT_SYLOTI_NAGRI: UScriptCode = 58i32;
pub const USCRIPT_SYMBOLS: UScriptCode = 129i32;
pub const USCRIPT_SYMBOLS_EMOJI: UScriptCode = 174i32;
pub const USCRIPT_SYRIAC: UScriptCode = 34i32;
pub const USCRIPT_TAGALOG: UScriptCode = 42i32;
pub const USCRIPT_TAGBANWA: UScriptCode = 45i32;
pub const USCRIPT_TAI_LE: UScriptCode = 52i32;
pub const USCRIPT_TAI_VIET: UScriptCode = 127i32;
pub const USCRIPT_TAKRI: UScriptCode = 153i32;
pub const USCRIPT_TAMIL: UScriptCode = 35i32;
pub const USCRIPT_TANGUT: UScriptCode = 154i32;
pub const USCRIPT_TELUGU: UScriptCode = 36i32;
pub const USCRIPT_TENGWAR: UScriptCode = 98i32;
pub const USCRIPT_THAANA: UScriptCode = 37i32;
pub const USCRIPT_THAI: UScriptCode = 38i32;
pub const USCRIPT_TIBETAN: UScriptCode = 39i32;
pub const USCRIPT_TIFINAGH: UScriptCode = 60i32;
pub const USCRIPT_TIRHUTA: UScriptCode = 158i32;
pub const USCRIPT_TRADITIONAL_HAN: UScriptCode = 74i32;
pub const USCRIPT_UCAS: UScriptCode = 40i32;
pub const USCRIPT_UGARITIC: UScriptCode = 53i32;
pub const USCRIPT_UNKNOWN: UScriptCode = 103i32;
pub const USCRIPT_UNWRITTEN_LANGUAGES: UScriptCode = 102i32;
pub const USCRIPT_USAGE_ASPIRATIONAL: UScriptUsage = 4i32;
pub const USCRIPT_USAGE_EXCLUDED: UScriptUsage = 2i32;
pub const USCRIPT_USAGE_LIMITED_USE: UScriptUsage = 3i32;
pub const USCRIPT_USAGE_NOT_ENCODED: UScriptUsage = 0i32;
pub const USCRIPT_USAGE_RECOMMENDED: UScriptUsage = 5i32;
pub const USCRIPT_USAGE_UNKNOWN: UScriptUsage = 1i32;
pub const USCRIPT_VAI: UScriptCode = 99i32;
pub const USCRIPT_VISIBLE_SPEECH: UScriptCode = 100i32;
pub const USCRIPT_WANCHO: UScriptCode = 188i32;
pub const USCRIPT_WARANG_CITI: UScriptCode = 146i32;
pub const USCRIPT_WESTERN_SYRIAC: UScriptCode = 96i32;
pub const USCRIPT_WOLEAI: UScriptCode = 155i32;
pub const USCRIPT_YEZIDI: UScriptCode = 192i32;
pub const USCRIPT_YI: UScriptCode = 41i32;
pub const USCRIPT_ZANABAZAR_SQUARE: UScriptCode = 177i32;
pub const USEARCH_ANY_BASE_WEIGHT_IS_WILDCARD: USearchAttributeValue = 4i32;
pub const USEARCH_DEFAULT: USearchAttributeValue = -1i32;
pub const USEARCH_DONE: i32 = -1i32;
pub const USEARCH_ELEMENT_COMPARISON: USearchAttribute = 2i32;
pub const USEARCH_OFF: USearchAttributeValue = 0i32;
pub const USEARCH_ON: USearchAttributeValue = 1i32;
pub const USEARCH_OVERLAP: USearchAttribute = 0i32;
pub const USEARCH_PATTERN_BASE_WEIGHT_IS_WILDCARD: USearchAttributeValue = 3i32;
pub const USEARCH_STANDARD_ELEMENT_COMPARISON: USearchAttributeValue = 2i32;
pub const USET_ADD_CASE_MAPPINGS: i32 = 4i32;
pub const USET_CASE_INSENSITIVE: i32 = 2i32;
pub const USET_IGNORE_SPACE: i32 = 1i32;
pub const USET_SERIALIZED_STATIC_ARRAY_CAPACITY: i32 = 8i32;
pub const USET_SPAN_CONTAINED: USetSpanCondition = 1i32;
pub const USET_SPAN_NOT_CONTAINED: USetSpanCondition = 0i32;
pub const USET_SPAN_SIMPLE: USetSpanCondition = 2i32;
pub const USPOOF_ALL_CHECKS: USpoofChecks = 65535i32;
pub const USPOOF_ASCII: URestrictionLevel = 268435456i32;
pub const USPOOF_AUX_INFO: USpoofChecks = 1073741824i32;
pub const USPOOF_CHAR_LIMIT: USpoofChecks = 64i32;
pub const USPOOF_CONFUSABLE: USpoofChecks = 7i32;
pub const USPOOF_HIDDEN_OVERLAY: USpoofChecks = 256i32;
pub const USPOOF_HIGHLY_RESTRICTIVE: URestrictionLevel = 805306368i32;
pub const USPOOF_INVISIBLE: USpoofChecks = 32i32;
pub const USPOOF_MINIMALLY_RESTRICTIVE: URestrictionLevel = 1342177280i32;
pub const USPOOF_MIXED_NUMBERS: USpoofChecks = 128i32;
pub const USPOOF_MIXED_SCRIPT_CONFUSABLE: USpoofChecks = 2i32;
pub const USPOOF_MODERATELY_RESTRICTIVE: URestrictionLevel = 1073741824i32;
pub const USPOOF_RESTRICTION_LEVEL: USpoofChecks = 16i32;
pub const USPOOF_RESTRICTION_LEVEL_MASK: URestrictionLevel = 2130706432i32;
pub const USPOOF_SINGLE_SCRIPT_CONFUSABLE: USpoofChecks = 1i32;
pub const USPOOF_SINGLE_SCRIPT_RESTRICTIVE: URestrictionLevel = 536870912i32;
pub const USPOOF_UNRESTRICTIVE: URestrictionLevel = 1610612736i32;
pub const USPOOF_WHOLE_SCRIPT_CONFUSABLE: USpoofChecks = 4i32;
pub const USPREP_ALLOW_UNASSIGNED: u32 = 1u32;
pub const USPREP_DEFAULT: u32 = 0u32;
pub const USPREP_RFC3491_NAMEPREP: UStringPrepProfileType = 0i32;
pub const USPREP_RFC3530_NFS4_CIS_PREP: UStringPrepProfileType = 3i32;
pub const USPREP_RFC3530_NFS4_CS_PREP: UStringPrepProfileType = 1i32;
pub const USPREP_RFC3530_NFS4_CS_PREP_CI: UStringPrepProfileType = 2i32;
pub const USPREP_RFC3530_NFS4_MIXED_PREP_PREFIX: UStringPrepProfileType = 4i32;
pub const USPREP_RFC3530_NFS4_MIXED_PREP_SUFFIX: UStringPrepProfileType = 5i32;
pub const USPREP_RFC3722_ISCSI: UStringPrepProfileType = 6i32;
pub const USPREP_RFC3920_NODEPREP: UStringPrepProfileType = 7i32;
pub const USPREP_RFC3920_RESOURCEPREP: UStringPrepProfileType = 8i32;
pub const USPREP_RFC4011_MIB: UStringPrepProfileType = 9i32;
pub const USPREP_RFC4013_SASLPREP: UStringPrepProfileType = 10i32;
pub const USPREP_RFC4505_TRACE: UStringPrepProfileType = 11i32;
pub const USPREP_RFC4518_LDAP: UStringPrepProfileType = 12i32;
pub const USPREP_RFC4518_LDAP_CI: UStringPrepProfileType = 13i32;
pub const USP_E_SCRIPT_NOT_IN_FONT: ::windows_sys::core::HRESULT = -2147220992i32;
pub const USTRINGTRIE_BUILD_FAST: UStringTrieBuildOption = 0i32;
pub const USTRINGTRIE_BUILD_SMALL: UStringTrieBuildOption = 1i32;
pub const USTRINGTRIE_FINAL_VALUE: UStringTrieResult = 2i32;
pub const USTRINGTRIE_INTERMEDIATE_VALUE: UStringTrieResult = 3i32;
pub const USTRINGTRIE_NO_MATCH: UStringTrieResult = 0i32;
pub const USTRINGTRIE_NO_VALUE: UStringTrieResult = 1i32;
pub const UTEXT_MAGIC: i32 = 878368812i32;
pub const UTEXT_PROVIDER_HAS_META_DATA: i32 = 4i32;
pub const UTEXT_PROVIDER_LENGTH_IS_EXPENSIVE: i32 = 1i32;
pub const UTEXT_PROVIDER_OWNS_TEXT: i32 = 5i32;
pub const UTEXT_PROVIDER_STABLE_CHUNKS: i32 = 2i32;
pub const UTEXT_PROVIDER_WRITABLE: i32 = 3i32;
pub const UTF16_MAX_CHAR_LENGTH: u32 = 2u32;
pub const UTF32_MAX_CHAR_LENGTH: u32 = 1u32;
pub const UTF8_ERROR_VALUE_1: u32 = 21u32;
pub const UTF8_ERROR_VALUE_2: u32 = 159u32;
pub const UTF8_MAX_CHAR_LENGTH: u32 = 4u32;
pub const UTF_ERROR_VALUE: u32 = 65535u32;
pub const UTF_MAX_CHAR_LENGTH: u32 = 2u32;
pub const UTF_SIZE: u32 = 16u32;
pub const UTRACE_COLLATION_START: UTraceFunctionNumber = 8192i32;
pub const UTRACE_CONVERSION_START: UTraceFunctionNumber = 4096i32;
pub const UTRACE_ERROR: UTraceLevel = 0i32;
pub const UTRACE_FUNCTION_START: UTraceFunctionNumber = 0i32;
pub const UTRACE_INFO: UTraceLevel = 7i32;
pub const UTRACE_OFF: UTraceLevel = -1i32;
pub const UTRACE_OPEN_CLOSE: UTraceLevel = 5i32;
pub const UTRACE_UCNV_CLONE: UTraceFunctionNumber = 4099i32;
pub const UTRACE_UCNV_CLOSE: UTraceFunctionNumber = 4100i32;
pub const UTRACE_UCNV_FLUSH_CACHE: UTraceFunctionNumber = 4101i32;
pub const UTRACE_UCNV_LOAD: UTraceFunctionNumber = 4102i32;
pub const UTRACE_UCNV_OPEN: UTraceFunctionNumber = 4096i32;
pub const UTRACE_UCNV_OPEN_ALGORITHMIC: UTraceFunctionNumber = 4098i32;
pub const UTRACE_UCNV_OPEN_PACKAGE: UTraceFunctionNumber = 4097i32;
pub const UTRACE_UCNV_UNLOAD: UTraceFunctionNumber = 4103i32;
pub const UTRACE_UCOL_CLOSE: UTraceFunctionNumber = 8193i32;
pub const UTRACE_UCOL_GETLOCALE: UTraceFunctionNumber = 8196i32;
pub const UTRACE_UCOL_GET_SORTKEY: UTraceFunctionNumber = 8195i32;
pub const UTRACE_UCOL_NEXTSORTKEYPART: UTraceFunctionNumber = 8197i32;
pub const UTRACE_UCOL_OPEN: UTraceFunctionNumber = 8192i32;
pub const UTRACE_UCOL_OPEN_FROM_SHORT_STRING: UTraceFunctionNumber = 8199i32;
pub const UTRACE_UCOL_STRCOLL: UTraceFunctionNumber = 8194i32;
pub const UTRACE_UCOL_STRCOLLITER: UTraceFunctionNumber = 8198i32;
pub const UTRACE_UCOL_STRCOLLUTF8: UTraceFunctionNumber = 8200i32;
pub const UTRACE_UDATA_BUNDLE: UTraceFunctionNumber = 12289i32;
pub const UTRACE_UDATA_DATA_FILE: UTraceFunctionNumber = 12290i32;
pub const UTRACE_UDATA_RESOURCE: UTraceFunctionNumber = 12288i32;
pub const UTRACE_UDATA_RES_FILE: UTraceFunctionNumber = 12291i32;
pub const UTRACE_UDATA_START: UTraceFunctionNumber = 12288i32;
pub const UTRACE_U_CLEANUP: UTraceFunctionNumber = 1i32;
pub const UTRACE_U_INIT: UTraceFunctionNumber = 0i32;
pub const UTRACE_VERBOSE: UTraceLevel = 9i32;
pub const UTRACE_WARNING: UTraceLevel = 3i32;
pub const UTRANS_FORWARD: UTransDirection = 0i32;
pub const UTRANS_REVERSE: UTransDirection = 1i32;
pub const UTSV_EPOCH_OFFSET_VALUE: UTimeScaleValue = 1i32;
pub const UTSV_FROM_MAX_VALUE: UTimeScaleValue = 3i32;
pub const UTSV_FROM_MIN_VALUE: UTimeScaleValue = 2i32;
pub const UTSV_TO_MAX_VALUE: UTimeScaleValue = 5i32;
pub const UTSV_TO_MIN_VALUE: UTimeScaleValue = 4i32;
pub const UTSV_UNITS_VALUE: UTimeScaleValue = 0i32;
pub const UTZFMT_PARSE_OPTION_ALL_STYLES: UTimeZoneFormatParseOption = 1i32;
pub const UTZFMT_PARSE_OPTION_NONE: UTimeZoneFormatParseOption = 0i32;
pub const UTZFMT_PARSE_OPTION_TZ_DATABASE_ABBREVIATIONS: UTimeZoneFormatParseOption = 2i32;
pub const UTZFMT_PAT_COUNT: UTimeZoneFormatGMTOffsetPatternType = 6i32;
pub const UTZFMT_PAT_NEGATIVE_H: UTimeZoneFormatGMTOffsetPatternType = 5i32;
pub const UTZFMT_PAT_NEGATIVE_HM: UTimeZoneFormatGMTOffsetPatternType = 2i32;
pub const UTZFMT_PAT_NEGATIVE_HMS: UTimeZoneFormatGMTOffsetPatternType = 3i32;
pub const UTZFMT_PAT_POSITIVE_H: UTimeZoneFormatGMTOffsetPatternType = 4i32;
pub const UTZFMT_PAT_POSITIVE_HM: UTimeZoneFormatGMTOffsetPatternType = 0i32;
pub const UTZFMT_PAT_POSITIVE_HMS: UTimeZoneFormatGMTOffsetPatternType = 1i32;
pub const UTZFMT_STYLE_EXEMPLAR_LOCATION: UTimeZoneFormatStyle = 19i32;
pub const UTZFMT_STYLE_GENERIC_LOCATION: UTimeZoneFormatStyle = 0i32;
pub const UTZFMT_STYLE_GENERIC_LONG: UTimeZoneFormatStyle = 1i32;
pub const UTZFMT_STYLE_GENERIC_SHORT: UTimeZoneFormatStyle = 2i32;
pub const UTZFMT_STYLE_ISO_BASIC_FIXED: UTimeZoneFormatStyle = 9i32;
pub const UTZFMT_STYLE_ISO_BASIC_FULL: UTimeZoneFormatStyle = 11i32;
pub const UTZFMT_STYLE_ISO_BASIC_LOCAL_FIXED: UTimeZoneFormatStyle = 10i32;
pub const UTZFMT_STYLE_ISO_BASIC_LOCAL_FULL: UTimeZoneFormatStyle = 12i32;
pub const UTZFMT_STYLE_ISO_BASIC_LOCAL_SHORT: UTimeZoneFormatStyle = 8i32;
pub const UTZFMT_STYLE_ISO_BASIC_SHORT: UTimeZoneFormatStyle = 7i32;
pub const UTZFMT_STYLE_ISO_EXTENDED_FIXED: UTimeZoneFormatStyle = 13i32;
pub const UTZFMT_STYLE_ISO_EXTENDED_FULL: UTimeZoneFormatStyle = 15i32;
pub const UTZFMT_STYLE_ISO_EXTENDED_LOCAL_FIXED: UTimeZoneFormatStyle = 14i32;
pub const UTZFMT_STYLE_ISO_EXTENDED_LOCAL_FULL: UTimeZoneFormatStyle = 16i32;
pub const UTZFMT_STYLE_LOCALIZED_GMT: UTimeZoneFormatStyle = 5i32;
pub const UTZFMT_STYLE_LOCALIZED_GMT_SHORT: UTimeZoneFormatStyle = 6i32;
pub const UTZFMT_STYLE_SPECIFIC_LONG: UTimeZoneFormatStyle = 3i32;
pub const UTZFMT_STYLE_SPECIFIC_SHORT: UTimeZoneFormatStyle = 4i32;
pub const UTZFMT_STYLE_ZONE_ID: UTimeZoneFormatStyle = 17i32;
pub const UTZFMT_STYLE_ZONE_ID_SHORT: UTimeZoneFormatStyle = 18i32;
pub const UTZFMT_TIME_TYPE_DAYLIGHT: UTimeZoneFormatTimeType = 2i32;
pub const UTZFMT_TIME_TYPE_STANDARD: UTimeZoneFormatTimeType = 1i32;
pub const UTZFMT_TIME_TYPE_UNKNOWN: UTimeZoneFormatTimeType = 0i32;
pub const UTZNM_EXEMPLAR_LOCATION: UTimeZoneNameType = 64i32;
pub const UTZNM_LONG_DAYLIGHT: UTimeZoneNameType = 4i32;
pub const UTZNM_LONG_GENERIC: UTimeZoneNameType = 1i32;
pub const UTZNM_LONG_STANDARD: UTimeZoneNameType = 2i32;
pub const UTZNM_SHORT_DAYLIGHT: UTimeZoneNameType = 32i32;
pub const UTZNM_SHORT_GENERIC: UTimeZoneNameType = 8i32;
pub const UTZNM_SHORT_STANDARD: UTimeZoneNameType = 16i32;
pub const UTZNM_UNKNOWN: UTimeZoneNameType = 0i32;
pub const U_ALPHAINDEX_INFLOW: UAlphabeticIndexLabelType = 2i32;
pub const U_ALPHAINDEX_NORMAL: UAlphabeticIndexLabelType = 0i32;
pub const U_ALPHAINDEX_OVERFLOW: UAlphabeticIndexLabelType = 3i32;
pub const U_ALPHAINDEX_UNDERFLOW: UAlphabeticIndexLabelType = 1i32;
pub const U_AMBIGUOUS_ALIAS_WARNING: UErrorCode = -122i32;
pub const U_ARABIC_NUMBER: UCharDirection = 5i32;
pub const U_ARGUMENT_TYPE_MISMATCH: UErrorCode = 65804i32;
pub const U_ASCII_FAMILY: u32 = 0u32;
pub const U_BAD_VARIABLE_DEFINITION: UErrorCode = 65536i32;
pub const U_BLOCK_SEPARATOR: UCharDirection = 7i32;
pub const U_BOUNDARY_NEUTRAL: UCharDirection = 18i32;
pub const U_BPT_CLOSE: UBidiPairedBracketType = 2i32;
pub const U_BPT_NONE: UBidiPairedBracketType = 0i32;
pub const U_BPT_OPEN: UBidiPairedBracketType = 1i32;
pub const U_BRK_ASSIGN_ERROR: UErrorCode = 66053i32;
pub const U_BRK_ERROR_START: UErrorCode = 66048i32;
pub const U_BRK_HEX_DIGITS_EXPECTED: UErrorCode = 66049i32;
pub const U_BRK_INIT_ERROR: UErrorCode = 66058i32;
pub const U_BRK_INTERNAL_ERROR: UErrorCode = 66048i32;
pub const U_BRK_MALFORMED_RULE_TAG: UErrorCode = 66061i32;
pub const U_BRK_MISMATCHED_PAREN: UErrorCode = 66055i32;
pub const U_BRK_NEW_LINE_IN_QUOTED_STRING: UErrorCode = 66056i32;
pub const U_BRK_RULE_EMPTY_SET: UErrorCode = 66059i32;
pub const U_BRK_RULE_SYNTAX: UErrorCode = 66051i32;
pub const U_BRK_SEMICOLON_EXPECTED: UErrorCode = 66050i32;
pub const U_BRK_UNCLOSED_SET: UErrorCode = 66052i32;
pub const U_BRK_UNDEFINED_VARIABLE: UErrorCode = 66057i32;
pub const U_BRK_UNRECOGNIZED_OPTION: UErrorCode = 66060i32;
pub const U_BRK_VARIABLE_REDFINITION: UErrorCode = 66054i32;
pub const U_BUFFER_OVERFLOW_ERROR: UErrorCode = 15i32;
pub const U_CE_NOT_FOUND_ERROR: UErrorCode = 21i32;
pub const U_CHAR16_IS_TYPEDEF: u32 = 1u32;
pub const U_CHARSET_FAMILY: u32 = 1u32;
pub const U_CHARSET_IS_UTF8: u32 = 1u32;
pub const U_CHAR_CATEGORY_COUNT: UCharCategory = 30i32;
pub const U_CHAR_NAME_ALIAS: UCharNameChoice = 3i32;
pub const U_CHECK_DYLOAD: u32 = 1u32;
pub const U_COLLATOR_VERSION_MISMATCH: UErrorCode = 28i32;
pub const U_COMBINED_IMPLEMENTATION: u32 = 1u32;
pub const U_COMBINING_SPACING_MARK: UCharCategory = 8i32;
pub const U_COMMON_NUMBER_SEPARATOR: UCharDirection = 6i32;
pub const U_COMPARE_CODE_POINT_ORDER: u32 = 32768u32;
pub const U_COMPARE_IGNORE_CASE: u32 = 65536u32;
pub const U_CONNECTOR_PUNCTUATION: UCharCategory = 22i32;
pub const U_CONTROL_CHAR: UCharCategory = 15i32;
pub const U_COPYRIGHT_STRING_LENGTH: u32 = 128u32;
pub const U_CPLUSPLUS_VERSION: u32 = 0u32;
pub const U_CURRENCY_SYMBOL: UCharCategory = 25i32;
pub const U_DASH_PUNCTUATION: UCharCategory = 19i32;
pub const U_DEBUG: u32 = 1u32;
pub const U_DECIMAL_DIGIT_NUMBER: UCharCategory = 9i32;
pub const U_DECIMAL_NUMBER_SYNTAX_ERROR: UErrorCode = 65808i32;
pub const U_DEFAULT_KEYWORD_MISSING: UErrorCode = 65807i32;
pub const U_DEFAULT_SHOW_DRAFT: u32 = 0u32;
pub const U_DEFINE_FALSE_AND_TRUE: u32 = 1u32;
pub const U_DIFFERENT_UCA_VERSION: UErrorCode = -121i32;
pub const U_DIR_NON_SPACING_MARK: UCharDirection = 17i32;
pub const U_DISABLE_RENAMING: u32 = 1u32;
pub const U_DT_CANONICAL: UDecompositionType = 1i32;
pub const U_DT_CIRCLE: UDecompositionType = 3i32;
pub const U_DT_COMPAT: UDecompositionType = 2i32;
pub const U_DT_FINAL: UDecompositionType = 4i32;
pub const U_DT_FONT: UDecompositionType = 5i32;
pub const U_DT_FRACTION: UDecompositionType = 6i32;
pub const U_DT_INITIAL: UDecompositionType = 7i32;
pub const U_DT_ISOLATED: UDecompositionType = 8i32;
pub const U_DT_MEDIAL: UDecompositionType = 9i32;
pub const U_DT_NARROW: UDecompositionType = 10i32;
pub const U_DT_NOBREAK: UDecompositionType = 11i32;
pub const U_DT_NONE: UDecompositionType = 0i32;
pub const U_DT_SMALL: UDecompositionType = 12i32;
pub const U_DT_SQUARE: UDecompositionType = 13i32;
pub const U_DT_SUB: UDecompositionType = 14i32;
pub const U_DT_SUPER: UDecompositionType = 15i32;
pub const U_DT_VERTICAL: UDecompositionType = 16i32;
pub const U_DT_WIDE: UDecompositionType = 17i32;
pub const U_DUPLICATE_KEYWORD: UErrorCode = 65805i32;
pub const U_EA_AMBIGUOUS: UEastAsianWidth = 1i32;
pub const U_EA_FULLWIDTH: UEastAsianWidth = 3i32;
pub const U_EA_HALFWIDTH: UEastAsianWidth = 2i32;
pub const U_EA_NARROW: UEastAsianWidth = 4i32;
pub const U_EA_NEUTRAL: UEastAsianWidth = 0i32;
pub const U_EA_WIDE: UEastAsianWidth = 5i32;
pub const U_EBCDIC_FAMILY: u32 = 1u32;
pub const U_EDITS_NO_RESET: u32 = 8192u32;
pub const U_ENABLE_DYLOAD: u32 = 1u32;
pub const U_ENABLE_TRACING: u32 = 0u32;
pub const U_ENCLOSING_MARK: UCharCategory = 7i32;
pub const U_END_PUNCTUATION: UCharCategory = 21i32;
pub const U_ENUM_OUT_OF_SYNC_ERROR: UErrorCode = 25i32;
pub const U_ERROR_WARNING_START: UErrorCode = -128i32;
pub const U_EUROPEAN_NUMBER: UCharDirection = 2i32;
pub const U_EUROPEAN_NUMBER_SEPARATOR: UCharDirection = 3i32;
pub const U_EUROPEAN_NUMBER_TERMINATOR: UCharDirection = 4i32;
pub const U_EXTENDED_CHAR_NAME: UCharNameChoice = 2i32;
pub const U_FILE_ACCESS_ERROR: UErrorCode = 4i32;
pub const U_FINAL_PUNCTUATION: UCharCategory = 29i32;
pub const U_FIRST_STRONG_ISOLATE: UCharDirection = 19i32;
pub const U_FMT_PARSE_ERROR_START: UErrorCode = 65792i32;
pub const U_FOLD_CASE_DEFAULT: u32 = 0u32;
pub const U_FOLD_CASE_EXCLUDE_SPECIAL_I: u32 = 1u32;
pub const U_FORMAT_CHAR: UCharCategory = 16i32;
pub const U_FORMAT_INEXACT_ERROR: UErrorCode = 65809i32;
pub const U_GCB_CONTROL: UGraphemeClusterBreak = 1i32;
pub const U_GCB_CR: UGraphemeClusterBreak = 2i32;
pub const U_GCB_EXTEND: UGraphemeClusterBreak = 3i32;
pub const U_GCB_E_BASE: UGraphemeClusterBreak = 13i32;
pub const U_GCB_E_BASE_GAZ: UGraphemeClusterBreak = 14i32;
pub const U_GCB_E_MODIFIER: UGraphemeClusterBreak = 15i32;
pub const U_GCB_GLUE_AFTER_ZWJ: UGraphemeClusterBreak = 16i32;
pub const U_GCB_L: UGraphemeClusterBreak = 4i32;
pub const U_GCB_LF: UGraphemeClusterBreak = 5i32;
pub const U_GCB_LV: UGraphemeClusterBreak = 6i32;
pub const U_GCB_LVT: UGraphemeClusterBreak = 7i32;
pub const U_GCB_OTHER: UGraphemeClusterBreak = 0i32;
pub const U_GCB_PREPEND: UGraphemeClusterBreak = 11i32;
pub const U_GCB_REGIONAL_INDICATOR: UGraphemeClusterBreak = 12i32;
pub const U_GCB_SPACING_MARK: UGraphemeClusterBreak = 10i32;
pub const U_GCB_T: UGraphemeClusterBreak = 8i32;
pub const U_GCB_V: UGraphemeClusterBreak = 9i32;
pub const U_GCB_ZWJ: UGraphemeClusterBreak = 17i32;
pub const U_GCC_MAJOR_MINOR: u32 = 0u32;
pub const U_GENERAL_OTHER_TYPES: UCharCategory = 0i32;
pub const U_HAVE_CHAR16_T: u32 = 1u32;
pub const U_HAVE_DEBUG_LOCATION_NEW: u32 = 1u32;
pub const U_HAVE_INTTYPES_H: u32 = 1u32;
pub const U_HAVE_LIB_SUFFIX: u32 = 1u32;
pub const U_HAVE_PLACEMENT_NEW: u32 = 0u32;
pub const U_HAVE_RBNF: u32 = 0u32;
pub const U_HAVE_RVALUE_REFERENCES: u32 = 1u32;
pub const U_HAVE_STDINT_H: u32 = 1u32;
pub const U_HAVE_STD_STRING: u32 = 0u32;
pub const U_HAVE_WCHAR_H: u32 = 0u32;
pub const U_HAVE_WCSCPY: u32 = 0u32;
pub const U_HIDE_DEPRECATED_API: u32 = 1u32;
pub const U_HIDE_DRAFT_API: u32 = 1u32;
pub const U_HIDE_INTERNAL_API: u32 = 1u32;
pub const U_HIDE_OBSOLETE_API: u32 = 1u32;
pub const U_HIDE_OBSOLETE_UTF_OLD_H: u32 = 0u32;
pub const U_HST_LEADING_JAMO: UHangulSyllableType = 1i32;
pub const U_HST_LVT_SYLLABLE: UHangulSyllableType = 5i32;
pub const U_HST_LV_SYLLABLE: UHangulSyllableType = 4i32;
pub const U_HST_NOT_APPLICABLE: UHangulSyllableType = 0i32;
pub const U_HST_TRAILING_JAMO: UHangulSyllableType = 3i32;
pub const U_HST_VOWEL_JAMO: UHangulSyllableType = 2i32;
pub const U_ICUDATA_TYPE_LETTER: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("e");
pub const U_ICU_DATA_KEY: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("DataVersion");
pub const U_ICU_VERSION_BUNDLE: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("icuver");
pub const U_IDNA_ACE_PREFIX_ERROR: UErrorCode = 66564i32;
pub const U_IDNA_CHECK_BIDI_ERROR: UErrorCode = 66562i32;
pub const U_IDNA_DOMAIN_NAME_TOO_LONG_ERROR: UErrorCode = 66568i32;
pub const U_IDNA_ERROR_START: UErrorCode = 66560i32;
pub const U_IDNA_LABEL_TOO_LONG_ERROR: UErrorCode = 66566i32;
pub const U_IDNA_PROHIBITED_ERROR: UErrorCode = 66560i32;
pub const U_IDNA_STD3_ASCII_RULES_ERROR: UErrorCode = 66563i32;
pub const U_IDNA_UNASSIGNED_ERROR: UErrorCode = 66561i32;
pub const U_IDNA_VERIFICATION_ERROR: UErrorCode = 66565i32;
pub const U_IDNA_ZERO_LENGTH_LABEL_ERROR: UErrorCode = 66567i32;
pub const U_ILLEGAL_ARGUMENT_ERROR: UErrorCode = 1i32;
pub const U_ILLEGAL_CHARACTER: UErrorCode = 65567i32;
pub const U_ILLEGAL_CHAR_FOUND: UErrorCode = 12i32;
pub const U_ILLEGAL_CHAR_IN_SEGMENT: UErrorCode = 65564i32;
pub const U_ILLEGAL_ESCAPE_SEQUENCE: UErrorCode = 18i32;
pub const U_ILLEGAL_PAD_POSITION: UErrorCode = 65800i32;
pub const U_INDEX_OUTOFBOUNDS_ERROR: UErrorCode = 8i32;
pub const U_INITIAL_PUNCTUATION: UCharCategory = 28i32;
pub const U_INPC_BOTTOM: UIndicPositionalCategory = 1i32;
pub const U_INPC_BOTTOM_AND_LEFT: UIndicPositionalCategory = 2i32;
pub const U_INPC_BOTTOM_AND_RIGHT: UIndicPositionalCategory = 3i32;
pub const U_INPC_LEFT: UIndicPositionalCategory = 4i32;
pub const U_INPC_LEFT_AND_RIGHT: UIndicPositionalCategory = 5i32;
pub const U_INPC_NA: UIndicPositionalCategory = 0i32;
pub const U_INPC_OVERSTRUCK: UIndicPositionalCategory = 6i32;
pub const U_INPC_RIGHT: UIndicPositionalCategory = 7i32;
pub const U_INPC_TOP: UIndicPositionalCategory = 8i32;
pub const U_INPC_TOP_AND_BOTTOM: UIndicPositionalCategory = 9i32;
pub const U_INPC_TOP_AND_BOTTOM_AND_LEFT: UIndicPositionalCategory = 15i32;
pub const U_INPC_TOP_AND_BOTTOM_AND_RIGHT: UIndicPositionalCategory = 10i32;
pub const U_INPC_TOP_AND_LEFT: UIndicPositionalCategory = 11i32;
pub const U_INPC_TOP_AND_LEFT_AND_RIGHT: UIndicPositionalCategory = 12i32;
pub const U_INPC_TOP_AND_RIGHT: UIndicPositionalCategory = 13i32;
pub const U_INPC_VISUAL_ORDER_LEFT: UIndicPositionalCategory = 14i32;
pub const U_INSC_AVAGRAHA: UIndicSyllabicCategory = 1i32;
pub const U_INSC_BINDU: UIndicSyllabicCategory = 2i32;
pub const U_INSC_BRAHMI_JOINING_NUMBER: UIndicSyllabicCategory = 3i32;
pub const U_INSC_CANTILLATION_MARK: UIndicSyllabicCategory = 4i32;
pub const U_INSC_CONSONANT: UIndicSyllabicCategory = 5i32;
pub const U_INSC_CONSONANT_DEAD: UIndicSyllabicCategory = 6i32;
pub const U_INSC_CONSONANT_FINAL: UIndicSyllabicCategory = 7i32;
pub const U_INSC_CONSONANT_HEAD_LETTER: UIndicSyllabicCategory = 8i32;
pub const U_INSC_CONSONANT_INITIAL_POSTFIXED: UIndicSyllabicCategory = 9i32;
pub const U_INSC_CONSONANT_KILLER: UIndicSyllabicCategory = 10i32;
pub const U_INSC_CONSONANT_MEDIAL: UIndicSyllabicCategory = 11i32;
pub const U_INSC_CONSONANT_PLACEHOLDER: UIndicSyllabicCategory = 12i32;
pub const U_INSC_CONSONANT_PRECEDING_REPHA: UIndicSyllabicCategory = 13i32;
pub const U_INSC_CONSONANT_PREFIXED: UIndicSyllabicCategory = 14i32;
pub const U_INSC_CONSONANT_SUBJOINED: UIndicSyllabicCategory = 15i32;
pub const U_INSC_CONSONANT_SUCCEEDING_REPHA: UIndicSyllabicCategory = 16i32;
pub const U_INSC_CONSONANT_WITH_STACKER: UIndicSyllabicCategory = 17i32;
pub const U_INSC_GEMINATION_MARK: UIndicSyllabicCategory = 18i32;
pub const U_INSC_INVISIBLE_STACKER: UIndicSyllabicCategory = 19i32;
pub const U_INSC_JOINER: UIndicSyllabicCategory = 20i32;
pub const U_INSC_MODIFYING_LETTER: UIndicSyllabicCategory = 21i32;
pub const U_INSC_NON_JOINER: UIndicSyllabicCategory = 22i32;
pub const U_INSC_NUKTA: UIndicSyllabicCategory = 23i32;
pub const U_INSC_NUMBER: UIndicSyllabicCategory = 24i32;
pub const U_INSC_NUMBER_JOINER: UIndicSyllabicCategory = 25i32;
pub const U_INSC_OTHER: UIndicSyllabicCategory = 0i32;
pub const U_INSC_PURE_KILLER: UIndicSyllabicCategory = 26i32;
pub const U_INSC_REGISTER_SHIFTER: UIndicSyllabicCategory = 27i32;
pub const U_INSC_SYLLABLE_MODIFIER: UIndicSyllabicCategory = 28i32;
pub const U_INSC_TONE_LETTER: UIndicSyllabicCategory = 29i32;
pub const U_INSC_TONE_MARK: UIndicSyllabicCategory = 30i32;
pub const U_INSC_VIRAMA: UIndicSyllabicCategory = 31i32;
pub const U_INSC_VISARGA: UIndicSyllabicCategory = 32i32;
pub const U_INSC_VOWEL: UIndicSyllabicCategory = 33i32;
pub const U_INSC_VOWEL_DEPENDENT: UIndicSyllabicCategory = 34i32;
pub const U_INSC_VOWEL_INDEPENDENT: UIndicSyllabicCategory = 35i32;
pub const U_INTERNAL_PROGRAM_ERROR: UErrorCode = 5i32;
pub const U_INTERNAL_TRANSLITERATOR_ERROR: UErrorCode = 65568i32;
pub const U_INVALID_CHAR_FOUND: UErrorCode = 10i32;
pub const U_INVALID_FORMAT_ERROR: UErrorCode = 3i32;
pub const U_INVALID_FUNCTION: UErrorCode = 65570i32;
pub const U_INVALID_ID: UErrorCode = 65569i32;
pub const U_INVALID_PROPERTY_PATTERN: UErrorCode = 65561i32;
pub const U_INVALID_RBT_SYNTAX: UErrorCode = 65560i32;
pub const U_INVALID_STATE_ERROR: UErrorCode = 27i32;
pub const U_INVALID_TABLE_FILE: UErrorCode = 14i32;
pub const U_INVALID_TABLE_FORMAT: UErrorCode = 13i32;
pub const U_INVARIANT_CONVERSION_ERROR: UErrorCode = 26i32;
pub const U_IOSTREAM_SOURCE: u32 = 199711u32;
pub const U_IS_BIG_ENDIAN: u32 = 0u32;
pub const U_JG_AFRICAN_FEH: UJoiningGroup = 86i32;
pub const U_JG_AFRICAN_NOON: UJoiningGroup = 87i32;
pub const U_JG_AFRICAN_QAF: UJoiningGroup = 88i32;
pub const U_JG_AIN: UJoiningGroup = 1i32;
pub const U_JG_ALAPH: UJoiningGroup = 2i32;
pub const U_JG_ALEF: UJoiningGroup = 3i32;
pub const U_JG_BEH: UJoiningGroup = 4i32;
pub const U_JG_BETH: UJoiningGroup = 5i32;
pub const U_JG_BURUSHASKI_YEH_BARREE: UJoiningGroup = 54i32;
pub const U_JG_DAL: UJoiningGroup = 6i32;
pub const U_JG_DALATH_RISH: UJoiningGroup = 7i32;
pub const U_JG_E: UJoiningGroup = 8i32;
pub const U_JG_FARSI_YEH: UJoiningGroup = 55i32;
pub const U_JG_FE: UJoiningGroup = 51i32;
pub const U_JG_FEH: UJoiningGroup = 9i32;
pub const U_JG_FINAL_SEMKATH: UJoiningGroup = 10i32;
pub const U_JG_GAF: UJoiningGroup = 11i32;
pub const U_JG_GAMAL: UJoiningGroup = 12i32;
pub const U_JG_HAH: UJoiningGroup = 13i32;
pub const U_JG_HAMZA_ON_HEH_GOAL: UJoiningGroup = 14i32;
pub const U_JG_HANIFI_ROHINGYA_KINNA_YA: UJoiningGroup = 100i32;
pub const U_JG_HANIFI_ROHINGYA_PA: UJoiningGroup = 101i32;
pub const U_JG_HE: UJoiningGroup = 15i32;
pub const U_JG_HEH: UJoiningGroup = 16i32;
pub const U_JG_HEH_GOAL: UJoiningGroup = 17i32;
pub const U_JG_HETH: UJoiningGroup = 18i32;
pub const U_JG_KAF: UJoiningGroup = 19i32;
pub const U_JG_KAPH: UJoiningGroup = 20i32;
pub const U_JG_KHAPH: UJoiningGroup = 52i32;
pub const U_JG_KNOTTED_HEH: UJoiningGroup = 21i32;
pub const U_JG_LAM: UJoiningGroup = 22i32;
pub const U_JG_LAMADH: UJoiningGroup = 23i32;
pub const U_JG_MALAYALAM_BHA: UJoiningGroup = 89i32;
pub const U_JG_MALAYALAM_JA: UJoiningGroup = 90i32;
pub const U_JG_MALAYALAM_LLA: UJoiningGroup = 91i32;
pub const U_JG_MALAYALAM_LLLA: UJoiningGroup = 92i32;
pub const U_JG_MALAYALAM_NGA: UJoiningGroup = 93i32;
pub const U_JG_MALAYALAM_NNA: UJoiningGroup = 94i32;
pub const U_JG_MALAYALAM_NNNA: UJoiningGroup = 95i32;
pub const U_JG_MALAYALAM_NYA: UJoiningGroup = 96i32;
pub const U_JG_MALAYALAM_RA: UJoiningGroup = 97i32;
pub const U_JG_MALAYALAM_SSA: UJoiningGroup = 98i32;
pub const U_JG_MALAYALAM_TTA: UJoiningGroup = 99i32;
pub const U_JG_MANICHAEAN_ALEPH: UJoiningGroup = 58i32;
pub const U_JG_MANICHAEAN_AYIN: UJoiningGroup = 59i32;
pub const U_JG_MANICHAEAN_BETH: UJoiningGroup = 60i32;
pub const U_JG_MANICHAEAN_DALETH: UJoiningGroup = 61i32;
pub const U_JG_MANICHAEAN_DHAMEDH: UJoiningGroup = 62i32;
pub const U_JG_MANICHAEAN_FIVE: UJoiningGroup = 63i32;
pub const U_JG_MANICHAEAN_GIMEL: UJoiningGroup = 64i32;
pub const U_JG_MANICHAEAN_HETH: UJoiningGroup = 65i32;
pub const U_JG_MANICHAEAN_HUNDRED: UJoiningGroup = 66i32;
pub const U_JG_MANICHAEAN_KAPH: UJoiningGroup = 67i32;
pub const U_JG_MANICHAEAN_LAMEDH: UJoiningGroup = 68i32;
pub const U_JG_MANICHAEAN_MEM: UJoiningGroup = 69i32;
pub const U_JG_MANICHAEAN_NUN: UJoiningGroup = 70i32;
pub const U_JG_MANICHAEAN_ONE: UJoiningGroup = 71i32;
pub const U_JG_MANICHAEAN_PE: UJoiningGroup = 72i32;
pub const U_JG_MANICHAEAN_QOPH: UJoiningGroup = 73i32;
pub const U_JG_MANICHAEAN_RESH: UJoiningGroup = 74i32;
pub const U_JG_MANICHAEAN_SADHE: UJoiningGroup = 75i32;
pub const U_JG_MANICHAEAN_SAMEKH: UJoiningGroup = 76i32;
pub const U_JG_MANICHAEAN_TAW: UJoiningGroup = 77i32;
pub const U_JG_MANICHAEAN_TEN: UJoiningGroup = 78i32;
pub const U_JG_MANICHAEAN_TETH: UJoiningGroup = 79i32;
pub const U_JG_MANICHAEAN_THAMEDH: UJoiningGroup = 80i32;
pub const U_JG_MANICHAEAN_TWENTY: UJoiningGroup = 81i32;
pub const U_JG_MANICHAEAN_WAW: UJoiningGroup = 82i32;
pub const U_JG_MANICHAEAN_YODH: UJoiningGroup = 83i32;
pub const U_JG_MANICHAEAN_ZAYIN: UJoiningGroup = 84i32;
pub const U_JG_MEEM: UJoiningGroup = 24i32;
pub const U_JG_MIM: UJoiningGroup = 25i32;
pub const U_JG_NOON: UJoiningGroup = 26i32;
pub const U_JG_NO_JOINING_GROUP: UJoiningGroup = 0i32;
pub const U_JG_NUN: UJoiningGroup = 27i32;
pub const U_JG_NYA: UJoiningGroup = 56i32;
pub const U_JG_PE: UJoiningGroup = 28i32;
pub const U_JG_QAF: UJoiningGroup = 29i32;
pub const U_JG_QAPH: UJoiningGroup = 30i32;
pub const U_JG_REH: UJoiningGroup = 31i32;
pub const U_JG_REVERSED_PE: UJoiningGroup = 32i32;
pub const U_JG_ROHINGYA_YEH: UJoiningGroup = 57i32;
pub const U_JG_SAD: UJoiningGroup = 33i32;
pub const U_JG_SADHE: UJoiningGroup = 34i32;
pub const U_JG_SEEN: UJoiningGroup = 35i32;
pub const U_JG_SEMKATH: UJoiningGroup = 36i32;
pub const U_JG_SHIN: UJoiningGroup = 37i32;
pub const U_JG_STRAIGHT_WAW: UJoiningGroup = 85i32;
pub const U_JG_SWASH_KAF: UJoiningGroup = 38i32;
pub const U_JG_SYRIAC_WAW: UJoiningGroup = 39i32;
pub const U_JG_TAH: UJoiningGroup = 40i32;
pub const U_JG_TAW: UJoiningGroup = 41i32;
pub const U_JG_TEH_MARBUTA: UJoiningGroup = 42i32;
pub const U_JG_TEH_MARBUTA_GOAL: UJoiningGroup = 14i32;
pub const U_JG_TETH: UJoiningGroup = 43i32;
pub const U_JG_WAW: UJoiningGroup = 44i32;
pub const U_JG_YEH: UJoiningGroup = 45i32;
pub const U_JG_YEH_BARREE: UJoiningGroup = 46i32;
pub const U_JG_YEH_WITH_TAIL: UJoiningGroup = 47i32;
pub const U_JG_YUDH: UJoiningGroup = 48i32;
pub const U_JG_YUDH_HE: UJoiningGroup = 49i32;
pub const U_JG_ZAIN: UJoiningGroup = 50i32;
pub const U_JG_ZHAIN: UJoiningGroup = 53i32;
pub const U_JT_DUAL_JOINING: UJoiningType = 2i32;
pub const U_JT_JOIN_CAUSING: UJoiningType = 1i32;
pub const U_JT_LEFT_JOINING: UJoiningType = 3i32;
pub const U_JT_NON_JOINING: UJoiningType = 0i32;
pub const U_JT_RIGHT_JOINING: UJoiningType = 4i32;
pub const U_JT_TRANSPARENT: UJoiningType = 5i32;
pub const U_LB_ALPHABETIC: ULineBreak = 2i32;
pub const U_LB_AMBIGUOUS: ULineBreak = 1i32;
pub const U_LB_BREAK_AFTER: ULineBreak = 4i32;
pub const U_LB_BREAK_BEFORE: ULineBreak = 5i32;
pub const U_LB_BREAK_BOTH: ULineBreak = 3i32;
pub const U_LB_BREAK_SYMBOLS: ULineBreak = 27i32;
pub const U_LB_CARRIAGE_RETURN: ULineBreak = 10i32;
pub const U_LB_CLOSE_PARENTHESIS: ULineBreak = 36i32;
pub const U_LB_CLOSE_PUNCTUATION: ULineBreak = 8i32;
pub const U_LB_COMBINING_MARK: ULineBreak = 9i32;
pub const U_LB_COMPLEX_CONTEXT: ULineBreak = 24i32;
pub const U_LB_CONDITIONAL_JAPANESE_STARTER: ULineBreak = 37i32;
pub const U_LB_CONTINGENT_BREAK: ULineBreak = 7i32;
pub const U_LB_EXCLAMATION: ULineBreak = 11i32;
pub const U_LB_E_BASE: ULineBreak = 40i32;
pub const U_LB_E_MODIFIER: ULineBreak = 41i32;
pub const U_LB_GLUE: ULineBreak = 12i32;
pub const U_LB_H2: ULineBreak = 31i32;
pub const U_LB_H3: ULineBreak = 32i32;
pub const U_LB_HEBREW_LETTER: ULineBreak = 38i32;
pub const U_LB_HYPHEN: ULineBreak = 13i32;
pub const U_LB_IDEOGRAPHIC: ULineBreak = 14i32;
pub const U_LB_INFIX_NUMERIC: ULineBreak = 16i32;
pub const U_LB_INSEPARABLE: ULineBreak = 15i32;
pub const U_LB_INSEPERABLE: ULineBreak = 15i32;
pub const U_LB_JL: ULineBreak = 33i32;
pub const U_LB_JT: ULineBreak = 34i32;
pub const U_LB_JV: ULineBreak = 35i32;
pub const U_LB_LINE_FEED: ULineBreak = 17i32;
pub const U_LB_MANDATORY_BREAK: ULineBreak = 6i32;
pub const U_LB_NEXT_LINE: ULineBreak = 29i32;
pub const U_LB_NONSTARTER: ULineBreak = 18i32;
pub const U_LB_NUMERIC: ULineBreak = 19i32;
pub const U_LB_OPEN_PUNCTUATION: ULineBreak = 20i32;
pub const U_LB_POSTFIX_NUMERIC: ULineBreak = 21i32;
pub const U_LB_PREFIX_NUMERIC: ULineBreak = 22i32;
pub const U_LB_QUOTATION: ULineBreak = 23i32;
pub const U_LB_REGIONAL_INDICATOR: ULineBreak = 39i32;
pub const U_LB_SPACE: ULineBreak = 26i32;
pub const U_LB_SURROGATE: ULineBreak = 25i32;
pub const U_LB_UNKNOWN: ULineBreak = 0i32;
pub const U_LB_WORD_JOINER: ULineBreak = 30i32;
pub const U_LB_ZWJ: ULineBreak = 42i32;
pub const U_LB_ZWSPACE: ULineBreak = 28i32;
pub const U_LEFT_TO_RIGHT: UCharDirection = 0i32;
pub const U_LEFT_TO_RIGHT_EMBEDDING: UCharDirection = 11i32;
pub const U_LEFT_TO_RIGHT_ISOLATE: UCharDirection = 20i32;
pub const U_LEFT_TO_RIGHT_OVERRIDE: UCharDirection = 12i32;
pub const U_LETTER_NUMBER: UCharCategory = 10i32;
pub const U_LIB_SUFFIX_C_NAME_STRING: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("");
pub const U_LINE_SEPARATOR: UCharCategory = 13i32;
pub const U_LONG_PROPERTY_NAME: UPropertyNameChoice = 1i32;
pub const U_LOWERCASE_LETTER: UCharCategory = 2i32;
pub const U_MALFORMED_EXPONENTIAL_PATTERN: UErrorCode = 65795i32;
pub const U_MALFORMED_PRAGMA: UErrorCode = 65562i32;
pub const U_MALFORMED_RULE: UErrorCode = 65537i32;
pub const U_MALFORMED_SET: UErrorCode = 65538i32;
pub const U_MALFORMED_SYMBOL_REFERENCE: UErrorCode = 65539i32;
pub const U_MALFORMED_UNICODE_ESCAPE: UErrorCode = 65540i32;
pub const U_MALFORMED_VARIABLE_DEFINITION: UErrorCode = 65541i32;
pub const U_MALFORMED_VARIABLE_REFERENCE: UErrorCode = 65542i32;
pub const U_MATH_SYMBOL: UCharCategory = 24i32;
pub const U_MAX_VERSION_LENGTH: u32 = 4u32;
pub const U_MAX_VERSION_STRING_LENGTH: u32 = 20u32;
pub const U_MEMORY_ALLOCATION_ERROR: UErrorCode = 7i32;
pub const U_MESSAGE_PARSE_ERROR: UErrorCode = 6i32;
pub const U_MILLIS_PER_DAY: u32 = 86400000u32;
pub const U_MILLIS_PER_HOUR: u32 = 3600000u32;
pub const U_MILLIS_PER_MINUTE: u32 = 60000u32;
pub const U_MILLIS_PER_SECOND: u32 = 1000u32;
pub const U_MISMATCHED_SEGMENT_DELIMITERS: UErrorCode = 65543i32;
pub const U_MISPLACED_ANCHOR_START: UErrorCode = 65544i32;
pub const U_MISPLACED_COMPOUND_FILTER: UErrorCode = 65558i32;
pub const U_MISPLACED_CURSOR_OFFSET: UErrorCode = 65545i32;
pub const U_MISPLACED_QUANTIFIER: UErrorCode = 65546i32;
pub const U_MISSING_OPERATOR: UErrorCode = 65547i32;
pub const U_MISSING_RESOURCE_ERROR: UErrorCode = 2i32;
pub const U_MISSING_SEGMENT_CLOSE: UErrorCode = 65548i32;
pub const U_MODIFIER_LETTER: UCharCategory = 4i32;
pub const U_MODIFIER_SYMBOL: UCharCategory = 26i32;
pub const U_MULTIPLE_ANTE_CONTEXTS: UErrorCode = 65549i32;
pub const U_MULTIPLE_COMPOUND_FILTERS: UErrorCode = 65559i32;
pub const U_MULTIPLE_CURSORS: UErrorCode = 65550i32;
pub const U_MULTIPLE_DECIMAL_SEPARATORS: UErrorCode = 65793i32;
pub const U_MULTIPLE_DECIMAL_SEPERATORS: UErrorCode = 65793i32;
pub const U_MULTIPLE_EXPONENTIAL_SYMBOLS: UErrorCode = 65794i32;
pub const U_MULTIPLE_PAD_SPECIFIERS: UErrorCode = 65798i32;
pub const U_MULTIPLE_PERCENT_SYMBOLS: UErrorCode = 65796i32;
pub const U_MULTIPLE_PERMILL_SYMBOLS: UErrorCode = 65797i32;
pub const U_MULTIPLE_POST_CONTEXTS: UErrorCode = 65551i32;
pub const U_NON_SPACING_MARK: UCharCategory = 6i32;
pub const U_NO_DEFAULT_INCLUDE_UTF_HEADERS: u32 = 1u32;
pub const U_NO_SPACE_AVAILABLE: UErrorCode = 20i32;
pub const U_NO_WRITE_PERMISSION: UErrorCode = 30i32;
pub const U_NT_DECIMAL: UNumericType = 1i32;
pub const U_NT_DIGIT: UNumericType = 2i32;
pub const U_NT_NONE: UNumericType = 0i32;
pub const U_NT_NUMERIC: UNumericType = 3i32;
pub const U_NUMBER_ARG_OUTOFBOUNDS_ERROR: UErrorCode = 65810i32;
pub const U_NUMBER_SKELETON_SYNTAX_ERROR: UErrorCode = 65811i32;
pub const U_OMIT_UNCHANGED_TEXT: u32 = 16384u32;
pub const U_OTHER_LETTER: UCharCategory = 5i32;
pub const U_OTHER_NEUTRAL: UCharDirection = 10i32;
pub const U_OTHER_NUMBER: UCharCategory = 11i32;
pub const U_OTHER_PUNCTUATION: UCharCategory = 23i32;
pub const U_OTHER_SYMBOL: UCharCategory = 27i32;
pub const U_OVERRIDE_CXX_ALLOCATION: u32 = 1u32;
pub const U_PARAGRAPH_SEPARATOR: UCharCategory = 14i32;
pub const U_PARSE_CONTEXT_LEN: i32 = 16i32;
pub const U_PARSE_ERROR: UErrorCode = 9i32;
pub const U_PARSE_ERROR_START: UErrorCode = 65536i32;
pub const U_PATTERN_SYNTAX_ERROR: UErrorCode = 65799i32;
pub const U_PF_AIX: u32 = 3100u32;
pub const U_PF_ANDROID: u32 = 4050u32;
pub const U_PF_BROWSER_NATIVE_CLIENT: u32 = 4020u32;
pub const U_PF_BSD: u32 = 3000u32;
pub const U_PF_CYGWIN: u32 = 1900u32;
pub const U_PF_DARWIN: u32 = 3500u32;
pub const U_PF_EMSCRIPTEN: u32 = 5010u32;
pub const U_PF_FUCHSIA: u32 = 4100u32;
pub const U_PF_HPUX: u32 = 2100u32;
pub const U_PF_IPHONE: u32 = 3550u32;
pub const U_PF_IRIX: u32 = 3200u32;
pub const U_PF_LINUX: u32 = 4000u32;
pub const U_PF_MINGW: u32 = 1800u32;
pub const U_PF_OS390: u32 = 9000u32;
pub const U_PF_OS400: u32 = 9400u32;
pub const U_PF_QNX: u32 = 3700u32;
pub const U_PF_SOLARIS: u32 = 2600u32;
pub const U_PF_UNKNOWN: u32 = 0u32;
pub const U_PF_WINDOWS: u32 = 1000u32;
pub const U_PLATFORM: u32 = 1800u32;
pub const U_PLATFORM_HAS_WIN32_API: u32 = 1u32;
pub const U_PLATFORM_HAS_WINUWP_API: u32 = 0u32;
pub const U_PLATFORM_IMPLEMENTS_POSIX: u32 = 0u32;
pub const U_PLATFORM_IS_DARWIN_BASED: u32 = 1u32;
pub const U_PLATFORM_IS_LINUX_BASED: u32 = 1u32;
pub const U_PLATFORM_USES_ONLY_WIN32_API: u32 = 1u32;
pub const U_PLUGIN_CHANGED_LEVEL_WARNING: UErrorCode = -120i32;
pub const U_PLUGIN_DIDNT_SET_LEVEL: UErrorCode = 66817i32;
pub const U_PLUGIN_ERROR_START: UErrorCode = 66816i32;
pub const U_PLUGIN_TOO_HIGH: UErrorCode = 66816i32;
pub const U_POP_DIRECTIONAL_FORMAT: UCharDirection = 16i32;
pub const U_POP_DIRECTIONAL_ISOLATE: UCharDirection = 22i32;
pub const U_PRIMARY_TOO_LONG_ERROR: UErrorCode = 22i32;
pub const U_PRIVATE_USE_CHAR: UCharCategory = 17i32;
pub const U_REGEX_BAD_ESCAPE_SEQUENCE: UErrorCode = 66307i32;
pub const U_REGEX_BAD_INTERVAL: UErrorCode = 66312i32;
pub const U_REGEX_ERROR_START: UErrorCode = 66304i32;
pub const U_REGEX_INTERNAL_ERROR: UErrorCode = 66304i32;
pub const U_REGEX_INVALID_BACK_REF: UErrorCode = 66314i32;
pub const U_REGEX_INVALID_CAPTURE_GROUP_NAME: UErrorCode = 66325i32;
pub const U_REGEX_INVALID_FLAG: UErrorCode = 66315i32;
pub const U_REGEX_INVALID_RANGE: UErrorCode = 66320i32;
pub const U_REGEX_INVALID_STATE: UErrorCode = 66306i32;
pub const U_REGEX_LOOK_BEHIND_LIMIT: UErrorCode = 66316i32;
pub const U_REGEX_MAX_LT_MIN: UErrorCode = 66313i32;
pub const U_REGEX_MISMATCHED_PAREN: UErrorCode = 66310i32;
pub const U_REGEX_MISSING_CLOSE_BRACKET: UErrorCode = 66319i32;
pub const U_REGEX_NUMBER_TOO_BIG: UErrorCode = 66311i32;
pub const U_REGEX_PATTERN_TOO_BIG: UErrorCode = 66324i32;
pub const U_REGEX_PROPERTY_SYNTAX: UErrorCode = 66308i32;
pub const U_REGEX_RULE_SYNTAX: UErrorCode = 66305i32;
pub const U_REGEX_SET_CONTAINS_STRING: UErrorCode = 66317i32;
pub const U_REGEX_STACK_OVERFLOW: UErrorCode = 66321i32;
pub const U_REGEX_STOPPED_BY_CALLER: UErrorCode = 66323i32;
pub const U_REGEX_TIME_OUT: UErrorCode = 66322i32;
pub const U_REGEX_UNIMPLEMENTED: UErrorCode = 66309i32;
pub const U_RESOURCE_TYPE_MISMATCH: UErrorCode = 17i32;
pub const U_RIGHT_TO_LEFT: UCharDirection = 1i32;
pub const U_RIGHT_TO_LEFT_ARABIC: UCharDirection = 13i32;
pub const U_RIGHT_TO_LEFT_EMBEDDING: UCharDirection = 14i32;
pub const U_RIGHT_TO_LEFT_ISOLATE: UCharDirection = 21i32;
pub const U_RIGHT_TO_LEFT_OVERRIDE: UCharDirection = 15i32;
pub const U_RULE_MASK_ERROR: UErrorCode = 65557i32;
pub const U_SAFECLONE_ALLOCATED_WARNING: UErrorCode = -126i32;
pub const U_SB_ATERM: USentenceBreak = 1i32;
pub const U_SB_CLOSE: USentenceBreak = 2i32;
pub const U_SB_CR: USentenceBreak = 11i32;
pub const U_SB_EXTEND: USentenceBreak = 12i32;
pub const U_SB_FORMAT: USentenceBreak = 3i32;
pub const U_SB_LF: USentenceBreak = 13i32;
pub const U_SB_LOWER: USentenceBreak = 4i32;
pub const U_SB_NUMERIC: USentenceBreak = 5i32;
pub const U_SB_OLETTER: USentenceBreak = 6i32;
pub const U_SB_OTHER: USentenceBreak = 0i32;
pub const U_SB_SCONTINUE: USentenceBreak = 14i32;
pub const U_SB_SEP: USentenceBreak = 7i32;
pub const U_SB_SP: USentenceBreak = 8i32;
pub const U_SB_STERM: USentenceBreak = 9i32;
pub const U_SB_UPPER: USentenceBreak = 10i32;
pub const U_SEGMENT_SEPARATOR: UCharDirection = 8i32;
pub const U_SENTINEL: i32 = -1i32;
pub const U_SHAPE_AGGREGATE_TASHKEEL: u32 = 16384u32;
pub const U_SHAPE_AGGREGATE_TASHKEEL_MASK: u32 = 16384u32;
pub const U_SHAPE_AGGREGATE_TASHKEEL_NOOP: u32 = 0u32;
pub const U_SHAPE_DIGITS_ALEN2AN_INIT_AL: u32 = 128u32;
pub const U_SHAPE_DIGITS_ALEN2AN_INIT_LR: u32 = 96u32;
pub const U_SHAPE_DIGITS_AN2EN: u32 = 64u32;
pub const U_SHAPE_DIGITS_EN2AN: u32 = 32u32;
pub const U_SHAPE_DIGITS_MASK: u32 = 224u32;
pub const U_SHAPE_DIGITS_NOOP: u32 = 0u32;
pub const U_SHAPE_DIGITS_RESERVED: u32 = 160u32;
pub const U_SHAPE_DIGIT_TYPE_AN: u32 = 0u32;
pub const U_SHAPE_DIGIT_TYPE_AN_EXTENDED: u32 = 256u32;
pub const U_SHAPE_DIGIT_TYPE_MASK: u32 = 768u32;
pub const U_SHAPE_DIGIT_TYPE_RESERVED: u32 = 512u32;
pub const U_SHAPE_LAMALEF_AUTO: u32 = 65536u32;
pub const U_SHAPE_LAMALEF_BEGIN: u32 = 3u32;
pub const U_SHAPE_LAMALEF_END: u32 = 2u32;
pub const U_SHAPE_LAMALEF_MASK: u32 = 65539u32;
pub const U_SHAPE_LAMALEF_NEAR: u32 = 1u32;
pub const U_SHAPE_LAMALEF_RESIZE: u32 = 0u32;
pub const U_SHAPE_LENGTH_FIXED_SPACES_AT_BEGINNING: u32 = 3u32;
pub const U_SHAPE_LENGTH_FIXED_SPACES_AT_END: u32 = 2u32;
pub const U_SHAPE_LENGTH_FIXED_SPACES_NEAR: u32 = 1u32;
pub const U_SHAPE_LENGTH_GROW_SHRINK: u32 = 0u32;
pub const U_SHAPE_LENGTH_MASK: u32 = 65539u32;
pub const U_SHAPE_LETTERS_MASK: u32 = 24u32;
pub const U_SHAPE_LETTERS_NOOP: u32 = 0u32;
pub const U_SHAPE_LETTERS_SHAPE: u32 = 8u32;
pub const U_SHAPE_LETTERS_SHAPE_TASHKEEL_ISOLATED: u32 = 24u32;
pub const U_SHAPE_LETTERS_UNSHAPE: u32 = 16u32;
pub const U_SHAPE_PRESERVE_PRESENTATION: u32 = 32768u32;
pub const U_SHAPE_PRESERVE_PRESENTATION_MASK: u32 = 32768u32;
pub const U_SHAPE_PRESERVE_PRESENTATION_NOOP: u32 = 0u32;
pub const U_SHAPE_SEEN_MASK: u32 = 7340032u32;
pub const U_SHAPE_SEEN_TWOCELL_NEAR: u32 = 2097152u32;
pub const U_SHAPE_SPACES_RELATIVE_TO_TEXT_BEGIN_END: u32 = 67108864u32;
pub const U_SHAPE_SPACES_RELATIVE_TO_TEXT_MASK: u32 = 67108864u32;
pub const U_SHAPE_TAIL_NEW_UNICODE: u32 = 134217728u32;
pub const U_SHAPE_TAIL_TYPE_MASK: u32 = 134217728u32;
pub const U_SHAPE_TASHKEEL_BEGIN: u32 = 262144u32;
pub const U_SHAPE_TASHKEEL_END: u32 = 393216u32;
pub const U_SHAPE_TASHKEEL_MASK: u32 = 917504u32;
pub const U_SHAPE_TASHKEEL_REPLACE_BY_TATWEEL: u32 = 786432u32;
pub const U_SHAPE_TASHKEEL_RESIZE: u32 = 524288u32;
pub const U_SHAPE_TEXT_DIRECTION_LOGICAL: u32 = 0u32;
pub const U_SHAPE_TEXT_DIRECTION_MASK: u32 = 4u32;
pub const U_SHAPE_TEXT_DIRECTION_VISUAL_LTR: u32 = 4u32;
pub const U_SHAPE_TEXT_DIRECTION_VISUAL_RTL: u32 = 0u32;
pub const U_SHAPE_YEHHAMZA_MASK: u32 = 58720256u32;
pub const U_SHAPE_YEHHAMZA_TWOCELL_NEAR: u32 = 16777216u32;
pub const U_SHORT_PROPERTY_NAME: UPropertyNameChoice = 0i32;
pub const U_SHOW_CPLUSPLUS_API: u32 = 0u32;
pub const U_SIZEOF_UCHAR: u32 = 2u32;
pub const U_SIZEOF_WCHAR_T: u32 = 1u32;
pub const U_SORT_KEY_TOO_SHORT_WARNING: UErrorCode = -123i32;
pub const U_SPACE_SEPARATOR: UCharCategory = 12i32;
pub const U_START_PUNCTUATION: UCharCategory = 20i32;
pub const U_STATE_OLD_WARNING: UErrorCode = -125i32;
pub const U_STATE_TOO_OLD_ERROR: UErrorCode = 23i32;
pub const U_STRINGPREP_CHECK_BIDI_ERROR: UErrorCode = 66562i32;
pub const U_STRINGPREP_PROHIBITED_ERROR: UErrorCode = 66560i32;
pub const U_STRINGPREP_UNASSIGNED_ERROR: UErrorCode = 66561i32;
pub const U_STRING_NOT_TERMINATED_WARNING: UErrorCode = -124i32;
pub const U_SURROGATE: UCharCategory = 18i32;
pub const U_TITLECASE_ADJUST_TO_CASED: u32 = 1024u32;
pub const U_TITLECASE_LETTER: UCharCategory = 3i32;
pub const U_TITLECASE_NO_BREAK_ADJUSTMENT: u32 = 512u32;
pub const U_TITLECASE_NO_LOWERCASE: u32 = 256u32;
pub const U_TITLECASE_SENTENCES: u32 = 64u32;
pub const U_TITLECASE_WHOLE_STRING: u32 = 32u32;
pub const U_TOO_MANY_ALIASES_ERROR: UErrorCode = 24i32;
pub const U_TRAILING_BACKSLASH: UErrorCode = 65552i32;
pub const U_TRUNCATED_CHAR_FOUND: UErrorCode = 11i32;
pub const U_UNASSIGNED: UCharCategory = 0i32;
pub const U_UNCLOSED_SEGMENT: UErrorCode = 65563i32;
pub const U_UNDEFINED_KEYWORD: UErrorCode = 65806i32;
pub const U_UNDEFINED_SEGMENT_REFERENCE: UErrorCode = 65553i32;
pub const U_UNDEFINED_VARIABLE: UErrorCode = 65554i32;
pub const U_UNEXPECTED_TOKEN: UErrorCode = 65792i32;
pub const U_UNICODE_CHAR_NAME: UCharNameChoice = 0i32;
pub const U_UNICODE_VERSION: ::windows_sys::core::PCSTR = ::windows_sys::core::s!("8.0");
pub const U_UNMATCHED_BRACES: UErrorCode = 65801i32;
pub const U_UNQUOTED_SPECIAL: UErrorCode = 65555i32;
pub const U_UNSUPPORTED_ATTRIBUTE: UErrorCode = 65803i32;
pub const U_UNSUPPORTED_ERROR: UErrorCode = 16i32;
pub const U_UNSUPPORTED_ESCAPE_SEQUENCE: UErrorCode = 19i32;
pub const U_UNSUPPORTED_PROPERTY: UErrorCode = 65802i32;
pub const U_UNTERMINATED_QUOTE: UErrorCode = 65556i32;
pub const U_UPPERCASE_LETTER: UCharCategory = 1i32;
pub const U_USELESS_COLLATOR_ERROR: UErrorCode = 29i32;
pub const U_USING_DEFAULT_WARNING: UErrorCode = -127i32;
pub const U_USING_FALLBACK_WARNING: UErrorCode = -128i32;
pub const U_USING_ICU_NAMESPACE: u32 = 1u32;
pub const U_VARIABLE_RANGE_EXHAUSTED: UErrorCode = 65565i32;
pub const U_VARIABLE_RANGE_OVERLAP: UErrorCode = 65566i32;
pub const U_VO_ROTATED: UVerticalOrientation = 0i32;
pub const U_VO_TRANSFORMED_ROTATED: UVerticalOrientation = 1i32;
pub const U_VO_TRANSFORMED_UPRIGHT: UVerticalOrientation = 2i32;
pub const U_VO_UPRIGHT: UVerticalOrientation = 3i32;
pub const U_WB_ALETTER: UWordBreakValues = 1i32;
pub const U_WB_CR: UWordBreakValues = 8i32;
pub const U_WB_DOUBLE_QUOTE: UWordBreakValues = 16i32;
pub const U_WB_EXTEND: UWordBreakValues = 9i32;
pub const U_WB_EXTENDNUMLET: UWordBreakValues = 7i32;
pub const U_WB_E_BASE: UWordBreakValues = 17i32;
pub const U_WB_E_BASE_GAZ: UWordBreakValues = 18i32;
pub const U_WB_E_MODIFIER: UWordBreakValues = 19i32;
pub const U_WB_FORMAT: UWordBreakValues = 2i32;
pub const U_WB_GLUE_AFTER_ZWJ: UWordBreakValues = 20i32;
pub const U_WB_HEBREW_LETTER: UWordBreakValues = 14i32;
pub const U_WB_KATAKANA: UWordBreakValues = 3i32;
pub const U_WB_LF: UWordBreakValues = 10i32;
pub const U_WB_MIDLETTER: UWordBreakValues = 4i32;
pub const U_WB_MIDNUM: UWordBreakValues = 5i32;
pub const U_WB_MIDNUMLET: UWordBreakValues = 11i32;
pub const U_WB_NEWLINE: UWordBreakValues = 12i32;
pub const U_WB_NUMERIC: UWordBreakValues = 6i32;
pub const U_WB_OTHER: UWordBreakValues = 0i32;
pub const U_WB_REGIONAL_INDICATOR: UWordBreakValues = 13i32;
pub const U_WB_SINGLE_QUOTE: UWordBreakValues = 15i32;
pub const U_WB_WSEGSPACE: UWordBreakValues = 22i32;
pub const U_WB_ZWJ: UWordBreakValues = 21i32;
pub const U_WHITE_SPACE_NEUTRAL: UCharDirection = 9i32;
pub const U_ZERO_ERROR: UErrorCode = 0i32;
pub const VS_ALLOW_LATIN: u32 = 1u32;
pub const WC_COMPOSITECHECK: u32 = 512u32;
pub const WC_DEFAULTCHAR: u32 = 64u32;
pub const WC_DISCARDNS: u32 = 16u32;
pub const WC_ERR_INVALID_CHARS: u32 = 128u32;
pub const WC_NO_BEST_FIT_CHARS: u32 = 1024u32;
pub const WC_SEPCHARS: u32 = 32u32;
pub const WORDLIST_TYPE_ADD: WORDLIST_TYPE = 1i32;
pub const WORDLIST_TYPE_AUTOCORRECT: WORDLIST_TYPE = 3i32;
pub const WORDLIST_TYPE_EXCLUDE: WORDLIST_TYPE = 2i32;
pub const WORDLIST_TYPE_IGNORE: WORDLIST_TYPE = 0i32;
pub const sidArabic: SCRIPTCONTF = 9i32;
pub const sidArmenian: SCRIPTCONTF = 7i32;
pub const sidAsciiLatin: SCRIPTCONTF = 3i32;
pub const sidAsciiSym: SCRIPTCONTF = 2i32;
pub const sidBengali: SCRIPTCONTF = 11i32;
pub const sidBopomofo: SCRIPTCONTF = 25i32;
pub const sidBraille: SCRIPTCONTF = 31i32;
pub const sidBurmese: SCRIPTCONTF = 36i32;
pub const sidCanSyllabic: SCRIPTCONTF = 28i32;
pub const sidCherokee: SCRIPTCONTF = 29i32;
pub const sidCyrillic: SCRIPTCONTF = 6i32;
pub const sidDefault: SCRIPTCONTF = 0i32;
pub const sidDevanagari: SCRIPTCONTF = 10i32;
pub const sidEthiopic: SCRIPTCONTF = 27i32;
pub const sidFEFirst: SCRIPTCONTF = 23i32;
pub const sidFELast: SCRIPTCONTF = 26i32;
pub const sidGeorgian: SCRIPTCONTF = 22i32;
pub const sidGreek: SCRIPTCONTF = 5i32;
pub const sidGujarati: SCRIPTCONTF = 13i32;
pub const sidGurmukhi: SCRIPTCONTF = 12i32;
pub const sidHan: SCRIPTCONTF = 26i32;
pub const sidHangul: SCRIPTCONTF = 23i32;
pub const sidHebrew: SCRIPTCONTF = 8i32;
pub const sidKana: SCRIPTCONTF = 24i32;
pub const sidKannada: SCRIPTCONTF = 17i32;
pub const sidKhmer: SCRIPTCONTF = 37i32;
pub const sidLao: SCRIPTCONTF = 20i32;
pub const sidLatin: SCRIPTCONTF = 4i32;
pub const sidLim: SCRIPTCONTF = 41i32;
pub const sidMalayalam: SCRIPTCONTF = 18i32;
pub const sidMerge: SCRIPTCONTF = 1i32;
pub const sidMongolian: SCRIPTCONTF = 39i32;
pub const sidOgham: SCRIPTCONTF = 33i32;
pub const sidOriya: SCRIPTCONTF = 14i32;
pub const sidRunic: SCRIPTCONTF = 32i32;
pub const sidSinhala: SCRIPTCONTF = 34i32;
pub const sidSyriac: SCRIPTCONTF = 35i32;
pub const sidTamil: SCRIPTCONTF = 15i32;
pub const sidTelugu: SCRIPTCONTF = 16i32;
pub const sidThaana: SCRIPTCONTF = 38i32;
pub const sidThai: SCRIPTCONTF = 19i32;
pub const sidTibetan: SCRIPTCONTF = 21i32;
pub const sidUserDefined: SCRIPTCONTF = 40i32;
pub const sidYi: SCRIPTCONTF = 30i32;
pub type COMPARESTRING_RESULT = i32;
pub type COMPARE_STRING_FLAGS = u32;
pub type CORRECTIVE_ACTION = i32;
pub type ENUM_DATE_FORMATS_FLAGS = u32;
pub type ENUM_SYSTEM_CODE_PAGES_FLAGS = u32;
pub type ENUM_SYSTEM_LANGUAGE_GROUPS_FLAGS = u32;
pub type FOLD_STRING_MAP_FLAGS = u32;
pub type IS_TEXT_UNICODE_RESULT = u32;
pub type IS_VALID_LOCALE_FLAGS = u32;
pub type MIMECONTF = i32;
pub type MLCONVCHAR = i32;
pub type MLCP = i32;
pub type MLDETECTCP = i32;
pub type MLSTR_FLAGS = i32;
pub type MULTI_BYTE_TO_WIDE_CHAR_FLAGS = u32;
pub type NORM_FORM = i32;
pub type SCRIPTCONTF = i32;
pub type SCRIPTFONTCONTF = i32;
pub type SCRIPT_IS_COMPLEX_FLAGS = u32;
pub type SCRIPT_JUSTIFY = i32;
pub type SYSGEOCLASS = i32;
pub type SYSGEOTYPE = i32;
pub type SYSNLS_FUNCTION = i32;
pub type TIME_FORMAT_FLAGS = u32;
pub type TRANSLATE_CHARSET_INFO_FLAGS = u32;
pub type UAcceptResult = i32;
pub type UAlphabeticIndexLabelType = i32;
pub type UBiDiDirection = i32;
pub type UBiDiMirroring = i32;
pub type UBiDiOrder = i32;
pub type UBiDiReorderingMode = i32;
pub type UBiDiReorderingOption = i32;
pub type UBidiPairedBracketType = i32;
pub type UBlockCode = i32;
pub type UBreakIteratorType = i32;
pub type UCPMapRangeOption = i32;
pub type UCPTrieType = i32;
pub type UCPTrieValueWidth = i32;
pub type UCalendarAMPMs = i32;
pub type UCalendarAttribute = i32;
pub type UCalendarDateFields = i32;
pub type UCalendarDaysOfWeek = i32;
pub type UCalendarDisplayNameType = i32;
pub type UCalendarLimitType = i32;
pub type UCalendarMonths = i32;
pub type UCalendarType = i32;
pub type UCalendarWallTimeOption = i32;
pub type UCalendarWeekdayType = i32;
pub type UCharCategory = i32;
pub type UCharDirection = i32;
pub type UCharIteratorOrigin = i32;
pub type UCharNameChoice = i32;
pub type UColAttribute = i32;
pub type UColAttributeValue = i32;
pub type UColBoundMode = i32;
pub type UColReorderCode = i32;
pub type UColRuleOption = i32;
pub type UCollationResult = i32;
pub type UConverterCallbackReason = i32;
pub type UConverterPlatform = i32;
pub type UConverterType = i32;
pub type UConverterUnicodeSet = i32;
pub type UCurrCurrencyType = i32;
pub type UCurrNameStyle = i32;
pub type UCurrencySpacing = i32;
pub type UCurrencyUsage = i32;
pub type UDateAbsoluteUnit = i32;
pub type UDateDirection = i32;
pub type UDateFormatBooleanAttribute = i32;
pub type UDateFormatField = i32;
pub type UDateFormatStyle = i32;
pub type UDateFormatSymbolType = i32;
pub type UDateRelativeDateTimeFormatterStyle = i32;
pub type UDateRelativeUnit = i32;
pub type UDateTimePGDisplayWidth = i32;
pub type UDateTimePatternConflict = i32;
pub type UDateTimePatternField = i32;
pub type UDateTimePatternMatchOptions = i32;
pub type UDateTimeScale = i32;
pub type UDecompositionType = i32;
pub type UDialectHandling = i32;
pub type UDisplayContext = i32;
pub type UDisplayContextType = i32;
pub type UEastAsianWidth = i32;
pub type UErrorCode = i32;
pub type UFieldCategory = i32;
pub type UFormattableType = i32;
pub type UGender = i32;
pub type UGraphemeClusterBreak = i32;
pub type UHangulSyllableType = i32;
pub type UIndicPositionalCategory = i32;
pub type UIndicSyllabicCategory = i32;
pub type UJoiningGroup = i32;
pub type UJoiningType = i32;
pub type ULayoutType = i32;
pub type ULineBreak = i32;
pub type ULineBreakTag = i32;
pub type UListFormatterField = i32;
pub type UListFormatterType = i32;
pub type UListFormatterWidth = i32;
pub type ULocAvailableType = i32;
pub type ULocDataLocaleType = i32;
pub type ULocaleDataDelimiterType = i32;
pub type ULocaleDataExemplarSetType = i32;
pub type UMeasureFormatWidth = i32;
pub type UMeasurementSystem = i32;
pub type UMessagePatternApostropheMode = i32;
pub type UMessagePatternArgType = i32;
pub type UMessagePatternPartType = i32;
pub type UNormalization2Mode = i32;
pub type UNormalizationCheckResult = i32;
pub type UNormalizationMode = i32;
pub type UNumberCompactStyle = i32;
pub type UNumberDecimalSeparatorDisplay = i32;
pub type UNumberFormatAttribute = i32;
pub type UNumberFormatAttributeValue = i32;
pub type UNumberFormatFields = i32;
pub type UNumberFormatPadPosition = i32;
pub type UNumberFormatRoundingMode = i32;
pub type UNumberFormatStyle = i32;
pub type UNumberFormatSymbol = i32;
pub type UNumberFormatTextAttribute = i32;
pub type UNumberGroupingStrategy = i32;
pub type UNumberRangeCollapse = i32;
pub type UNumberRangeIdentityFallback = i32;
pub type UNumberRangeIdentityResult = i32;
pub type UNumberSignDisplay = i32;
pub type UNumberUnitWidth = i32;
pub type UNumericType = i32;
pub type UPluralType = i32;
pub type UProperty = i32;
pub type UPropertyNameChoice = i32;
pub type URegexpFlag = i32;
pub type URegionType = i32;
pub type URelativeDateTimeFormatterField = i32;
pub type URelativeDateTimeUnit = i32;
pub type UResType = i32;
pub type URestrictionLevel = i32;
pub type UScriptCode = i32;
pub type UScriptUsage = i32;
pub type USearchAttribute = i32;
pub type USearchAttributeValue = i32;
pub type USentenceBreak = i32;
pub type USentenceBreakTag = i32;
pub type USetSpanCondition = i32;
pub type USpoofChecks = i32;
pub type UStringPrepProfileType = i32;
pub type UStringTrieBuildOption = i32;
pub type UStringTrieResult = i32;
pub type USystemTimeZoneType = i32;
pub type UTimeScaleValue = i32;
pub type UTimeZoneFormatGMTOffsetPatternType = i32;
pub type UTimeZoneFormatParseOption = i32;
pub type UTimeZoneFormatStyle = i32;
pub type UTimeZoneFormatTimeType = i32;
pub type UTimeZoneNameType = i32;
pub type UTimeZoneTransitionType = i32;
pub type UTraceFunctionNumber = i32;
pub type UTraceLevel = i32;
pub type UTransDirection = i32;
pub type UVerticalOrientation = i32;
pub type UWordBreak = i32;
pub type UWordBreakValues = i32;
pub type WORDLIST_TYPE = i32;
#[repr(C)]
pub struct CHARSETINFO {
    pub ciCharset: u32,
    pub ciACP: u32,
    pub fs: FONTSIGNATURE,
}
impl ::core::marker::Copy for CHARSETINFO {}
impl ::core::clone::Clone for CHARSETINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPINFO {
    pub MaxCharSize: u32,
    pub DefaultChar: [u8; 2],
    pub LeadByte: [u8; 12],
}
impl ::core::marker::Copy for CPINFO {}
impl ::core::clone::Clone for CPINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPINFOEXA {
    pub MaxCharSize: u32,
    pub DefaultChar: [u8; 2],
    pub LeadByte: [u8; 12],
    pub UnicodeDefaultChar: u16,
    pub CodePage: u32,
    pub CodePageName: [u8; 260],
}
impl ::core::marker::Copy for CPINFOEXA {}
impl ::core::clone::Clone for CPINFOEXA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CPINFOEXW {
    pub MaxCharSize: u32,
    pub DefaultChar: [u8; 2],
    pub LeadByte: [u8; 12],
    pub UnicodeDefaultChar: u16,
    pub CodePage: u32,
    pub CodePageName: [u16; 260],
}
impl ::core::marker::Copy for CPINFOEXW {}
impl ::core::clone::Clone for CPINFOEXW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CURRENCYFMTA {
    pub NumDigits: u32,
    pub LeadingZero: u32,
    pub Grouping: u32,
    pub lpDecimalSep: ::windows_sys::core::PSTR,
    pub lpThousandSep: ::windows_sys::core::PSTR,
    pub NegativeOrder: u32,
    pub PositiveOrder: u32,
    pub lpCurrencySymbol: ::windows_sys::core::PSTR,
}
impl ::core::marker::Copy for CURRENCYFMTA {}
impl ::core::clone::Clone for CURRENCYFMTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct CURRENCYFMTW {
    pub NumDigits: u32,
    pub LeadingZero: u32,
    pub Grouping: u32,
    pub lpDecimalSep: ::windows_sys::core::PWSTR,
    pub lpThousandSep: ::windows_sys::core::PWSTR,
    pub NegativeOrder: u32,
    pub PositiveOrder: u32,
    pub lpCurrencySymbol: ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for CURRENCYFMTW {}
impl ::core::clone::Clone for CURRENCYFMTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct DetectEncodingInfo {
    pub nLangID: u32,
    pub nCodePage: u32,
    pub nDocPercent: i32,
    pub nConfidence: i32,
}
impl ::core::marker::Copy for DetectEncodingInfo {}
impl ::core::clone::Clone for DetectEncodingInfo {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct ENUMTEXTMETRICA {
    pub etmNewTextMetricEx: NEWTEXTMETRICEXA,
    pub etmAxesList: super::Graphics::Gdi::AXESLISTA,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for ENUMTEXTMETRICA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for ENUMTEXTMETRICA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct ENUMTEXTMETRICW {
    pub etmNewTextMetricEx: NEWTEXTMETRICEXW,
    pub etmAxesList: super::Graphics::Gdi::AXESLISTW,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for ENUMTEXTMETRICW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for ENUMTEXTMETRICW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FILEMUIINFO {
    pub dwSize: u32,
    pub dwVersion: u32,
    pub dwFileType: u32,
    pub pChecksum: [u8; 16],
    pub pServiceChecksum: [u8; 16],
    pub dwLanguageNameOffset: u32,
    pub dwTypeIDMainSize: u32,
    pub dwTypeIDMainOffset: u32,
    pub dwTypeNameMainOffset: u32,
    pub dwTypeIDMUISize: u32,
    pub dwTypeIDMUIOffset: u32,
    pub dwTypeNameMUIOffset: u32,
    pub abBuffer: [u8; 8],
}
impl ::core::marker::Copy for FILEMUIINFO {}
impl ::core::clone::Clone for FILEMUIINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct FONTSIGNATURE {
    pub fsUsb: [u32; 4],
    pub fsCsb: [u32; 2],
}
impl ::core::marker::Copy for FONTSIGNATURE {}
impl ::core::clone::Clone for FONTSIGNATURE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct GOFFSET {
    pub du: i32,
    pub dv: i32,
}
impl ::core::marker::Copy for GOFFSET {}
impl ::core::clone::Clone for GOFFSET {
    fn clone(&self) -> Self {
        *self
    }
}
pub type HIMC = isize;
pub type HIMCC = isize;
pub type HSAVEDUILANGUAGES = isize;
#[repr(C)]
pub struct LOCALESIGNATURE {
    pub lsUsb: [u32; 4],
    pub lsCsbDefault: [u32; 2],
    pub lsCsbSupported: [u32; 2],
}
impl ::core::marker::Copy for LOCALESIGNATURE {}
impl ::core::clone::Clone for LOCALESIGNATURE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPPING_DATA_RANGE {
    pub dwStartIndex: u32,
    pub dwEndIndex: u32,
    pub pszDescription: ::windows_sys::core::PWSTR,
    pub dwDescriptionLength: u32,
    pub pData: *mut ::core::ffi::c_void,
    pub dwDataSize: u32,
    pub pszContentType: ::windows_sys::core::PWSTR,
    pub prgActionIds: *mut ::windows_sys::core::PWSTR,
    pub dwActionsCount: u32,
    pub prgActionDisplayNames: *mut ::windows_sys::core::PWSTR,
}
impl ::core::marker::Copy for MAPPING_DATA_RANGE {}
impl ::core::clone::Clone for MAPPING_DATA_RANGE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPPING_ENUM_OPTIONS {
    pub Size: usize,
    pub pszCategory: ::windows_sys::core::PWSTR,
    pub pszInputLanguage: ::windows_sys::core::PWSTR,
    pub pszOutputLanguage: ::windows_sys::core::PWSTR,
    pub pszInputScript: ::windows_sys::core::PWSTR,
    pub pszOutputScript: ::windows_sys::core::PWSTR,
    pub pszInputContentType: ::windows_sys::core::PWSTR,
    pub pszOutputContentType: ::windows_sys::core::PWSTR,
    pub pGuid: *mut ::windows_sys::core::GUID,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for MAPPING_ENUM_OPTIONS {}
impl ::core::clone::Clone for MAPPING_ENUM_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPPING_OPTIONS {
    pub Size: usize,
    pub pszInputLanguage: ::windows_sys::core::PWSTR,
    pub pszOutputLanguage: ::windows_sys::core::PWSTR,
    pub pszInputScript: ::windows_sys::core::PWSTR,
    pub pszOutputScript: ::windows_sys::core::PWSTR,
    pub pszInputContentType: ::windows_sys::core::PWSTR,
    pub pszOutputContentType: ::windows_sys::core::PWSTR,
    pub pszUILanguage: ::windows_sys::core::PWSTR,
    pub pfnRecognizeCallback: PFN_MAPPINGCALLBACKPROC,
    pub pRecognizeCallerData: *mut ::core::ffi::c_void,
    pub dwRecognizeCallerDataSize: u32,
    pub pfnActionCallback: PFN_MAPPINGCALLBACKPROC,
    pub pActionCallerData: *mut ::core::ffi::c_void,
    pub dwActionCallerDataSize: u32,
    pub dwServiceFlag: u32,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for MAPPING_OPTIONS {}
impl ::core::clone::Clone for MAPPING_OPTIONS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPPING_PROPERTY_BAG {
    pub Size: usize,
    pub prgResultRanges: *mut MAPPING_DATA_RANGE,
    pub dwRangesCount: u32,
    pub pServiceData: *mut ::core::ffi::c_void,
    pub dwServiceDataSize: u32,
    pub pCallerData: *mut ::core::ffi::c_void,
    pub dwCallerDataSize: u32,
    pub pContext: *mut ::core::ffi::c_void,
}
impl ::core::marker::Copy for MAPPING_PROPERTY_BAG {}
impl ::core::clone::Clone for MAPPING_PROPERTY_BAG {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MAPPING_SERVICE_INFO {
    pub Size: usize,
    pub pszCopyright: ::windows_sys::core::PWSTR,
    pub wMajorVersion: u16,
    pub wMinorVersion: u16,
    pub wBuildVersion: u16,
    pub wStepVersion: u16,
    pub dwInputContentTypesCount: u32,
    pub prgInputContentTypes: *mut ::windows_sys::core::PWSTR,
    pub dwOutputContentTypesCount: u32,
    pub prgOutputContentTypes: *mut ::windows_sys::core::PWSTR,
    pub dwInputLanguagesCount: u32,
    pub prgInputLanguages: *mut ::windows_sys::core::PWSTR,
    pub dwOutputLanguagesCount: u32,
    pub prgOutputLanguages: *mut ::windows_sys::core::PWSTR,
    pub dwInputScriptsCount: u32,
    pub prgInputScripts: *mut ::windows_sys::core::PWSTR,
    pub dwOutputScriptsCount: u32,
    pub prgOutputScripts: *mut ::windows_sys::core::PWSTR,
    pub guid: ::windows_sys::core::GUID,
    pub pszCategory: ::windows_sys::core::PWSTR,
    pub pszDescription: ::windows_sys::core::PWSTR,
    pub dwPrivateDataSize: u32,
    pub pPrivateData: *mut ::core::ffi::c_void,
    pub pContext: *mut ::core::ffi::c_void,
    pub _bitfield: u32,
}
impl ::core::marker::Copy for MAPPING_SERVICE_INFO {}
impl ::core::clone::Clone for MAPPING_SERVICE_INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIMECPINFO {
    pub dwFlags: u32,
    pub uiCodePage: u32,
    pub uiFamilyCodePage: u32,
    pub wszDescription: [u16; 64],
    pub wszWebCharset: [u16; 50],
    pub wszHeaderCharset: [u16; 50],
    pub wszBodyCharset: [u16; 50],
    pub wszFixedWidthFont: [u16; 32],
    pub wszProportionalFont: [u16; 32],
    pub bGDICharset: u8,
}
impl ::core::marker::Copy for MIMECPINFO {}
impl ::core::clone::Clone for MIMECPINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct MIMECSETINFO {
    pub uiCodePage: u32,
    pub uiInternetEncoding: u32,
    pub wszCharset: [u16; 50],
}
impl ::core::marker::Copy for MIMECSETINFO {}
impl ::core::clone::Clone for MIMECSETINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct NEWTEXTMETRICEXA {
    pub ntmTm: super::Graphics::Gdi::NEWTEXTMETRICA,
    pub ntmFontSig: FONTSIGNATURE,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for NEWTEXTMETRICEXA {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for NEWTEXTMETRICEXA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
#[doc = "Required features: `\"Win32_Graphics_Gdi\"`"]
#[cfg(feature = "Win32_Graphics_Gdi")]
pub struct NEWTEXTMETRICEXW {
    pub ntmTm: super::Graphics::Gdi::NEWTEXTMETRICW,
    pub ntmFontSig: FONTSIGNATURE,
}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::marker::Copy for NEWTEXTMETRICEXW {}
#[cfg(feature = "Win32_Graphics_Gdi")]
impl ::core::clone::Clone for NEWTEXTMETRICEXW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NLSVERSIONINFO {
    pub dwNLSVersionInfoSize: u32,
    pub dwNLSVersion: u32,
    pub dwDefinedVersion: u32,
    pub dwEffectiveId: u32,
    pub guidCustomVersion: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for NLSVERSIONINFO {}
impl ::core::clone::Clone for NLSVERSIONINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NLSVERSIONINFOEX {
    pub dwNLSVersionInfoSize: u32,
    pub dwNLSVersion: u32,
    pub dwDefinedVersion: u32,
    pub dwEffectiveId: u32,
    pub guidCustomVersion: ::windows_sys::core::GUID,
}
impl ::core::marker::Copy for NLSVERSIONINFOEX {}
impl ::core::clone::Clone for NLSVERSIONINFOEX {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NUMBERFMTA {
    pub NumDigits: u32,
    pub LeadingZero: u32,
    pub Grouping: u32,
    pub lpDecimalSep: ::windows_sys::core::PSTR,
    pub lpThousandSep: ::windows_sys::core::PSTR,
    pub NegativeOrder: u32,
}
impl ::core::marker::Copy for NUMBERFMTA {}
impl ::core::clone::Clone for NUMBERFMTA {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct NUMBERFMTW {
    pub NumDigits: u32,
    pub LeadingZero: u32,
    pub Grouping: u32,
    pub lpDecimalSep: ::windows_sys::core::PWSTR,
    pub lpThousandSep: ::windows_sys::core::PWSTR,
    pub NegativeOrder: u32,
}
impl ::core::marker::Copy for NUMBERFMTW {}
impl ::core::clone::Clone for NUMBERFMTW {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct OPENTYPE_FEATURE_RECORD {
    pub tagFeature: u32,
    pub lParameter: i32,
}
impl ::core::marker::Copy for OPENTYPE_FEATURE_RECORD {}
impl ::core::clone::Clone for OPENTYPE_FEATURE_RECORD {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct RFC1766INFO {
    pub lcid: u32,
    pub wszRfc1766: [u16; 6],
    pub wszLocaleName: [u16; 32],
}
impl ::core::marker::Copy for RFC1766INFO {}
impl ::core::clone::Clone for RFC1766INFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPTFONTINFO {
    pub scripts: i64,
    pub wszFont: [u16; 32],
}
impl ::core::marker::Copy for SCRIPTFONTINFO {}
impl ::core::clone::Clone for SCRIPTFONTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPTINFO {
    pub ScriptId: u8,
    pub uiCodePage: u32,
    pub wszDescription: [u16; 48],
    pub wszFixedWidthFont: [u16; 32],
    pub wszProportionalFont: [u16; 32],
}
impl ::core::marker::Copy for SCRIPTINFO {}
impl ::core::clone::Clone for SCRIPTINFO {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_ANALYSIS {
    pub _bitfield: u16,
    pub s: SCRIPT_STATE,
}
impl ::core::marker::Copy for SCRIPT_ANALYSIS {}
impl ::core::clone::Clone for SCRIPT_ANALYSIS {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_CHARPROP {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for SCRIPT_CHARPROP {}
impl ::core::clone::Clone for SCRIPT_CHARPROP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_CONTROL {
    pub _bitfield: u32,
}
impl ::core::marker::Copy for SCRIPT_CONTROL {}
impl ::core::clone::Clone for SCRIPT_CONTROL {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_DIGITSUBSTITUTE {
    pub _bitfield1: u32,
    pub _bitfield2: u32,
    pub dwReserved: u32,
}
impl ::core::marker::Copy for SCRIPT_DIGITSUBSTITUTE {}
impl ::core::clone::Clone for SCRIPT_DIGITSUBSTITUTE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_FONTPROPERTIES {
    pub cBytes: i32,
    pub wgBlank: u16,
    pub wgDefault: u16,
    pub wgInvalid: u16,
    pub wgKashida: u16,
    pub iKashidaWidth: i32,
}
impl ::core::marker::Copy for SCRIPT_FONTPROPERTIES {}
impl ::core::clone::Clone for SCRIPT_FONTPROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_GLYPHPROP {
    pub sva: SCRIPT_VISATTR,
    pub reserved: u16,
}
impl ::core::marker::Copy for SCRIPT_GLYPHPROP {}
impl ::core::clone::Clone for SCRIPT_GLYPHPROP {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_ITEM {
    pub iCharPos: i32,
    pub a: SCRIPT_ANALYSIS,
}
impl ::core::marker::Copy for SCRIPT_ITEM {}
impl ::core::clone::Clone for SCRIPT_ITEM {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_LOGATTR {
    pub _bitfield: u8,
}
impl ::core::marker::Copy for SCRIPT_LOGATTR {}
impl ::core::clone::Clone for SCRIPT_LOGATTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_PROPERTIES {
    pub _bitfield1: u32,
    pub _bitfield2: u32,
}
impl ::core::marker::Copy for SCRIPT_PROPERTIES {}
impl ::core::clone::Clone for SCRIPT_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_STATE {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for SCRIPT_STATE {}
impl ::core::clone::Clone for SCRIPT_STATE {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_TABDEF {
    pub cTabStops: i32,
    pub iScale: i32,
    pub pTabStops: *mut i32,
    pub iTabOrigin: i32,
}
impl ::core::marker::Copy for SCRIPT_TABDEF {}
impl ::core::clone::Clone for SCRIPT_TABDEF {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct SCRIPT_VISATTR {
    pub _bitfield: u16,
}
impl ::core::marker::Copy for SCRIPT_VISATTR {}
impl ::core::clone::Clone for SCRIPT_VISATTR {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct TEXTRANGE_PROPERTIES {
    pub potfRecords: *mut OPENTYPE_FEATURE_RECORD,
    pub cotfRecords: i32,
}
impl ::core::marker::Copy for TEXTRANGE_PROPERTIES {}
impl ::core::clone::Clone for TEXTRANGE_PROPERTIES {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UBiDi = isize;
pub type UBiDiTransform = isize;
pub type UBreakIterator = isize;
pub type UCPMap = isize;
#[repr(C)]
pub struct UCPTrie {
    pub index: *const u16,
    pub data: UCPTrieData,
    pub indexLength: i32,
    pub dataLength: i32,
    pub highStart: i32,
    pub shifted12HighStart: u16,
    pub r#type: i8,
    pub valueWidth: i8,
    pub reserved32: u32,
    pub reserved16: u16,
    pub index3NullOffset: u16,
    pub dataNullOffset: i32,
    pub nullValue: u32,
}
impl ::core::marker::Copy for UCPTrie {}
impl ::core::clone::Clone for UCPTrie {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub union UCPTrieData {
    pub ptr0: *const ::core::ffi::c_void,
    pub ptr16: *const u16,
    pub ptr32: *const u32,
    pub ptr8: *const u8,
}
impl ::core::marker::Copy for UCPTrieData {}
impl ::core::clone::Clone for UCPTrieData {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UCaseMap = isize;
#[repr(C)]
pub struct UCharIterator {
    pub context: *const ::core::ffi::c_void,
    pub length: i32,
    pub start: i32,
    pub index: i32,
    pub limit: i32,
    pub reservedField: i32,
    pub getIndex: UCharIteratorGetIndex,
    pub r#move: UCharIteratorMove,
    pub hasNext: UCharIteratorHasNext,
    pub hasPrevious: UCharIteratorHasPrevious,
    pub current: UCharIteratorCurrent,
    pub next: UCharIteratorNext,
    pub previous: UCharIteratorPrevious,
    pub reservedFn: UCharIteratorReserved,
    pub getState: UCharIteratorGetState,
    pub setState: UCharIteratorSetState,
}
impl ::core::marker::Copy for UCharIterator {}
impl ::core::clone::Clone for UCharIterator {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UCharsetDetector = isize;
pub type UCharsetMatch = isize;
pub type UCollationElements = isize;
pub type UCollator = isize;
pub type UConstrainedFieldPosition = isize;
pub type UConverter = isize;
#[repr(C)]
pub struct UConverterFromUnicodeArgs {
    pub size: u16,
    pub flush: i8,
    pub converter: *mut UConverter,
    pub source: *const u16,
    pub sourceLimit: *const u16,
    pub target: ::windows_sys::core::PSTR,
    pub targetLimit: ::windows_sys::core::PCSTR,
    pub offsets: *mut i32,
}
impl ::core::marker::Copy for UConverterFromUnicodeArgs {}
impl ::core::clone::Clone for UConverterFromUnicodeArgs {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UConverterSelector = isize;
#[repr(C)]
pub struct UConverterToUnicodeArgs {
    pub size: u16,
    pub flush: i8,
    pub converter: *mut UConverter,
    pub source: ::windows_sys::core::PCSTR,
    pub sourceLimit: ::windows_sys::core::PCSTR,
    pub target: *mut u16,
    pub targetLimit: *const u16,
    pub offsets: *mut i32,
}
impl ::core::marker::Copy for UConverterToUnicodeArgs {}
impl ::core::clone::Clone for UConverterToUnicodeArgs {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UDateFormatSymbols = isize;
pub type UDateIntervalFormat = isize;
pub type UEnumeration = isize;
#[repr(C)]
pub struct UFieldPosition {
    pub field: i32,
    pub beginIndex: i32,
    pub endIndex: i32,
}
impl ::core::marker::Copy for UFieldPosition {}
impl ::core::clone::Clone for UFieldPosition {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UFieldPositionIterator = isize;
pub type UFormattedDateInterval = isize;
pub type UFormattedList = isize;
pub type UFormattedNumber = isize;
pub type UFormattedNumberRange = isize;
pub type UFormattedRelativeDateTime = isize;
pub type UFormattedValue = isize;
pub type UGenderInfo = isize;
pub type UHashtable = isize;
pub type UIDNA = isize;
#[repr(C)]
pub struct UIDNAInfo {
    pub size: i16,
    pub isTransitionalDifferent: i8,
    pub reservedB3: i8,
    pub errors: u32,
    pub reservedI2: i32,
    pub reservedI3: i32,
}
impl ::core::marker::Copy for UIDNAInfo {}
impl ::core::clone::Clone for UIDNAInfo {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UListFormatter = isize;
pub type ULocaleData = isize;
pub type ULocaleDisplayNames = isize;
pub type UMutableCPTrie = isize;
#[repr(C)]
pub struct UNICODERANGE {
    pub wcFrom: u16,
    pub wcTo: u16,
}
impl ::core::marker::Copy for UNICODERANGE {}
impl ::core::clone::Clone for UNICODERANGE {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UNormalizer2 = isize;
pub type UNumberFormatter = isize;
pub type UNumberingSystem = isize;
#[repr(C)]
pub struct UParseError {
    pub line: i32,
    pub offset: i32,
    pub preContext: [u16; 16],
    pub postContext: [u16; 16],
}
impl ::core::marker::Copy for UParseError {}
impl ::core::clone::Clone for UParseError {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UPluralRules = isize;
pub type URegion = isize;
pub type URegularExpression = isize;
pub type URelativeDateTimeFormatter = isize;
#[repr(C)]
pub struct UReplaceableCallbacks {
    pub length: isize,
    pub charAt: isize,
    pub char32At: isize,
    pub replace: isize,
    pub extract: isize,
    pub copy: isize,
}
impl ::core::marker::Copy for UReplaceableCallbacks {}
impl ::core::clone::Clone for UReplaceableCallbacks {
    fn clone(&self) -> Self {
        *self
    }
}
pub type UResourceBundle = isize;
pub type USearch = isize;
#[repr(C)]
pub struct USerializedSet {
    pub array: *const u16,
    pub bmpLength: i32,
    pub length: i32,
    pub staticArray: [u16; 8],
}
impl ::core::marker::Copy for USerializedSet {}
impl ::core::clone::Clone for USerializedSet {
    fn clone(&self) -> Self {
        *self
    }
}
pub type USet = isize;
pub type USpoofCheckResult = isize;
pub type USpoofChecker = isize;
pub type UStringPrepProfile = isize;
pub type UStringSearch = isize;
#[repr(C)]
pub struct UText {
    pub magic: u32,
    pub flags: i32,
    pub providerProperties: i32,
    pub sizeOfStruct: i32,
    pub chunkNativeLimit: i64,
    pub extraSize: i32,
    pub nativeIndexingLimit: i32,
    pub chunkNativeStart: i64,
    pub chunkOffset: i32,
    pub chunkLength: i32,
    pub chunkContents: *const u16,
    pub pFuncs: *const UTextFuncs,
    pub pExtra: *mut ::core::ffi::c_void,
    pub context: *const ::core::ffi::c_void,
    pub p: *const ::core::ffi::c_void,
    pub q: *const ::core::ffi::c_void,
    pub r: *const ::core::ffi::c_void,
    pub privP: *mut ::core::ffi::c_void,
    pub a: i64,
    pub b: i32,
    pub c: i32,
    pub privA: i64,
    pub privB: i32,
    pub privC: i32,
}
impl ::core::marker::Copy for UText {}
impl ::core::clone::Clone for UText {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UTextFuncs {
    pub tableSize: i32,
    pub reserved1: i32,
    pub reserved2: i32,
    pub reserved3: i32,
    pub clone: UTextClone,
    pub nativeLength: UTextNativeLength,
    pub access: UTextAccess,
    pub extract: UTextExtract,
    pub replace: UTextReplace,
    pub copy: UTextCopy,
    pub mapOffsetToNative: UTextMapOffsetToNative,
    pub mapNativeIndexToUTF16: UTextMapNativeIndexToUTF16,
    pub close: UTextClose,
    pub spare1: UTextClose,
    pub spare2: UTextClose,
    pub spare3: UTextClose,
}
impl ::core::marker::Copy for UTextFuncs {}
impl ::core::clone::Clone for UTextFuncs {
    fn clone(&self) -> Self {
        *self
    }
}
#[repr(C)]
pub struct UTransPosition {
    pub contextStart: i32,
    pub contextLimit: i32,
    pub start: i32,
    pub limit: i32,
}
impl ::core::marker::Copy for UTransPosition {}
impl ::core::clone::Clone for UTransPosition {
    fn clone(&self) -> Self {
        *self
    }
}
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CALINFO_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CALINFO_ENUMPROCEXA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: u32) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CALINFO_ENUMPROCEXEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32, param2: ::windows_sys::core::PCWSTR, param3: super::Foundation::LPARAM) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CALINFO_ENUMPROCEXW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CALINFO_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CODEPAGE_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type CODEPAGE_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DATEFMT_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DATEFMT_ENUMPROCEXA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: u32) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DATEFMT_ENUMPROCEXEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32, param2: super::Foundation::LPARAM) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DATEFMT_ENUMPROCEXW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type DATEFMT_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type GEO_ENUMNAMEPROC = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: super::Foundation::LPARAM) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type GEO_ENUMPROC = ::core::option::Option<unsafe extern "system" fn(param0: i32) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LANGGROUPLOCALE_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: u32, param1: u32, param2: ::windows_sys::core::PCSTR, param3: isize) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LANGGROUPLOCALE_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: u32, param1: u32, param2: ::windows_sys::core::PCWSTR, param3: isize) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LANGUAGEGROUP_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: u32, param1: ::windows_sys::core::PCSTR, param2: ::windows_sys::core::PCSTR, param3: u32, param4: isize) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LANGUAGEGROUP_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: u32, param1: ::windows_sys::core::PCWSTR, param2: ::windows_sys::core::PCWSTR, param3: u32, param4: isize) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LOCALE_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LOCALE_ENUMPROCEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: u32, param2: super::Foundation::LPARAM) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type LOCALE_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::Foundation::BOOL>;
pub type PFN_MAPPINGCALLBACKPROC = ::core::option::Option<unsafe extern "system" fn(pbag: *mut MAPPING_PROPERTY_BAG, data: *mut ::core::ffi::c_void, dwdatasize: u32, result: ::windows_sys::core::HRESULT) -> ()>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TIMEFMT_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TIMEFMT_ENUMPROCEX = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: super::Foundation::LPARAM) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type TIMEFMT_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR) -> super::Foundation::BOOL>;
pub type UBiDiClassCallback = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, c: i32) -> UCharDirection>;
pub type UCPMapValueFilter = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, value: u32) -> u32>;
pub type UCharEnumTypeRange = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, start: i32, limit: i32, r#type: UCharCategory) -> i8>;
pub type UCharIteratorCurrent = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator) -> i32>;
pub type UCharIteratorGetIndex = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator, origin: UCharIteratorOrigin) -> i32>;
pub type UCharIteratorGetState = ::core::option::Option<unsafe extern "system" fn(iter: *const UCharIterator) -> u32>;
pub type UCharIteratorHasNext = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator) -> i8>;
pub type UCharIteratorHasPrevious = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator) -> i8>;
pub type UCharIteratorMove = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator, delta: i32, origin: UCharIteratorOrigin) -> i32>;
pub type UCharIteratorNext = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator) -> i32>;
pub type UCharIteratorPrevious = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator) -> i32>;
pub type UCharIteratorReserved = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator, something: i32) -> i32>;
pub type UCharIteratorSetState = ::core::option::Option<unsafe extern "system" fn(iter: *mut UCharIterator, state: u32, perrorcode: *mut UErrorCode) -> ()>;
pub type UConverterFromUCallback = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, args: *mut UConverterFromUnicodeArgs, codeunits: *const u16, length: i32, codepoint: i32, reason: UConverterCallbackReason, perrorcode: *mut UErrorCode) -> ()>;
pub type UConverterToUCallback = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, args: *mut UConverterToUnicodeArgs, codeunits: ::windows_sys::core::PCSTR, length: i32, reason: UConverterCallbackReason, perrorcode: *mut UErrorCode) -> ()>;
pub type UEnumCharNamesFn = ::core::option::Option<unsafe extern "system" fn(context: *mut ::core::ffi::c_void, code: i32, namechoice: UCharNameChoice, name: ::windows_sys::core::PCSTR, length: i32) -> i8>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type UILANGUAGE_ENUMPROCA = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCSTR, param1: isize) -> super::Foundation::BOOL>;
#[doc = "Required features: `\"Win32_Foundation\"`"]
#[cfg(feature = "Win32_Foundation")]
pub type UILANGUAGE_ENUMPROCW = ::core::option::Option<unsafe extern "system" fn(param0: ::windows_sys::core::PCWSTR, param1: isize) -> super::Foundation::BOOL>;
pub type UMemAllocFn = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, size: usize) -> *mut ::core::ffi::c_void>;
pub type UMemFreeFn = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, mem: *mut ::core::ffi::c_void) -> ()>;
pub type UMemReallocFn = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, mem: *mut ::core::ffi::c_void, size: usize) -> *mut ::core::ffi::c_void>;
pub type UNESCAPE_CHAR_AT = ::core::option::Option<unsafe extern "system" fn(offset: i32, context: *mut ::core::ffi::c_void) -> u16>;
pub type URegexFindProgressCallback = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, matchindex: i64) -> i8>;
pub type URegexMatchCallback = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, steps: i32) -> i8>;
pub type UStringCaseMapper = ::core::option::Option<unsafe extern "system" fn(csm: *const UCaseMap, dest: *mut u16, destcapacity: i32, src: *const u16, srclength: i32, perrorcode: *mut UErrorCode) -> i32>;
pub type UTextAccess = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText, nativeindex: i64, forward: i8) -> i8>;
pub type UTextClone = ::core::option::Option<unsafe extern "system" fn(dest: *mut UText, src: *const UText, deep: i8, status: *mut UErrorCode) -> *mut UText>;
pub type UTextClose = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText) -> ()>;
pub type UTextCopy = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText, nativestart: i64, nativelimit: i64, nativedest: i64, r#move: i8, status: *mut UErrorCode) -> ()>;
pub type UTextExtract = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText, nativestart: i64, nativelimit: i64, dest: *mut u16, destcapacity: i32, status: *mut UErrorCode) -> i32>;
pub type UTextMapNativeIndexToUTF16 = ::core::option::Option<unsafe extern "system" fn(ut: *const UText, nativeindex: i64) -> i32>;
pub type UTextMapOffsetToNative = ::core::option::Option<unsafe extern "system" fn(ut: *const UText) -> i64>;
pub type UTextNativeLength = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText) -> i64>;
pub type UTextReplace = ::core::option::Option<unsafe extern "system" fn(ut: *mut UText, nativestart: i64, nativelimit: i64, replacementtext: *const u16, replacmentlength: i32, status: *mut UErrorCode) -> i32>;
pub type UTraceData = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, fnnumber: i32, level: i32, fmt: ::windows_sys::core::PCSTR, args: *mut i8) -> ()>;
pub type UTraceEntry = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, fnnumber: i32) -> ()>;
pub type UTraceExit = ::core::option::Option<unsafe extern "system" fn(context: *const ::core::ffi::c_void, fnnumber: i32, fmt: ::windows_sys::core::PCSTR, args: *mut i8) -> ()>;
