#[cfg(test)]
use alloc::string::String;

use markup::Render;

use crate::CompactString;

#[cfg_attr(docsrs, doc(cfg(feature = "markup")))]
impl Render for CompactString {
    #[inline]
    fn render(&self, writer: &mut impl core::fmt::Write) -> core::fmt::Result {
        self.as_str().render(writer)
    }
}

#[cfg(test)]
#[test]
fn test_markup() {
    const TEXT: &str = "<script>alert('Hello, world!')</script>";

    markup::define!(Template<M: Render>(msg: M) {
        textarea { @msg }
    });

    let compact = Template {
        msg: CompactString::from(TEXT),
    };
    let control = Template {
        msg: String::from(TEXT),
    };
    assert_eq!(
        compact.to_string(),
        "<textarea>&lt;script&gt;alert('Hello, world!')&lt;/script&gt;</textarea>",
    );
    assert_eq!(
        control.to_string(),
        "<textarea>&lt;script&gt;alert('Hello, world!')&lt;/script&gt;</textarea>",
    );
}
