# SQLite Rust Migration Plan (Feature Clusters)

## Objective
Incrementally replace the SQLite runtime in `src/sqlite.c` with a Rust backend by
working cluster-by-cluster. Each cluster focuses on a related set of features so
we can land small, verifiable changes without regressing the Lisp-visible API.

## Global Guardrails
- **Parity First:** Keep Lisp primitives, error signalling, and GC contracts intact.
- **Incremental Merge:** After each cluster, the tree must configure, build, and
  pass the relevant portions of `make check` on macOS (Cocoa) and Linux (PGTK).
- **Fallback Lever:** During development, retain a simple toggle (e.g., a build
  option or runtime dispatch) so we can fall back to the legacy C path if a
  cluster exposes regressions.

### Shared Prerequisites
- Rust toolchain ≥ 1.80, `cargo`, `cbindgen`, and system `libsqlite3` headers.
- Existing ERT coverage: `test/src/sqlite-tests.el`, `test/lisp/sqlite-tests.el`.
- Baseline logs from the current C implementation (`make check`, `cargo test -p libemacs`).

---
## Cluster Roadmap

### Cluster A – Connection Lifecycle & Availability
**Scope**
- Primitives: `sqlite-open`, `sqlite-close`, `sqlitep`, `sqlite-available-p`, `sqlite-version`.
- Internal helpers: `check_sqlite`, `make_sqlite`, GC finalizer, open flags, in-memory naming.

**Tasks**
1. Extend Rust FFI with connection handles, open/close logic, availability probe,
   and version string helpers.
2. Adjust `struct Lisp_Sqlite` to store opaque Rust handles while keeping GC semantics.
3. Update C helpers to interpret Rust error codes and messages; preserve
   `db_count` behaviour for in-memory DBs.
4. Add focused tests: open/close failure cases, version string parity, GC finalizer smoke test.

**Exit Criteria**
- `sqlite-open`/`close` exercise the Rust path; all connection lifecycle tests pass.
- `make check` subset covering `sqlite-open/close/version` is clean on macOS.
- Fallback mechanism still in place for subsequent clusters.

---
### Cluster B – Statement Preparation & Iteration
**Scope**
- Primitives: `sqlite-select` (`return-type` variants), `sqlite-columns`, `sqlite-more-p`,
  `sqlite-next`, `sqlite-finalize`.
- Helpers: parameter binding (`bind_values`), row materialisation (`row_to_value`), column naming.

**Tasks**
1. Implement Rust-side prepared statement management (creation, binding, iteration,
   EOF tracking) returning FFI-friendly row data.
2. Rewire C wrappers to translate Lisp vectors/lists into the FFI parameter array
   and to decode row results (respecting UTF-8 vs. binary semantics).
3. Update tests to cover parameter binding edge cases (blobs, encodings, booleans)
   and call paths for `return-type = set/full`.
4. Benchmark representative queries to confirm no major regressions; capture notes.

**Exit Criteria**
- All statement-related primitives run through Rust implementation.
- `test/src/sqlite-tests.el` passes on macOS (no new expected failures).
- Verified that blob/text encoding logic matches the C baseline (manual diff vs. logs).

---
### Cluster C – Exec & Row-Count Operations
**Scope**
- Primitives: `sqlite-execute`, `sqlite-execute-batch`, `sqlite-pragma`, `sqlite-changes`
  (from Lisp wrappers), and internal helpers such as `sqlite_exec`.

**Tasks**
1. Add Rust FFI entry points for non-select statements and batch execution, returning
   affected row counts and error metadata.
2. Port pragma execution and row-count retrieval; ensure URI/readonly flags are still honoured.
3. Extend tests to cover batch failures, pragma round-trips, and row-count assertions.
4. Confirm the new code respects the allowlist for multi-statement batches (if applicable).

**Exit Criteria**
- Exec/batch paths operate via Rust and pass regression tests on macOS & Linux.
- `sqlite-changes` (via Lisp wrappers) reports identical counts to the C baseline.

---
### Cluster D – Transactions & Error Contracts
**Scope**
- Primitives: `sqlite-transaction`, `sqlite-commit`, `sqlite-rollback` and Lisp-side
  macros depending on them; shared error machinery (`sqlite_error`, `sqlite_locked_error`).

**Tasks**
1. Leverage Cluster C execution helpers to implement transaction commands in Rust.
2. Audit error propagation: ensure locked/busy errors still raise the correct
   condition symbols and include extended codes.
3. Add stress tests covering nested transactions, rollback-on-error, and lock contention.
4. Capture diagnostic logging for debugging (optional, guarded by `#ifdef`/feature flag).

**Exit Criteria**
- Transaction helpers run through Rust with unchanged error semantics.
- `test/lisp/sqlite-tests.el` (transaction macros) clean on both macOS and Linux.

---
### Cluster E – Extension Loading & Metadata
**Scope**
- Primitives: `sqlite-load-extension`, allowlist enforcement, column metadata helpers.
- Build hooks: requirement for `sqlite3_load_extension`, configure-time detection.

**Tasks**
1. Port `sqlite-load-extension` path to Rust, reusing existing allowlist and
   ensuring extensions are enabled/disabled atomically.
2. Mirror column metadata support (already surfaced via Cluster B) and ensure
   extension errors produce actionable messages.
3. Add opt-in integration test (skipped by default) that loads a known-safe
   extension; document prerequisites.
4. Update configure help/INSTALL docs to note new requirements (if any).

**Exit Criteria**
- Extension loading uses Rust backend without relaxing security gates.
- Optional smoke test documented; default `make check` remains unaffected (skipped when extensions absent).

---
### Cluster F – Cross-Cutting Error & Encoding Polish
**Scope**
- Consistency checks for error codes/messages, UTF-8 vs. binary conversion, GC finalizers.
- Update documentation (NEWS, README, RustIntroductionPlan.md) once parity is confirmed.

**Tasks**
1. Perform diff of error outputs (Rust vs. C) for key failure scenarios; adjust
   error formatting helpers if needed.
2. Revisit `struct Lisp_Sqlite` to remove legacy fields or guards no longer needed.
3. Update docs and plan trackers; remove fallback toggles once confident.
4. Run full `make check` on macOS/Linux and `cargo test -p libemacs`; capture final logs.

**Exit Criteria**
- No observable behavioural drift vs. baseline in error paths or encoding.
- Legacy C code removed or gated off; documentation reflects Rust default.

---
## Execution Tips
- After each cluster, rerun `./autogen.sh all && ./configure --with-ns --with-modules` (macOS)
  and `./configure --with-pgtk --with-modules` (Linux), plus relevant subsets of `make check`.
- Keep `src/rust-sqlite.h` generation scripted (e.g., `cbindgen` rule) to avoid manual drift.
- Use environment isolation in tests (`ZDOTDIR`, hunspell dictionaries, rust-analyzer) as set up earlier.

_Last updated: 2025-09-28_
