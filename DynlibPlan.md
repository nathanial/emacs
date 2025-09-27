# Dynlib Migration Plan

## Objective
Lay out the steps required to replace Emacs’s `src/dynlib.c` implementation with
Rust code while preserving the existing module loading API and keeping macOS
Cocoa plus Linux GTK/PGTK builds stable throughout the transition.

## Scope
- Covers the runtime dynamic loader used by modules, native-comp, and
  Lisp-facing helpers (`dynlib_open`, `dynlib_sym`, `dynlib_close`, `dynlib_addr`,
  etc.).
- Excludes platform ports that were recently removed (Windows/MS-DOS/GNUstep).
- Assumes Autotools is still the active build while the CMake migration is in
  progress; the plan notes alignment points with both systems.

## Constraints & Risks
- **GC interaction:** Returned pointers feed into Lisp objects; any Rust bridge
  must ensure lifetime management matches GC expectations.
- **Native-comp reliance:** ELN loading depends on `dynlib_open_for_eln` and
  subtle error reporting; regressions here could break user configurations.
- **Platform variance:** macOS needs `NSModule` semantics; Linux relies on
  `RTLD_GLOBAL/LAZY`. We must verify feature parity on both.
- **Error propagation:** Existing callers expect `dynlib_error` to return a
  thread-local string. The Rust rewrite must provide identical behaviour.
- **Testing gap:** There are no direct unit tests for dynlib; we rely on module
  smoke tests and native-comp exercises. Extra coverage will be required.

## Dependencies
- Rust toolchain ≥ 1.80 (already standardised for Phase A CLI work).
- `libloading` crate for cross-platform `dlopen` access.
- `cbindgen` to generate C headers consumed by the C side.
- Availability of representative native modules or native-comp artefacts to
  smoke-test the loader.

## Phase Breakdown

### Phase 0 – Discovery (2 days)
1. Catalogue every call site of `dynlib_*` functions (`rg "dynlib_" src lisp`).
2. Document platform-specific branches (e.g., `#ifdef WINDOWSNT` sections) and
   confirm they are dead after recent cleanup.
3. Capture current error-string semantics and `dynlib_addr` behaviour for
   both macOS and Linux.
4. Identify existing native-comp / module tests to reuse; note missing ones.

**Deliverables:** Discovery notes linked from `cmake/Phase0Discovery.md`, updated
inventory in `DynlibPlan.md` if new risks arise.

### Phase 1 – FFI scaffolding (3–4 days)
1. Create a `rust/libemacs` crate exporting a minimal dynlib interface.
2. Generate headers via `cbindgen`, commit them under `src/` (temporary path)
   and guard inclusion behind `HAVE_RUST_DYNLIB`.
3. Add Autotools glue: `configure.ac` option `--with-rust-dynlib[=no/yes]`
   defaulting to `no`; update `lib-src/Makefile.in` to build the Rust crate when
   enabled.
4. Introduce stub Rust implementations that just call back into the existing C
   functions (`extern "C"`), validating the build pipeline without behaviour
   changes.

**Exit criteria:** `./configure --with-ns --with-modules --with-rust-dynlib` and
`make -j` succeed, and `dynlib` symbols link correctly from Rust stubs.

### Phase 2 – Rust implementation (5–6 days)
1. Replace the Rust stubs with `libloading`-based code for macOS & Linux.
2. Mirror current semantics:
   - Default handle (`NULL` path) loads the main executable.
   - `dynlib_error` stores the last OS error string.
   - `dynlib_addr` fills `file`/`sym` consistently with `dladdr`.
3. Provide safe wrappers for ingestion into C: `dynlib_open` returns `void *`
   backed by `Box<DynlibHandle>`; `dynlib_close` drops the box; symbol lookups
   preserve const correctness.
4. Wire native-comp special cases (`dynlib_open_for_eln`) and ensure module
   allowlists still work.
5. Keep the legacy C implementation gated by `#ifndef HAVE_RUST_DYNLIB` for
   fallback builds.

**Exit criteria:** Unit tests in Rust compile; `make -C test manual-modules` (or
custom script) succeeds on macOS & Linux with the Rust backend enabled.

### Phase 3 – Testing & harden (4 days)
1. Add ERT smoke tests invoking `dynlib-open`/`dynlib-error` via a lightweight
   test module built during `make check` (consider reusing `test/manual/modules`).
2. Run `make check` on macOS Cocoa and Linux GTK/PGTK with
   `--with-rust-dynlib=yes`.
3. Validate native compilation of a sample Elisp file (`native-compile`), load
   the resulting `.eln`, and ensure `dynlib_addr` still resolves doc strings.
4. Measure performance (dlopen latency) to ensure no regressions; capture in
   plan notes.
5. Document configuration knobs in `INSTALL.REPO`, `NEWS`, and update
   `LegacyCleanupPlan.md` Phase entries with the new status.

**Exit criteria:** All targeted tests pass, documentation merged, release notes
prepared. `--with-rust-dynlib` becomes the default in nightly builds once both
platforms are stable.

### Phase 4 – Cleanup & Default Flip (2 days)
1. Remove the legacy C implementation, leaving only the Rust variant.
2. Flip configure default to `yes`; drop `--with-rust-dynlib` option after one
   release cycle if no issues arise.
3. Trim CI scripts and bootstrap instructions to require Rust for developers.
4. Open follow-up issues for Windows support if the platform is ever re-added
   (document assumptions for future maintainers).

## Testing Matrix
| Platform | Build | Tests | Notes |
| --- | --- | --- | --- |
| macOS 14 (arm64) | `--with-ns --with-modules --with-rust-dynlib` | `make -j`, `make check`, native-comp smoke | Ensure notarised modules still load. |
| Ubuntu 24.04 (x86_64) | `--with-pgtk --with-modules --with-rust-dynlib` | `make -j`, `make check`, module load harness | Watch for `LD_LIBRARY_PATH` differences. |
| macOS CI | Same as above | `cargo test -p libemacs` | Tie into existing CI once CMake migration proceeds. |

## Rollback Strategy
- Keep the C implementation behind a configure flag until Phase 4 completes.
- During Phases 2–3, nightly builds should run both backends to catch drift.
- If regressions surface, flip `--with-rust-dynlib` off and reopen plan without
  destabilising releases.

## Open Questions
1. Should we expose additional diagnostics (e.g., stack traces) from the Rust
   layer, or remain bit-for-bit compatible with existing errors?
2. Can we share the Rust loader between Emacs proper and an eventual CMake
   build without duplicating build logic?
3. Do we need a public API for Lisp to query the loader backend (useful for
   debugging)?

---
Last updated: 2025-09-27.
