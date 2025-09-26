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
| Windows/MS-DOS residual glue: `src/conf_post.h` (MSDOS block), `src/w16select.c`, `lisp/term/common-win.el`, `lisp/term/pc-win.el`, `admin/CPP-DEFINES` entries | Leftover runtime shims for ports we have already removed. | With the platform directories gone, these files only add unreachable code and build-time complexity. | Delete the sources, collapse related `#ifdef WINDOWSNT`/`MSDOS` branches, and re-run autoreconf to verify generated files no longer mention these targets. |
| `etc/NEXTSTEP` and GNUstep historical docs | Documentation for previously supported GNUstep deployment. | GNUstep support has been removed; the doc is purely archival. | Decide whether to move the history to `etc/HISTORY` or prune it; ensure `INSTALL`/`README` no longer reference GNUstep as a viable build. |

## Additional Cleanup Opportunities
- Check `lisp/term/` for platform-specific terminal definitions that only exist for retired platforms (e.g., `pc-win.el`, `android-win.el`, `haiku-win.el`) and prune them while keeping the shared TTY support we still rely on.
- Remove conditional compilation blocks guarded by `WINDOWSNT`, `DOS_NT`, `HAVE_ANDROID`, `HAVE_HAIKU`, etc., once their implementations are gone.
- Review `admin/` scripts that package legacy installers or Android artifacts (`admin/download-android-deps.sh`) so they don't linger in release tarballs.

## Phased Execution Plan
- **Phase 1 – Scope Lockdown (Week of 2025-09-29):** Update INSTALL/README/CONTRIBUTE to state the macOS (Cocoa) and Linux (GTK/PGTK) focus; make `./configure` fail fast for unsupported switches such as `--with-android`, `--with-gs`, and Windows options, then regenerate via `autogen.sh all`; align NEWS and CI matrices with the new scope while keeping TTY coverage.
- **Phase 2 – Retired Platform Shims (Early October 2025):** Delete Windows/MS-DOS residue (`src/conf_post.h` MSDOS block, `src/w16select.c`, `lisp/term/common-win.el`, `pc-win.el`) and remove GNUstep artifacts (`etc/NEXTSTEP`, stale references in INSTALL/FolderStructure.md); regenerate build files, reconfigure with `--with-ns --with-modules`, and run `make -j` plus `make check` on macOS to confirm stability.
- **Phase 3 – Android and Haiku Retirement (Mid October 2025):** Excise Android sources (`src/android*.c`, headers, Lisp/tests, admin scripts) and strip `HAVE_ANDROID` logic; drop Haiku UI support (`src/haiku*`, `lisp/term/haiku-win.el`) while verifying GTK/PGTK and NS builds still pass bootstrap, test, and GUI smoke checks on Linux and macOS; archive or tag the removed code as needed.

## Sequencing Recommendations
1. **Plan the order**: Continue pruning remaining platform directories (e.g., legacy documentation) so downstream references can be removed methodically.
2. **Adjust the build system**: Update `configure.ac`, regenerate `configure` with `autogen.sh`, and remove related options from `INSTALL.REPO`.
3. **Remove dependent source paths**: Use `rg`/`git grep` to eliminate residual `#ifdef` branches and load-path entries referencing the removed directories.
4. **Prune documentation**: Delete the obsolete manuals and update `doc/` indices to avoid build failures in the Info manuals.
5. **Validate on target platforms**: Re-run full macOS Cocoa and Linux GTK/PGTK builds (GUI + TTY) to ensure no regressions.

## Risks & Mitigations
- **Hidden dependencies**: Some Lisp packages may still reference Windows- or GNUstep-specific features. Run `make check` and grep for platform guards after removal.
- **Community patches**: External contributors might expect Windows/Android support; clearly communicate the narrower scope in `CONTRIBUTE` and release notes.
- **Build system brittleness**: Aggressive pruning can break Autotools logic. Maintain incremental commits with CI on macOS/Linux to catch regressions early.

## Next Steps
- Decide whether to archive the removed directories in a branch/tag before deletion.
- Stage the cleanup in small patches (one platform/toolkit at a time) to simplify review and rollback.
- After each removal, run `make bootstrap`, `make check`, and GUI smoke tests on both supported platforms.
