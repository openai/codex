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

use std::ptr;

use dupe::Dupe;

use crate::errors::Frame;
use crate::eval::runtime::frame_span::FrameSpan;
use crate::values::FrozenHeap;
use crate::values::FrozenRef;
use crate::values::FrozenValue;

/// When a function `a` is inlined into `b`, this struct contains
/// the inlined frame for expressions in `a` which now reside in `b`.
#[derive(Debug, PartialEq)]
pub(crate) struct InlinedFrame {
    pub(crate) span: FrameSpan,
    pub(crate) fun: FrozenValue,
}

impl InlinedFrame {
    /// Recursively collect frames.
    ///
    /// Resulting frames are ordered bottom-to-top, same order as in `CallStack`.
    pub(crate) fn extend_frames(&self, frames: &mut Vec<Frame>) {
        frames.push(Frame {
            name: self.fun.to_value().name_for_call_stack(),
            location: Some(self.span.span.to_file_span()),
        });
        self.span.inlined_frames.extend_frames(frames);
    }
}

/// Stack of inlined frames (maybe empty).
#[derive(Copy, Clone, Dupe, Debug, Default)]
pub(crate) struct InlinedFrames {
    /// Linked list.
    pub(crate) frames: Option<FrozenRef<'static, InlinedFrame>>,
}

impl PartialEq for InlinedFrames {
    fn eq(&self, other: &Self) -> bool {
        match (self.frames, other.frames) {
            (Some(a), Some(b)) => ptr::eq(a.as_ref(), b.as_ref()),
            (None, None) => true,
            (Some(_), None) | (None, Some(_)) => false,
        }
    }
}

impl Eq for InlinedFrames {}

impl InlinedFrames {
    /// Collect frames, bottom-to-top, same order as in `CallStack`.
    pub(crate) fn extend_frames(self, frames: &mut Vec<Frame>) {
        if let Some(f) = self.frames {
            f.extend_frames(frames);
        }
    }

    fn to_inlined_frames(self) -> Vec<FrozenRef<'static, InlinedFrame>> {
        let mut r = Vec::new();
        let mut frames_iter = self;
        while let Some(frames) = frames_iter.frames {
            r.push(frames);
            frames_iter = frames.span.inlined_frames;
        }
        r
    }

    /// Inline this stack into given span.
    ///
    /// E. g. when inlining `def a(): return {}` into `def b(): a()`,
    /// self is empty stack for expression `{}`, `span` is `a()` and `fun` is `a`.
    pub(crate) fn inline_into(
        &mut self,
        span: FrameSpan,
        fun: FrozenValue,
        span_alloc: &mut InlinedFrameAlloc,
    ) {
        self.frames = Some(span_alloc.alloc_frame(InlinedFrame {
            span: FrameSpan {
                span: span.span,
                inlined_frames: *self,
            },
            fun,
        }));
        for f in span.inlined_frames.to_inlined_frames().into_iter().rev() {
            self.frames = Some(span_alloc.alloc_frame(InlinedFrame {
                span: FrameSpan {
                    span: f.span.span,
                    inlined_frames: *self,
                },
                fun: f.fun,
            }));
        }
    }
}

/// Heap allocator for `InlinedFrame` which attempts to reuse previous allocation.
pub(crate) struct InlinedFrameAlloc<'f> {
    frozen_heap: &'f FrozenHeap,
    last_alloc: Option<FrozenRef<'static, InlinedFrame>>,
}

impl<'f> InlinedFrameAlloc<'f> {
    pub(crate) fn new(frozen_heap: &'f FrozenHeap) -> Self {
        Self {
            frozen_heap,
            last_alloc: None,
        }
    }

    pub(crate) fn alloc_frame(&mut self, frame: InlinedFrame) -> FrozenRef<'static, InlinedFrame> {
        if let Some(last_alloc) = self.last_alloc {
            if *last_alloc == frame {
                return last_alloc;
            }
        }
        let frame = self.frozen_heap.alloc_any(frame);
        self.last_alloc = Some(frame);
        frame
    }
}

#[cfg(test)]
mod tests {
    use starlark_syntax::slice_vec_ext::SliceExt;

    use crate::codemap::CodeMap;
    use crate::eval::runtime::frame_span::FrameSpan;
    use crate::eval::runtime::frozen_file_span::FrozenFileSpan;
    use crate::eval::runtime::inlined_frame::InlinedFrameAlloc;
    use crate::eval::runtime::inlined_frame::InlinedFrames;
    use crate::values::FrozenHeap;

    #[test]
    fn test_inline_into() {
        // Test frame inlining with this code:
        //
        // ```
        // def a(): return {}
        // def b(): return a()
        // def c(): return b()
        //
        // def d(): return c()
        // def e(): return d()
        // def f(): return e()
        // ```
        //
        // If `a` is inlined into `b` and then inlined into `c`,
        // then if `d` is inlined into `e` and then inlined into `f`,
        // and then `c` is inlined into `d` (which is already inlined into `f`),
        // the resulting stack trace should be `f`, `e`, `d`, `c`, `b`, `a`.

        let frozen_heap = FrozenHeap::new();

        fn make_span(heap: &FrozenHeap, text: &str) -> FrameSpan {
            let codemap = CodeMap::new(format!("{}.bzl", text), text.to_owned());
            let codemap = heap.alloc_any(codemap);
            FrameSpan {
                span: FrozenFileSpan::new(codemap, codemap.full_span()),
                inlined_frames: InlinedFrames::default(),
            }
        }

        let mut span_alloc = InlinedFrameAlloc::new(&frozen_heap);

        fn assert_stack(expected: &[&str], span: &FrameSpan) {
            let mut frames = Vec::new();
            span.inlined_frames.extend_frames(&mut frames);
            let frames = frames.map(|f| {
                let span = f.location.as_ref().unwrap().source_span();
                let f = f.name.trim_matches('"');
                format!("{} in {}", span, f)
            });
            assert_eq!(expected, &frames);
        }

        let mut a = make_span(&frozen_heap, "{}");
        let b = make_span(&frozen_heap, "a()");
        let c = make_span(&frozen_heap, "b()");
        a.inlined_frames.inline_into(
            b,
            frozen_heap.alloc_str("b").to_frozen_value(),
            &mut span_alloc,
        );
        a.inlined_frames.inline_into(
            c,
            frozen_heap.alloc_str("c").to_frozen_value(),
            &mut span_alloc,
        );

        assert_stack(&["b() in c", "a() in b"], &a);

        let mut d = make_span(&frozen_heap, "c()");
        let e = make_span(&frozen_heap, "d()");
        let f = make_span(&frozen_heap, "e()");

        d.inlined_frames.inline_into(
            e,
            frozen_heap.alloc_str("e").to_frozen_value(),
            &mut span_alloc,
        );
        d.inlined_frames.inline_into(
            f,
            frozen_heap.alloc_str("f").to_frozen_value(),
            &mut span_alloc,
        );

        assert_stack(&["e() in f", "d() in e"], &d);

        a.inlined_frames.inline_into(
            d,
            frozen_heap.alloc_str("d").to_frozen_value(),
            &mut span_alloc,
        );

        assert_stack(
            &["e() in f", "d() in e", "c() in d", "b() in c", "a() in b"],
            &a,
        );
    }
}
