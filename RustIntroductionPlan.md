# Rust Introduction Plan

## Purpose
Establish a pragmatic starting point for introducing Rust into the Emacs C runtime while preserving shipping quality on macOS Cocoa and Linux GTK/PGTK. The intent is to replace leaf modules first, validate integration workflows, and incrementally expand the Rust surface without destabilising the Lisp engine or redisplay core.

**Status:** Phase A (tooling beachhead) completed 2025-09-27; Cargo workspace and first wave of replacements are in tree.

## Selection Criteria for Early Conversions
- **Low fan-in/out**: Modules with narrow, well-defined entry points and limited callers reduce FFI surface and review scope.
- **Portable logic**: Preference for code paths that already support only POSIX/Mach semantics (after recent platform pruning) and avoid heavy macro soup.
- **Existing regression tests**: ERT or unit tests must cover the behaviour so the Rust rewrite can prove parity quickly; otherwise add coverage before starting.
- **Minimal build entanglement**: Targets that can be built as separate static/SHARED libraries without touching the bootstrap (`temacs`) flow ease adoption of Cargo-driven builds.

## Quick-Win Candidates
| Area | Path | ~LOC | Primary Role | Key Dependencies | Existing Tests | Rust Notes |
| --- | --- | ---: | --- | --- | --- | --- |
| lib-src tool (replaced) | `rust/hexl` | 200 | Hexl mode converter CLI | Rust stdlib | Manual usage only | Completed in Phase A; serves as reference for future CLI ports. |
| lib-src tool (replaced) | `rust/make-fingerprint` | 170 | Compute/patch temacs fingerprint | `sha2`, Rust stdlib | `test/manual/temacs-fingerprint` | Completed in Phase A; demonstrates build-time integration with generated headers. |
| lib-src tool (replaced) | `rust/update-game-score` | 330 | High-score file updater | `libc`, `tempfile` | `test/lisp/playgame-tests.el` | Completed in Phase A; validates privileged locking semantics in Rust. |
| Runtime library | `src/dynlib.c` | 340 | Module loader abstraction | POSIX `dlopen`, Cocoa `NSModule` | `test/lisp/emacs-module-tests.el` | Rust’s `libloading` crate can replace hand-rolled loaders; keep exported C ABI via `extern "C"`. |
| Runtime library | `src/sqlite.c` | 928 | Built-in SQLite veneer | libsqlite3, coding system hooks | `test/src/sqlite-tests.el`, `test/lisp/sqlite-tests.el` | Map to `rusqlite` while reusing existing Lisp-facing API; validates mixed Rust/C unit testing. |
| Data structure | `src/itree.c` + `src/itree.h` | 1,426 | Interval tree for overlays | `lisp.h` (Lisp_Object interop) | `test/manual/noverlay/itree-tests.c` (manual) | Rewrite as pure Rust crate exporting C iterators; add automated tests before porting. |
| Data structure | `src/json.c` | 1,877 | JSON parse/encode | buffer, coding, Lisp_Object | `test/lisp/json-tests.el` | Replace with `serde_json`-backed implementation while keeping configurable object/array modes. |
| Cache utility | `src/region-cache.c` | 781 | Buffer region memoization | buffer gap ops | Lacks automated tests | Low hanging after tests exist; rewrite benefits from Rust slices and ownership to guard cache invariants. |

LOC counts obtained via `wc -l` (2025-09-27). Tests marked “manual” require automation before any port.

## Phased Introduction
1. **Phase A – Tooling beachhead**
   - Create `rust/` workspace with Cargo.toml and top-level `CMakeLists.txt` hook once the ongoing CMake migration begins.
   - Build lib-src replacements (`hexl`, `update-game-score`, `make-fingerprint`) as standalone Cargo binaries, keeping existing CLI flags.
   - Extend CI to run `cargo fmt`/`cargo clippy` and build the new binaries; wire Autotools targets to call Cargo-produced executables during bootstrap.

2. **Phase B – Leaf runtime libraries**
   - Introduce a `rust/libemacs` crate exporting C symbols via `cbindgen`.
   - Re-implement `dynlib` in Rust using `libloading`, exposing the same `dynlib_open/close/symbol` API and replacing the legacy C implementation once tests stay green.
   - Port `sqlite.c` by wrapping `rusqlite` and bridging to Lisp via helper FFI shims; reuse existing ERT suites to compare behaviour.

3. **Phase C – Data structure rewrites**
   - Augment coverage for `itree` and `region-cache` (new ERT tests exercising overlay operations).
   - Replace the C implementations with Rust equivalents, keeping FFI boundaries narrow (e.g., expose iterators that emit plain structs the C core already expects).
   - Evaluate JSON rewrite: prototype `serde_json` integration behind a `json-use-rust` Lisp variable; run `test/lisp/json-tests.el` under both paths before flipping the default.

4. **Phase D – Consolidation**
   - Once multiple modules live in Rust, factor shared helpers (allocation hooks, error handling) into a core crate.
   - Document coding standards, error conventions, and interop patterns in `CONTRIBUTING` so new contributors follow the same model.

## Methodical Workflow Checklist
- **Before each port**: confirm automated tests exist and run in CI; add missing coverage first.
- **Build integration**: use `cbindgen` to generate headers consumed by the remaining C code; ensure compatibility with both Autotools and the forthcoming CMake build.
- **Testing**: extend `make check` to invoke any Rust `cargo test` suites; add cross-platform smoke tests for the new binaries.
- **Performance gates**: capture baseline benchmarks (e.g., JSON encoding throughput, overlay operations) and compare after Rust rewrites.
- **Fallback plan**: keep original C implementations behind compilation flags until the Rust version survives at least one release cycle.

## Immediate Next Steps
1. Stand up a minimal Cargo workspace (`rust/README.md`, `.cargo/config.toml`) and document the toolchain version policy in `CONTRIBUTING`.
2. Draft automated ERT coverage for `region-cache` and non-manual `itree` scenarios to de-risk later ports.
3. Prepare RFC describing the Phase A CLI rewrites and collect feedback from maintainer list before landing code.
4. Prototype a Rust `hexl` binary out-of-tree to validate packaging, licensing headers, and cross-compilation story (Intel/ARM macOS, x86_64 Linux).
