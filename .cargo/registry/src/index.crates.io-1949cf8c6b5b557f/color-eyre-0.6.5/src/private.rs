use crate::eyre::Report;
pub trait Sealed {}

impl<T, E> Sealed for std::result::Result<T, E> where E: Into<Report> {}
impl Sealed for Report {}
