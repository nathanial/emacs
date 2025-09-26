# Legacy Cleanup Plan

## Goal
Rationalize the Emacs source tree for a codebase that only targets modern macOS (Cocoa/NS) and contemporary Linux distributions (GTK/PGTK), eliminating directories that primarily serve legacy platforms or obsolete toolkits.

## Assumptions
- GUI support will continue to rely on `nextstep/Cocoa` for macOS and GTK/PGTK for Linux.
- Terminal and TTY builds remain required on both platforms.
- We can afford to rework build scripts after pruning directories but want to avoid deep refactors outside the flagged components.

## Progress Update
- [x] Removed `msdos/` directory and associated MS-DOS port sources on 2025-09-25.
- [x] Removed `nt/` port directories, build hooks, and documentation on 2025-09-26.
- [x] Removed `java/` Android packaging directory and host tooling on 2025-09-26.
- [x] Removed `cross/` cross-compilation scaffolding and build glue on 2025-09-26.
- [x] Removed `oldXMenu/` legacy X11 menu library and ended non-toolkit X builds on 2025-09-26.
- [x] Removed `lwlib/` Lucid widget toolkit implementation on 2025-09-26.
- [x] Removed `nextstep/GNUstep/` GNUstep bundle assets and support on 2025-09-26.
- [x] Retired Windows/MS-DOS shims (`src/w16select.c`, `lisp/term/pc-win.el`) and dropped the `etc/NEXTSTEP` GNUstep doc stub on 2025-09-26.

## Candidate Directories to Retire
| Directory / Files | Primary Purpose | Why It Can Likely Be Removed | Follow-Up Tasks & Risks |
|-------------------|------------------|------------------------------|--------------------------|
| `msdos/` | MS-DOS port sources, docs, and build glue. | Removed on 2025-09-25; MS-DOS is no longer a supported target. | Ensure any lingering conditionals guarding MS-DOS code paths are cleaned up as subsequent refactors land. |
| `nt/` | Windows (NT) port including resource files, w32 GUI back-end, installer scripts. | Removed on 2025-09-26; Windows support is no longer part of the target matrix. | Monitor for residual `WINDOWSNT` conditionals that can be simplified in subsequent refactors. |
| `java/` | Android port scaffolding and Gradle project. | Removed on 2025-09-26; Android packages are no longer built from this tree. | Continue auditing `--with-android` configure logic and `HAVE_ANDROID` code for retirement in future passes. |
| `cross/` | Cross-compilation helper configs for niche targets (e.g., MIPS, ARM). | Removed on 2025-09-26; cross-compilation scaffolding is no longer supported. | Double-check configuration help text (`--with-android`, `--with-ndk-*`) and contributor docs to reflect the narrower platform scope. |
| `lwlib/` | Lucid Widget library (Motif-style X toolkit). | Removed on 2025-09-26; GTK/PGTK now provide the supported X GUI paths. | Documentation pruning (e.g., `xresources` Lucid appendix) still pending; source code references guarded by `USE_LUCID`/`USE_MOTIF` were scrubbed on 2025-09-26. |
| `src/android*`, `lisp/term/android-win.el`, `test/infra/android/`, `admin/download-android-deps.sh` | Android runtime, Lisp front-ends, and CI helpers. | We no longer target Android, so the runtime stubs, tests, and tooling are dead weight. | Remove `HAVE_ANDROID`/`ANDROID_STUBIFY` guards, drop configure options, and ensure remaining files do not expect JNI or Android assets. |
| `src/haiku*`, `lisp/term/haiku-win.el`, `src/haiku_*` support files | Haiku window-system implementation. | Haiku is outside the supported macOS/Linux matrix; keeping it increases maintenance burden. | Rip out Haiku-specific code paths, simplify toolkit selection logic, and update docs (`INSTALL`, `etc/NEWS`) accordingly. |
| Windows/MS-DOS residual glue: `src/conf_post.h` (legacy guards), `lisp/term/common-win.el` (old Windows branches), `admin/CPP-DEFINES` entries | Leftover conditionals for retired ports. | Major shims (`src/w16select.c`, `lisp/term/pc-win.el`) are gone, and `common-win.el` no longer checks `system-type 'windows-nt`. | Continue pruning `WINDOWSNT`/`MSDOS` branches in C and Lisp sources as they are encountered during Phase 3. |

## Additional Cleanup Opportunities
- Continue trimming platform-specific conditionals (e.g., `WINDOWSNT`, `DOS_NT`) that are now permanently false after the legacy removals.
- Audit manuals and user-facing docs for residual references to the retired Android/Haiku ports and update wording accordingly.
- Revisit CI job definitions once Android/Haiku artifacts are absent to ensure no stale cache paths remain.

## Phased Execution Plan
- **Phase 1 – Scope Lockdown (Week of 2025-09-29):** Update INSTALL/README/CONTRIBUTE to state the macOS (Cocoa) and Linux (GTK/PGTK) focus; make `./configure` fail fast for unsupported switches such as `--with-android`, `--with-gs`, and Windows options, then regenerate via `autogen.sh all`; align NEWS and CI matrices with the new scope while keeping TTY coverage.
- **Phase 2 – Retired Platform Shims (Completed 2025-09-26):** Removed the remaining Windows/MS-DOS shims (`src/w16select.c`, `lisp/term/pc-win.el`), scrubbed the MSDOS/MinGW guards in `src/conf_post.h`, trimmed Windows-only logic from `lisp/term/common-win.el`, dropped the `etc/NEXTSTEP` historical doc, refreshed `admin/CPP-DEFINES`, regenerated `configure` via `autogen.sh`, and rebuilt with `./configure --with-ns --with-modules` followed by `make -j` and `make check` on macOS (GUI build succeeded; `make check` still reports known Eglot/rust-analyzer failures in the local environment).
- **Phase 3 – Android and Haiku Retirement (Completed 2025-09-26):** Deleted the Android and Haiku runtime trees (`src/android*`, `src/haiku*`, `lisp/term/android-win.el`, `lisp/term/haiku-win.el`, `test/infra/android/`, `admin/download-android-deps.sh`), pruned configure flags and Autotools logic for both platforms, refreshed makefiles and `admin/CPP-DEFINES`, and updated documentation to reflect the macOS/Linux-only policy.  Regenerated the build system (`./autogen.sh all`), reconfigured with `./configure --with-ns --with-modules`, and rebuilt successfully with `make -j`.  Follow-up: continue simplifying residual source guards that mention the removed platforms and expand doc clean-up passes.

## Sequencing Recommendations
1. **Plan the order**: Continue pruning remaining platform directories (e.g., legacy documentation) so downstream references can be removed methodically.
2. **Adjust the build system**: Update `configure.ac`, regenerate `configure` with `autogen.sh`, and remove related options from `INSTALL.REPO`.
3. **Remove dependent source paths**: Use `rg`/`git grep` to eliminate residual `#ifdef` branches and load-path entries referencing the removed directories.
4. **Prune documentation**: Delete the obsolete manuals and update `doc/` indices to avoid build failures in the Info manuals.
5. **Validate on target platforms**: Re-run full macOS Cocoa and Linux GTK/PGTK builds (GUI + TTY) to ensure no regressions.

## Risks & Mitigations
- **Hidden dependencies**: Some Lisp packages may still reference Windows- or GNUstep-specific features. Run `make check` and grep for platform guards after removal.
- **Test flakiness**: `make check` currently fails when Rust tooling (`rust-analyzer`) is unavailable; capture these as expected in CI or provide skip hooks before shipping Phase 3 deletions.
- **Community patches**: External contributors might expect Windows/Android support; clearly communicate the narrower scope in `CONTRIBUTE` and release notes.
- **Build system brittleness**: Aggressive pruning can break Autotools logic. Maintain incremental commits with CI on macOS/Linux to catch regressions early.

## Next Steps
- Decide whether to archive the removed directories in a branch/tag before deletion.
- Stage the cleanup in small patches (one platform/toolkit at a time) to simplify review and rollback.
- After each removal, run `make bootstrap`, `make check`, and GUI smoke tests on both supported platforms.
