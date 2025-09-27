# SQLite Rust Port Plan

## Objective
Provide a step-by-step roadmap for replacing the `src/sqlite.c` runtime with a
Rust implementation while preserving the existing Lisp-facing API and keeping
the macOS Cocoa and Linux GTK/PGTK builds stable. This plan fulfils the Phase B
commitment in `RustIntroductionPlan.md` to migrate leaf runtime libraries after
the dynlib rewrite.

## Scope
- Covers the built-in SQLite bindings exposed through functions such as
  `sqlite-open`, `sqlite-execute`, `sqlite-select`, `sqlite-more-p`,
  `sqlite-next`, `sqlite-finalize`, `sqlite-close`, `sqlite-version`, and the
  optional `sqlite-load-extension` entry points.
- Includes the `struct Lisp_Sqlite` lifetime management, finalizers, and
  encoding helpers currently implemented in `src/sqlite.c`.
- Integrates with existing ERT suites (`test/src/sqlite-tests.el`,
  `test/lisp/sqlite-tests.el`) and any C-level regression tests.
- Excludes higher-level Elisp wrapper work (`lisp/sqlite.el`) and the unrelated
  native JSON/itree rewrites tracked elsewhere.

## Constraints & Risks
- **GC contracts:** `Lisp_Sqlite` objects rely on Emacs GC finalizers to release
  connections and prepared statements. Rust code must honour the same
  finalization semantics to avoid resource leaks.
- **Encoding parity:** The current C implementation handles multibyte strings
  and text properties via `coding.c`. The Rust port must preserve UTF-8 and
  binary blob behaviour, including `coding-system` text properties.
- **Extension loading:** Optional support for `sqlite-load-extension` must
  remain disabled by default and respect the same gating rules. Rusqlite feature
  flags need to align with this policy.
- **Thread/locking model:** Emacs links SQLite in serialized or full-mutex mode.
  The Rust layer must reuse the same open flags and avoid introducing parallel
  access patterns that violate Emacs's single-thread expectations.
- **Build integration:** Autotools currently probes for `libsqlite3` and exposes
  `HAVE_SQLITE3`. The Rust build must continue to honour these checks and link
  against the detected library rather than vendoring a different copy.
- **Platform coverage:** Only macOS and Linux are in scope. Any Windows-era
  dynamic loading helpers can be deleted during the port, but the plan must call
  out the removal explicitly.

## Existing Behaviour Inventory
- **Connection lifecycle:** `sqlite-open`, `sqlite-close`, and in-memory database
  naming logic (`:memory:N` sequence) with read-only/open flags.
- **Statement management:** `sqlite-select` returning statement objects,
  `sqlite-more-p`, `sqlite-next`, and `sqlite-finalize` finalizers.
- **Exec helpers:** `sqlite-execute`, `sqlite-execute-batch`, `sqlite-changes`,
  and automatic binding of vectors or lists via `bind_values`.
- **Encoding support:** `encode_string_utf_8`, `binary` property handling,
  integer/float conversion, and blob support.
- **Error handling:** Raising `sqlite_error` with `sqlite3_errmsg` contents,
  propagating detailed error codes, and maintaining `sqlite-available-p`.
- **Version and extension APIs:** `sqlite-version` and guarded
  `sqlite-load-extension`, including URI and `SQLITE_OPEN_FULLMUTEX` flags.

## Dependencies & Tooling
- Rust toolchain (1.80 or newer) already mandated by earlier phases.
- Add `rusqlite` and `libsqlite3-sys` dependencies to `rust/libemacs/Cargo.toml`
  (configure to link against system SQLite; avoid `bundled` unless opted in).
- Continue using `cbindgen` to generate the C header consumed by the shim C
  layer; update generation scripts to include the new functions.
- Reuse `pkg-config` probing from Autotools to feed the paths required by
  `libsqlite3-sys` via environment variables (`SQLITE3_LIB_DIR`, `SQLITE3_INCLUDE_DIR`).

## Architectural Outline
- Extend the existing `rust/libemacs` crate with a new `sqlite` module exporting
  `extern "C"` functions that operate on opaque handles (`ConnectionHandle` and
  `StatementHandle`).
- Represent `struct Lisp_Sqlite` as a thin wrapper: store a tagged pointer to a
  Rust-managed enum (`Connection` vs `Statement`) alongside metadata already
  required by the GC (finalizer, EOF flag). The C side remains responsible for
  object allocation and GC tagging; Rust owns the underlying SQLite resources.
- Mirror the current API surface by providing FFI functions for opening,
  executing, fetching rows, stepping statements, finalizing, and reporting
  errors. C wrappers translate `Lisp_Object` arguments to primitive types and
  delegate to Rust for the database work.
- Use `rusqlite` high-level helpers where practical (parameter binding,
  statement stepping) while falling back to `libsqlite3_sys` for behaviours not
  directly supported (e.g., URI open flags or extension toggles).
- Centralise error messaging in Rust, returning UTF-8 strings that the C shim
  converts to Lisp strings via existing helpers.

## Phase Breakdown

### Phase 0 - Discovery & Baseline (estimate: 2 days)
- Audit `src/sqlite.c` for all exported symbols and internal helpers; confirm no
  additional users beyond the file itself.
- Document the exact behaviour of URI handling, in-memory database naming, and
  full-mutex flags.
- Review `test/src/sqlite-tests.el` and `test/lisp/sqlite-tests.el` to ensure the
  suites cover connection, statement, blob, numeric, and extension flows; file
  gaps as action items (e.g., concurrent statements, error propagation).
- Capture current `make check` results to establish a baseline and archive the
  `sqlite-tests.log` output for comparison.
- Record the Autotools configuration knobs related to SQLite (`HAVE_SQLITE3`,
  `HAVE_SQLITE3_LOAD_EXTENSION`) for reuse in the Rust build.

**Deliverables:** Discovery notes appended to `SqliteRustPlan.md` (or linked
appendix), list of missing tests (if any), and confirmation that no legacy
platform guards remain necessary.

### Phase 1 - FFI Scaffolding (estimate: 3 days)
- Add a `sqlite` module inside `rust/libemacs` exposing stub functions matching
  the eventual API (`emacs_rust_sqlite_open`, `..._exec`, `..._step`, etc.).
- Generate a companion header (`src/rust-sqlite.h`) via `cbindgen` and include it
  from a slimmed-down C shim file (e.g., rename `src/sqlite.c` to
  `src/sqlite-shim.c`).
- Wire Autotools to build the Rust code as part of the normal build when
  `HAVE_SQLITE3` is set; ensure `#ifdef HAVE_SQLITE3` still gates the Lisp
  primitives.
- Initially implement each Rust function as a passthrough to the existing C
  helpers (call into preserved C implementations) to validate the build pipeline.
- Update `RustIntroductionPlan.md` progress notes to record that sqlite porting
  scaffolding has started.

**Exit criteria:** `./configure --with-ns --with-modules` and `make -j` succeed
with the new stubs in place; `sqlite-tests` continue to pass.

### Phase 2 - Rust Implementation (estimate: 5-6 days)
- Replace stubbed functions with real Rust code using `rusqlite`:
  - Manage connections with `rusqlite::Connection`, applying the same open
    flags (URI, memory, full-mutex).
  - Represent prepared statements using `rusqlite::Statement` and iterators;
    implement parameter binding for lists/vectors, including binary blobs.
  - Mirror column extraction logic, returning Lisp scalars via helper functions
    in the C shim (numbers, floats, strings, unibyte blobs).
  - Implement `sqlite-more-p`, `sqlite-next`, and EOF tracking inside Rust while
    keeping the Lisp-visible state on the C side.
  - Port extension gating using `Connection::load_extension_enable` / manual FFI
    if rusqlite lacks direct support.
  - Surface error codes/messages via a shared error enum translated to Lisp
    `sqlite_error` signals by the C shim.
- Delete now-redundant C helpers (`bind_values`, `sqlite_exec_internal`, etc.)
  once the Rust versions are functional.
- Maintain feature parity for `sqlite-available-p` and `sqlite-version` by
  exposing Rust functions that query `rusqlite::version` or
  `libsqlite3_sys::sqlite3_libversion`.

**Exit criteria:** All unit tests introduced in Rust pass (`cargo test -p libemacs`),
`make check` passes on macOS, and runtime functionality matches the baseline.

### Phase 3 - Integration & Cross-Platform Validation (estimate: 3 days)
- Run full builds on macOS (`--with-ns --with-modules`) and Linux
  (`--with-pgtk --with-modules`) with the Rust implementation.
- Execute `make check` in both environments; compare logs to baseline and ensure
  no new expected-fail annotations are required.
- Add targeted stress tests if gaps remain (e.g., concurrent statements, blob
  round-trips larger than 16 KB, error propagation for missing tables).
- Update CI scripts to invoke relevant `cargo test` targets and document any new
  environment prerequisites (e.g., `SQLITE3_*` env vars) in `INSTALL.REPO`.

**Exit criteria:** Both platform builds succeed, test suites pass, and a release
note draft covering the Rust migration is ready in `etc/NEWS` or a staging doc.

### Phase 4 - Cleanup & Default Adoption (estimate: 1-2 days)
- Remove the obsoleted C implementation entirely, leaving only the thin shim and
  the Rust backend; ensure no stale references remain in `src/Makefile.in` or
  `admin/CPP-DEFINES`.
- Update documentation (`RustIntroductionPlan.md`, `DynlibPlan.md` cross-links,
  `README`, `CONTRIBUTE`) to note that SQLite now ships with the Rust
  implementation by default.
- Regenerate build artefacts (`autogen.sh all`, `configure`, `make -j`) and run
  `cargo fmt`/`cargo clippy` to keep the Rust workspace tidy.
- File follow-up issues for optional enhancements (e.g., connection pooling,
  prepared statement cache) or for restoring Windows support if the platform is
  ever reintroduced.

**Exit criteria:** Tree builds cleanly without legacy code; plan marked complete
and archived in `SqliteRustPlan.md`.

## Testing Matrix
| Platform | Build Flags | Tests | Notes |
| --- | --- | --- | --- |
| macOS 14 (arm64) | `--with-ns --with-modules` | `make -j`, `make check`, `cargo test -p libemacs` | Verify homebrew-provided SQLite works with rust bindings. |
| Ubuntu 24.04 (x86_64) | `--with-pgtk --with-modules` | `make -j`, `make check`, `cargo test -p libemacs` | Ensure pkg-config paths bridge into `libsqlite3-sys`. |
| macOS/Linux CI | Same as above | Targeted stress tests (big blobs, extension gating) | Enable once CI images include Rust + sqlite dev headers. |

## Rollback Strategy
- Keep the original `src/sqlite.c` implementation under `#ifdef EMACS_SQLITE_C_FALLBACK`
  during Phase 2/3 so we can flip a configure flag if blockers arise. Delete the
  fallback only after both platforms pass all tests.
- If a regression ships, revert to the last known-good tag containing the C
  implementation and restore the Autotools glue; ensure backups of the deleted
  file exist in version control history.

## Open Questions
1. Should we centralise Lisp object marshaling (numbers, strings, blobs) in shared
   helpers to reuse across future Rust ports (json, itree)?
2. Do we want to expose additional instrumentation (e.g., query timing) once the
   Rust backend lands, or keep behaviour identical for now?
3. How should we package the requirement for `libsqlite3` headers in CI images
   that currently rely on system defaults? Document in `INSTALL.REPO` or add a
   bootstrap script?

---
Last updated: 2025-09-27.
