use std::{
    collections::HashMap, ffi::CStr, fmt, os::raw::c_char, process::abort, slice, str,
    sync::atomic::AtomicUsize,
};

use regex::Regex;
use tree_sitter::Language;

use super::{Error, Highlight, HighlightConfiguration, Highlighter, HtmlRenderer};

pub struct TSHighlighter {
    pub languages: HashMap<String, (Option<Regex>, HighlightConfiguration)>,
    pub attribute_strings: Vec<&'static [u8]>,
    pub highlight_names: Vec<String>,
    pub carriage_return_index: Option<usize>,
}

pub struct TSHighlightBuffer {
    highlighter: Highlighter,
    renderer: HtmlRenderer,
}

#[repr(C)]
pub enum ErrorCode {
    Ok,
    UnknownScope,
    Timeout,
    InvalidLanguage,
    InvalidUtf8,
    InvalidRegex,
    InvalidQuery,
    InvalidLanguageName,
}

/// Create a new [`TSHighlighter`] instance.
///
/// # Safety
///
/// The caller must ensure that the `highlight_names` and `attribute_strings` arrays are valid for
/// the lifetime of the returned [`TSHighlighter`] instance, and are non-null.
#[no_mangle]
pub unsafe extern "C" fn ts_highlighter_new(
    highlight_names: *const *const c_char,
    attribute_strings: *const *const c_char,
    highlight_count: u32,
) -> *mut TSHighlighter {
    let highlight_names = slice::from_raw_parts(highlight_names, highlight_count as usize);
    let attribute_strings = slice::from_raw_parts(attribute_strings, highlight_count as usize);
    let highlight_names = highlight_names
        .iter()
        .map(|s| CStr::from_ptr(*s).to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let attribute_strings = attribute_strings
        .iter()
        .map(|s| CStr::from_ptr(*s).to_bytes())
        .collect();
    let carriage_return_index = highlight_names.iter().position(|s| s == "carriage-return");
    Box::into_raw(Box::new(TSHighlighter {
        languages: HashMap::new(),
        attribute_strings,
        highlight_names,
        carriage_return_index,
    }))
}

/// Add a language to a [`TSHighlighter`] instance.
///
/// Returns an [`ErrorCode`] indicating whether the language was added successfully or not.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlighter`] instance
/// created by [`ts_highlighter_new`].
///
/// The caller must ensure that any `*const c_char` (C-style string) parameters are valid for the
/// lifetime of the [`TSHighlighter`] instance, and are non-null.
#[no_mangle]
pub unsafe extern "C" fn ts_highlighter_add_language(
    this: *mut TSHighlighter,
    language_name: *const c_char,
    scope_name: *const c_char,
    injection_regex: *const c_char,
    language: Language,
    highlight_query: *const c_char,
    injection_query: *const c_char,
    locals_query: *const c_char,
    highlight_query_len: u32,
    injection_query_len: u32,
    locals_query_len: u32,
) -> ErrorCode {
    let f = move || {
        let this = unwrap_mut_ptr(this);
        let scope_name = CStr::from_ptr(scope_name);
        let scope_name = scope_name
            .to_str()
            .or(Err(ErrorCode::InvalidUtf8))?
            .to_string();
        let injection_regex = if injection_regex.is_null() {
            None
        } else {
            let pattern = CStr::from_ptr(injection_regex);
            let pattern = pattern.to_str().or(Err(ErrorCode::InvalidUtf8))?;
            Some(Regex::new(pattern).or(Err(ErrorCode::InvalidRegex))?)
        };

        let highlight_query =
            slice::from_raw_parts(highlight_query.cast::<u8>(), highlight_query_len as usize);

        let highlight_query = str::from_utf8(highlight_query).or(Err(ErrorCode::InvalidUtf8))?;

        let injection_query = if injection_query_len > 0 {
            let query =
                slice::from_raw_parts(injection_query.cast::<u8>(), injection_query_len as usize);
            str::from_utf8(query).or(Err(ErrorCode::InvalidUtf8))?
        } else {
            ""
        };

        let locals_query = if locals_query_len > 0 {
            let query = slice::from_raw_parts(locals_query.cast::<u8>(), locals_query_len as usize);
            str::from_utf8(query).or(Err(ErrorCode::InvalidUtf8))?
        } else {
            ""
        };

        let lang = CStr::from_ptr(language_name)
            .to_str()
            .or(Err(ErrorCode::InvalidLanguageName))?;

        let mut config = HighlightConfiguration::new(
            language,
            lang,
            highlight_query,
            injection_query,
            locals_query,
        )
        .or(Err(ErrorCode::InvalidQuery))?;
        config.configure(this.highlight_names.as_slice());
        this.languages.insert(scope_name, (injection_regex, config));

        Ok(())
    };

    match f() {
        Ok(()) => ErrorCode::Ok,
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn ts_highlight_buffer_new() -> *mut TSHighlightBuffer {
    Box::into_raw(Box::new(TSHighlightBuffer {
        highlighter: Highlighter::new(),
        renderer: HtmlRenderer::new(),
    }))
}

/// Deletes a [`TSHighlighter`] instance.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlighter`] instance
/// created by [`ts_highlighter_new`].
///
/// It cannot be used after this function is called.
#[no_mangle]
pub unsafe extern "C" fn ts_highlighter_delete(this: *mut TSHighlighter) {
    drop(Box::from_raw(this));
}

/// Deletes a [`TSHighlightBuffer`] instance.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlightBuffer`] instance
/// created by [`ts_highlight_buffer_new`]
///
/// It cannot be used after this function is called.
#[no_mangle]
pub unsafe extern "C" fn ts_highlight_buffer_delete(this: *mut TSHighlightBuffer) {
    drop(Box::from_raw(this));
}

/// Get the HTML content of a [`TSHighlightBuffer`] instance as a raw pointer.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlightBuffer`] instance
/// created by [`ts_highlight_buffer_new`].
///
/// The returned pointer, a C-style string, must not outlive the [`TSHighlightBuffer`] instance,
/// else the data will point to garbage.
///
/// To get the length of the HTML content, use [`ts_highlight_buffer_len`].
#[no_mangle]
pub unsafe extern "C" fn ts_highlight_buffer_content(this: *const TSHighlightBuffer) -> *const u8 {
    let this = unwrap_ptr(this);
    this.renderer.html.as_slice().as_ptr()
}

/// Get the line offsets of a [`TSHighlightBuffer`] instance as a C-style array.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlightBuffer`] instance
/// created by [`ts_highlight_buffer_new`].
///
/// The returned pointer, a C-style array of [`u32`]s, must not outlive the [`TSHighlightBuffer`]
/// instance, else the data will point to garbage.
///
/// To get the length of the array, use [`ts_highlight_buffer_line_count`].
#[no_mangle]
pub unsafe extern "C" fn ts_highlight_buffer_line_offsets(
    this: *const TSHighlightBuffer,
) -> *const u32 {
    let this = unwrap_ptr(this);
    this.renderer.line_offsets.as_slice().as_ptr()
}

/// Get the length of the HTML content of a [`TSHighlightBuffer`] instance.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlightBuffer`] instance
/// created by [`ts_highlight_buffer_new`].
#[no_mangle]
pub unsafe extern "C" fn ts_highlight_buffer_len(this: *const TSHighlightBuffer) -> u32 {
    let this = unwrap_ptr(this);
    this.renderer.html.len() as u32
}

/// Get the number of lines in a [`TSHighlightBuffer`] instance.
///
/// # Safety
///
/// `this` must be non-null and must be a valid pointer to a [`TSHighlightBuffer`] instance
/// created by [`ts_highlight_buffer_new`].
#[no_mangle]
pub unsafe extern "C" fn ts_highlight_buffer_line_count(this: *const TSHighlightBuffer) -> u32 {
    let this = unwrap_ptr(this);
    this.renderer.line_offsets.len() as u32
}

/// Highlight a string of source code.
///
/// # Safety
///
/// The caller must ensure that `scope_name`, `source_code`, `output`, and `cancellation_flag` are
/// valid for the lifetime of the [`TSHighlighter`] instance, and are non-null.
///
/// `this` must be a non-null pointer to a [`TSHighlighter`] instance created by
/// [`ts_highlighter_new`]
#[no_mangle]
pub unsafe extern "C" fn ts_highlighter_highlight(
    this: *const TSHighlighter,
    scope_name: *const c_char,
    source_code: *const c_char,
    source_code_len: u32,
    output: *mut TSHighlightBuffer,
    cancellation_flag: *const AtomicUsize,
) -> ErrorCode {
    let this = unwrap_ptr(this);
    let output = unwrap_mut_ptr(output);
    let scope_name = unwrap(CStr::from_ptr(scope_name).to_str());
    let source_code = slice::from_raw_parts(source_code.cast::<u8>(), source_code_len as usize);
    let cancellation_flag = cancellation_flag.as_ref();
    this.highlight(source_code, scope_name, output, cancellation_flag)
}

impl TSHighlighter {
    fn highlight(
        &self,
        source_code: &[u8],
        scope_name: &str,
        output: &mut TSHighlightBuffer,
        cancellation_flag: Option<&AtomicUsize>,
    ) -> ErrorCode {
        let entry = self.languages.get(scope_name);
        if entry.is_none() {
            return ErrorCode::UnknownScope;
        }
        let (_, configuration) = entry.unwrap();
        let languages = &self.languages;

        let highlights = output.highlighter.highlight(
            configuration,
            source_code,
            cancellation_flag,
            move |injection_string| {
                languages.values().find_map(|(injection_regex, config)| {
                    injection_regex.as_ref().and_then(|regex| {
                        if regex.is_match(injection_string) {
                            Some(config)
                        } else {
                            None
                        }
                    })
                })
            },
        );

        if let Ok(highlights) = highlights {
            output.renderer.reset();
            output
                .renderer
                .set_carriage_return_highlight(self.carriage_return_index.map(Highlight));
            let result = output.renderer.render(highlights, source_code, &|s, out| {
                out.extend(self.attribute_strings[s.0]);
            });
            match result {
                Err(Error::Cancelled | Error::Unknown) => ErrorCode::Timeout,
                Err(Error::InvalidLanguage) => ErrorCode::InvalidLanguage,
                Ok(()) => ErrorCode::Ok,
            }
        } else {
            ErrorCode::Timeout
        }
    }
}

unsafe fn unwrap_ptr<'a, T>(result: *const T) -> &'a T {
    result.as_ref().unwrap_or_else(|| {
        eprintln!("{}:{} - pointer must not be null", file!(), line!());
        abort();
    })
}

unsafe fn unwrap_mut_ptr<'a, T>(result: *mut T) -> &'a mut T {
    result.as_mut().unwrap_or_else(|| {
        eprintln!("{}:{} - pointer must not be null", file!(), line!());
        abort();
    })
}

fn unwrap<T, E: fmt::Display>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| {
        eprintln!("tree-sitter highlight error: {error}");
        abort();
    })
}
