/*
 * Copyright 2019 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use starlark_syntax::codemap::CodeMap;
use starlark_syntax::codemap::Pos;
use starlark_syntax::codemap::Span;

use crate::analysis::unused_loads::find::find_unused_loads;

struct Out<'a> {
    codemap: &'a CodeMap,
    out: String,
    pos: Pos,
}

impl<'a> Out<'a> {
    fn append_to(&mut self, pos: Pos) {
        assert!(self.pos <= pos);
        assert!(pos <= self.codemap.full_span().end());
        self.out
            .push_str(self.codemap.source_span(Span::new(self.pos, pos)));
        self.pos = pos;
    }

    fn skip_to(&mut self, pos: Pos) {
        assert!(self.pos <= pos);
        assert!(pos <= self.codemap.full_span().end());
        self.pos = pos;
    }

    /// Append to the beginning of the span, and set the position to the end of the span.
    fn skip_span(&mut self, span: Span) {
        self.append_to(span.begin());
        self.skip_to(span.end());
    }
}

/// Return `None` if there is no unused loads.
pub fn remove_unused_loads(name: &str, program: &str) -> crate::Result<Option<String>> {
    let (codemap, unused_loads) = find_unused_loads(name, program)?;
    if unused_loads.is_empty() {
        return Ok(None);
    }

    let mut out = Out {
        codemap: &codemap,
        out: String::new(),
        pos: Pos::new(0),
    };

    for load in unused_loads {
        if load.all_unused() {
            out.skip_span(load.load.span);
        } else {
            for arg in load.unused_args {
                out.skip_span(arg.span_with_trailing_comma());
            }
        }
    }

    out.append_to(codemap.full_span().end());

    Ok(Some(out.out))
}
