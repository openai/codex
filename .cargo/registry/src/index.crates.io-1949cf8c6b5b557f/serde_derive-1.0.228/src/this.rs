use crate::internals::ast::Container;
use syn::{Path, PathArguments, Token};

pub fn this_type(cont: &Container) -> Path {
    if let Some(remote) = cont.attrs.remote() {
        let mut this = remote.clone();
        for segment in &mut this.segments {
            if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                arguments.colon2_token = None;
            }
        }
        this
    } else {
        Path::from(cont.ident.clone())
    }
}

pub fn this_value(cont: &Container) -> Path {
    if let Some(remote) = cont.attrs.remote() {
        let mut this = remote.clone();
        for segment in &mut this.segments {
            if let PathArguments::AngleBracketed(arguments) = &mut segment.arguments {
                if arguments.colon2_token.is_none() {
                    arguments.colon2_token = Some(Token![::](arguments.lt_token.span));
                }
            }
        }
        this
    } else {
        Path::from(cont.ident.clone())
    }
}
