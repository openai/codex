//! Certain bits of environmental state are too annoying to thread
//! around everywhere, so pack them into TLS.

use crate::file_text::FileText;
use crate::session::Session;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Tls {
    _dummy: (),
}

#[derive(Clone)]
struct TlsFields {
    session: Rc<Session>,
    file_text: Rc<FileText>,
}

thread_local! {
    static THE_TLS_FIELDS: RefCell<Option<TlsFields>> =
        RefCell::new(None)
}

impl Tls {
    #[cfg(test)]
    pub fn test() -> Tls {
        Self::install(Rc::new(Session::test()), Rc::new(FileText::test()))
    }

    #[cfg(test)]
    pub fn test_string(text: &str) -> Tls {
        use std::path::PathBuf;
        Self::install(
            Rc::new(Session::test()),
            Rc::new(FileText::new(PathBuf::from("tmp.txt"), String::from(text))),
        )
    }

    /// Installs `Tls` and returns a placeholder value.  When this
    /// value is dropped, the `Tls` entries will be removed. To access
    /// the values from `Tls`, call `Tls::session()` or
    /// `Tls::file_text()`.
    pub fn install(session: Rc<Session>, file_text: Rc<FileText>) -> Tls {
        let fields = TlsFields { session, file_text };

        THE_TLS_FIELDS.with(|s| {
            let mut s = s.borrow_mut();
            assert!(s.is_none());
            *s = Some(fields.clone());
        });

        Tls { _dummy: () }
    }

    fn fields() -> TlsFields {
        THE_TLS_FIELDS.with(|s| s.borrow().clone().expect("TLS is not installed"))
    }

    pub fn session() -> Rc<Session> {
        Self::fields().session
    }

    pub fn file_text() -> Rc<FileText> {
        Self::fields().file_text
    }
}

impl Drop for Tls {
    fn drop(&mut self) {
        THE_TLS_FIELDS.with(|s| {
            *s.borrow_mut() = None;
        })
    }
}
