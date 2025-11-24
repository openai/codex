// This file contains generated code. Do not edit directly.
// To regenerate this, run 'make'.

//! Bindings to the `XPrint` X11 extension.

#![allow(clippy::too_many_arguments)]

#[allow(unused_imports)]
use std::borrow::Cow;
#[allow(unused_imports)]
use std::convert::TryInto;
#[allow(unused_imports)]
use crate::utils::RawFdContainer;
#[allow(unused_imports)]
use crate::x11_utils::{Request, RequestHeader, Serialize, TryParse, TryParseFd};
use std::io::IoSlice;
use crate::connection::RequestConnection;
#[allow(unused_imports)]
use crate::connection::Connection as X11Connection;
#[allow(unused_imports)]
use crate::cookie::{Cookie, CookieWithFds, VoidCookie};
use crate::errors::ConnectionError;
#[allow(unused_imports)]
use crate::errors::ReplyOrIdError;
#[allow(unused_imports)]
use super::xproto;

pub use x11rb_protocol::protocol::xprint::*;

/// Get the major opcode of this extension
fn major_opcode<Conn: RequestConnection + ?Sized>(conn: &Conn) -> Result<u8, ConnectionError> {
    let info = conn.extension_information(X11_EXTENSION_NAME)?;
    let info = info.ok_or(ConnectionError::UnsupportedExtension)?;
    Ok(info.major_opcode)
}

pub fn print_query_version<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, PrintQueryVersionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintQueryVersionRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_get_printer_list<'c, 'input, Conn>(conn: &'c Conn, printer_name: &'input [String8], locale: &'input [String8]) -> Result<Cookie<'c, Conn, PrintGetPrinterListReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetPrinterListRequest {
        printer_name: Cow::Borrowed(printer_name),
        locale: Cow::Borrowed(locale),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_rehash_printer_list<Conn>(conn: &Conn) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintRehashPrinterListRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn create_context<'c, 'input, Conn>(conn: &'c Conn, context_id: u32, printer_name: &'input [String8], locale: &'input [String8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = CreateContextRequest {
        context_id,
        printer_name: Cow::Borrowed(printer_name),
        locale: Cow::Borrowed(locale),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_set_context<Conn>(conn: &Conn, context: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintSetContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_get_context<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, PrintGetContextReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetContextRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_destroy_context<Conn>(conn: &Conn, context: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintDestroyContextRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_get_screen_of_context<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, PrintGetScreenOfContextReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetScreenOfContextRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_start_job<Conn>(conn: &Conn, output_mode: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintStartJobRequest {
        output_mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_end_job<Conn>(conn: &Conn, cancel: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintEndJobRequest {
        cancel,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_start_doc<Conn>(conn: &Conn, driver_mode: u8) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintStartDocRequest {
        driver_mode,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_end_doc<Conn>(conn: &Conn, cancel: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintEndDocRequest {
        cancel,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_put_document_data<'c, 'input, Conn>(conn: &'c Conn, drawable: xproto::Drawable, data: &'input [u8], doc_format: &'input [String8], options: &'input [String8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintPutDocumentDataRequest {
        drawable,
        data: Cow::Borrowed(data),
        doc_format: Cow::Borrowed(doc_format),
        options: Cow::Borrowed(options),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2]), IoSlice::new(&bytes[3]), IoSlice::new(&bytes[4]), IoSlice::new(&bytes[5]), IoSlice::new(&bytes[6])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_get_document_data<Conn>(conn: &Conn, context: Pcontext, max_bytes: u32) -> Result<Cookie<'_, Conn, PrintGetDocumentDataReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetDocumentDataRequest {
        context,
        max_bytes,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_start_page<Conn>(conn: &Conn, window: xproto::Window) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintStartPageRequest {
        window,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_end_page<Conn>(conn: &Conn, cancel: bool) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintEndPageRequest {
        cancel,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_select_input<Conn>(conn: &Conn, context: Pcontext, event_mask: u32) -> Result<VoidCookie<'_, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintSelectInputRequest {
        context,
        event_mask,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_input_selected<Conn>(conn: &Conn, context: Pcontext) -> Result<Cookie<'_, Conn, PrintInputSelectedReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintInputSelectedRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_get_attributes<Conn>(conn: &Conn, context: Pcontext, pool: u8) -> Result<Cookie<'_, Conn, PrintGetAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetAttributesRequest {
        context,
        pool,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_get_one_attributes<'c, 'input, Conn>(conn: &'c Conn, context: Pcontext, pool: u8, name: &'input [String8]) -> Result<Cookie<'c, Conn, PrintGetOneAttributesReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetOneAttributesRequest {
        context,
        pool,
        name: Cow::Borrowed(name),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_set_attributes<'c, 'input, Conn>(conn: &'c Conn, context: Pcontext, string_len: u32, pool: u8, rule: u8, attributes: &'input [String8]) -> Result<VoidCookie<'c, Conn>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintSetAttributesRequest {
        context,
        string_len,
        pool,
        rule,
        attributes: Cow::Borrowed(attributes),
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0]), IoSlice::new(&bytes[1]), IoSlice::new(&bytes[2])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_without_reply(&slices, fds)
}

pub fn print_get_page_dimensions<Conn>(conn: &Conn, context: Pcontext) -> Result<Cookie<'_, Conn, PrintGetPageDimensionsReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetPageDimensionsRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_query_screens<Conn>(conn: &Conn) -> Result<Cookie<'_, Conn, PrintQueryScreensReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintQueryScreensRequest;
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_set_image_resolution<Conn>(conn: &Conn, context: Pcontext, image_resolution: u16) -> Result<Cookie<'_, Conn, PrintSetImageResolutionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintSetImageResolutionRequest {
        context,
        image_resolution,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

pub fn print_get_image_resolution<Conn>(conn: &Conn, context: Pcontext) -> Result<Cookie<'_, Conn, PrintGetImageResolutionReply>, ConnectionError>
where
    Conn: RequestConnection + ?Sized,
{
    let request0 = PrintGetImageResolutionRequest {
        context,
    };
    let (bytes, fds) = request0.serialize(major_opcode(conn)?);
    let slices = [IoSlice::new(&bytes[0])];
    assert_eq!(slices.len(), bytes.len());
    conn.send_request_with_reply(&slices, fds)
}

/// Extension trait defining the requests of this extension.
pub trait ConnectionExt: RequestConnection {
    fn xprint_print_query_version(&self) -> Result<Cookie<'_, Self, PrintQueryVersionReply>, ConnectionError>
    {
        print_query_version(self)
    }
    fn xprint_print_get_printer_list<'c, 'input>(&'c self, printer_name: &'input [String8], locale: &'input [String8]) -> Result<Cookie<'c, Self, PrintGetPrinterListReply>, ConnectionError>
    {
        print_get_printer_list(self, printer_name, locale)
    }
    fn xprint_print_rehash_printer_list(&self) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_rehash_printer_list(self)
    }
    fn xprint_create_context<'c, 'input>(&'c self, context_id: u32, printer_name: &'input [String8], locale: &'input [String8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        create_context(self, context_id, printer_name, locale)
    }
    fn xprint_print_set_context(&self, context: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_set_context(self, context)
    }
    fn xprint_print_get_context(&self) -> Result<Cookie<'_, Self, PrintGetContextReply>, ConnectionError>
    {
        print_get_context(self)
    }
    fn xprint_print_destroy_context(&self, context: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_destroy_context(self, context)
    }
    fn xprint_print_get_screen_of_context(&self) -> Result<Cookie<'_, Self, PrintGetScreenOfContextReply>, ConnectionError>
    {
        print_get_screen_of_context(self)
    }
    fn xprint_print_start_job(&self, output_mode: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_start_job(self, output_mode)
    }
    fn xprint_print_end_job(&self, cancel: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_end_job(self, cancel)
    }
    fn xprint_print_start_doc(&self, driver_mode: u8) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_start_doc(self, driver_mode)
    }
    fn xprint_print_end_doc(&self, cancel: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_end_doc(self, cancel)
    }
    fn xprint_print_put_document_data<'c, 'input>(&'c self, drawable: xproto::Drawable, data: &'input [u8], doc_format: &'input [String8], options: &'input [String8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        print_put_document_data(self, drawable, data, doc_format, options)
    }
    fn xprint_print_get_document_data(&self, context: Pcontext, max_bytes: u32) -> Result<Cookie<'_, Self, PrintGetDocumentDataReply>, ConnectionError>
    {
        print_get_document_data(self, context, max_bytes)
    }
    fn xprint_print_start_page(&self, window: xproto::Window) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_start_page(self, window)
    }
    fn xprint_print_end_page(&self, cancel: bool) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_end_page(self, cancel)
    }
    fn xprint_print_select_input(&self, context: Pcontext, event_mask: u32) -> Result<VoidCookie<'_, Self>, ConnectionError>
    {
        print_select_input(self, context, event_mask)
    }
    fn xprint_print_input_selected(&self, context: Pcontext) -> Result<Cookie<'_, Self, PrintInputSelectedReply>, ConnectionError>
    {
        print_input_selected(self, context)
    }
    fn xprint_print_get_attributes(&self, context: Pcontext, pool: u8) -> Result<Cookie<'_, Self, PrintGetAttributesReply>, ConnectionError>
    {
        print_get_attributes(self, context, pool)
    }
    fn xprint_print_get_one_attributes<'c, 'input>(&'c self, context: Pcontext, pool: u8, name: &'input [String8]) -> Result<Cookie<'c, Self, PrintGetOneAttributesReply>, ConnectionError>
    {
        print_get_one_attributes(self, context, pool, name)
    }
    fn xprint_print_set_attributes<'c, 'input>(&'c self, context: Pcontext, string_len: u32, pool: u8, rule: u8, attributes: &'input [String8]) -> Result<VoidCookie<'c, Self>, ConnectionError>
    {
        print_set_attributes(self, context, string_len, pool, rule, attributes)
    }
    fn xprint_print_get_page_dimensions(&self, context: Pcontext) -> Result<Cookie<'_, Self, PrintGetPageDimensionsReply>, ConnectionError>
    {
        print_get_page_dimensions(self, context)
    }
    fn xprint_print_query_screens(&self) -> Result<Cookie<'_, Self, PrintQueryScreensReply>, ConnectionError>
    {
        print_query_screens(self)
    }
    fn xprint_print_set_image_resolution(&self, context: Pcontext, image_resolution: u16) -> Result<Cookie<'_, Self, PrintSetImageResolutionReply>, ConnectionError>
    {
        print_set_image_resolution(self, context, image_resolution)
    }
    fn xprint_print_get_image_resolution(&self, context: Pcontext) -> Result<Cookie<'_, Self, PrintGetImageResolutionReply>, ConnectionError>
    {
        print_get_image_resolution(self, context)
    }
}

impl<C: RequestConnection + ?Sized> ConnectionExt for C {}
