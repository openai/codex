use std::fmt;
use std::io;

pub trait AnyWrite {
    type Wstr: ?Sized;
    type Error;

    fn write_fmt(&mut self, fmt: fmt::Arguments) -> Result<(), Self::Error>;

    fn write_str(&mut self, s: &Self::Wstr) -> Result<(), Self::Error>;
}

impl<'a> AnyWrite for dyn fmt::Write + 'a {
    type Wstr = str;
    type Error = fmt::Error;

    fn write_fmt(&mut self, fmt: fmt::Arguments) -> Result<(), Self::Error> {
        fmt::Write::write_fmt(self, fmt)
    }

    fn write_str(&mut self, s: &Self::Wstr) -> Result<(), Self::Error> {
        fmt::Write::write_str(self, s)
    }
}

impl<'a> AnyWrite for dyn io::Write + 'a {
    type Wstr = [u8];
    type Error = io::Error;

    fn write_fmt(&mut self, fmt: fmt::Arguments) -> Result<(), Self::Error> {
        io::Write::write_fmt(self, fmt)
    }

    fn write_str(&mut self, s: &Self::Wstr) -> Result<(), Self::Error> {
        io::Write::write_all(self, s)
    }
}
