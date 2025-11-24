use std::path::{Path, PathBuf};

use ts_rs::{Dependency, ExportError, TypeVisitor, TS};

#[rustfmt::skip]
trait Malicious {
    type WithoutGenerics: TS + ?Sized;
    const DOCS: Option<&'static str> = None;
    
    fn ident() -> String { unimplemented!() }
    fn decl() -> String { unimplemented!() }
    fn decl_concrete() -> String { unimplemented!() }
    fn name() -> String { unimplemented!() }
    fn inline() -> String { unimplemented!() }
    fn inline_flattened() -> String { unimplemented!() }
    fn dependencies() -> Vec<Dependency> { unimplemented!() }
    fn visit_dependencies(_: &mut impl TypeVisitor) { unimplemented!() }
    fn visit_generics(_: &mut impl TypeVisitor) { unimplemented!() }
    fn export() -> Result<(), ExportError> { unimplemented!() }
    fn export_all() -> Result<(), ExportError> { unimplemented!() }
    fn export_all_to(out_dir: impl AsRef<Path>) -> Result<(), ExportError> { unimplemented!() }
    fn export_to_string() -> Result<String, ExportError> { unimplemented!() }
    fn output_path() -> Option<PathBuf> { unimplemented!() }
    fn default_output_path() -> Option<PathBuf> { unimplemented!() }
}

impl<T> Malicious for T {
    type WithoutGenerics = ();
}

#[derive(TS)]
#[ts(export, export_to = "issue_308/")]
struct MyStruct<A, B>(A, B);
