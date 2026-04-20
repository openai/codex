---
name: code-review-context
description: Model visible context
---

Codex maintains a context (history of messages) that is sent to the model in inference requests.

1. No history rewrite - the context must be build up incrementally.  
2. Avoid frequent changes to context that cause cache misses. 
3. No unbounded items - everything injected in the model context must have a bounded size.
4. Hightlight new individual items that can cross >1k tokens as P0. These need and additional manual review.