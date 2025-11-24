use proc_macro2::TokenStream;
use syn::Ident;

use crate::error::Errors;
use crate::mir::Mir;
use crate::parser::definition::{bytes_to_regex_string, Literal};

#[derive(Default)]
pub struct Subpatterns {
    map: Vec<(Ident, String)>,
}

impl Subpatterns {
    pub fn add(&mut self, param: Ident, pattern: TokenStream, errors: &mut Errors) {
        let lit = match syn::parse2::<Literal>(pattern) {
            Ok(lit) => lit,
            Err(e) => {
                errors.err(e.to_string(), e.span());
                return;
            }
        };

        if let Some((name, _)) = self.map.iter().find(|(name, _)| *name == param) {
            errors
                .err(format!("{} can only be assigned once", param), param.span())
                .err("Previously assigned here", name.span());
            return;
        }

        let fixed = self.fix(&lit, errors);

        // Validate the literal as proper regex. If it's not, emit an error.
        let mir = match &lit {
            Literal::Utf8(_) => Mir::utf8(&fixed),
            Literal::Bytes(_) => Mir::binary(&fixed),
        };

        if let Err(err) = mir {
            errors.err(err, lit.span());
        };

        self.map.push((param, fixed));
    }

    pub fn fix(&self, lit: &Literal, errors: &mut Errors) -> String {
        let mut i = 0;
        let mut pattern = match lit {
            Literal::Utf8(s) => s.value(),
            Literal::Bytes(b) => bytes_to_regex_string(b.value()),
        };

        while let Some(f) = pattern[i..].find("(?&") {
            i += f;
            pattern.replace_range(i..i + 3, "(?:");
            i += 3;

            let subref_end = if let Some(f) = pattern[i..].find(')') {
                i + f
            } else {
                pattern.truncate(i); // truncate so latter error doesn't suppress
                break; // regex-syntax will report the unclosed group
            };

            let name = &pattern[i..subref_end];
            let name = match syn::parse_str::<Ident>(name) {
                Ok(name) => name,
                Err(_) => {
                    errors.err(
                        format!("subpattern reference `{}` is not an identifier", name),
                        lit.span(),
                    );
                    // we emitted the error; make something up and continue
                    pattern.replace_range(i..subref_end, "_");
                    i += 2;
                    continue;
                }
            };

            match self.map.iter().find(|(def, _)| *def == name) {
                Some((_, subpattern)) => {
                    pattern.replace_range(i..subref_end, &subpattern);
                    i += subpattern.len() + 1;
                }
                None => {
                    errors.err(
                        format!("subpattern reference `{}` has not been defined", name),
                        lit.span(),
                    );
                    // leaving `(?:name)` is fine
                    i = subref_end + 1;
                }
            }
        }

        pattern
    }
}
