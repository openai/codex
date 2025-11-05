# Research Plan: Rustのプロセス分離 (2023-2025)

## 1. Scope
- **Goal**: Compare approaches and evolution of process isolation techniques in Rust between 2023 and 2025, covering OS-level sandboxing, capability-based runtimes, Wasm/container isolation, and emerging libraries.
- **Evaluation Axes**: Security guarantees, performance overhead, portability (Linux/macOS/Windows/WebAssembly), developer ergonomics, and maturity (production adoption, maintenance).
- **Success Criteria**:
  - Identify at least three distinct Rust isolation solutions with publication or update dates between 2023-2025.
  - Provide comparative analysis of strengths/limitations with multiple domain citations.
  - Highlight changes vs prior state (new features, adoption stories, security incidents).
- **Constraints**: Must cite multiple independent sources; respect security/policy guidelines; capture logs and contradictions in `/artifacts`.
- **Out-of-Scope**: Non-Rust isolation frameworks unless used to compare Rust-specific capabilities.

## 2. Sub-Queries
1. OS sandboxing crates (e.g., `seatbelt`, `seccomp`, `landlock`, `sandcastle`) updates 2023-2025.
2. Rust-based container/runtime isolation (e.g., `youki` OCI runtime, `Kata Containers` Rust components).
3. WebAssembly/WASI-based sandboxing in Rust (e.g., `wasmtime`, `wasmer`, `lunatic`).
4. Security research papers/blogs on Rust isolation architectures, including vulnerabilities or benchmarks.
5. Tooling integration (Cargo, CI pipelines) and developer adoption case studies.

## 3. Refutation Targets
- Reports of privilege escalation or sandbox escapes in Rust isolators during 2023-2025.
- Critiques about performance or maintainability compared to other languages.
- Claims that Rust cannot provide effective isolation vs alternatives.

## 4. Stop Conditions
- Three or more domains provide sufficient detail for each isolation approach.
- No new significant findings after breadth 8/depth 3 exploration.
- Budget/time limits approached or satisfied.

