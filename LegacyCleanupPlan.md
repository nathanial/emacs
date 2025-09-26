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
| Directory | Primary Purpose | Why It Can Likely Be Removed | Follow-Up Tasks & Risks |
|-----------|------------------|------------------------------|--------------------------|
| `msdos/` | MS-DOS port sources, docs, and build glue. | Removed on 2025-09-25; MS-DOS is no longer a supported target. | Ensure any lingering conditionals guarding MS-DOS code paths are cleaned up as subsequent refactors land. |
| `nt/` | Windows (NT) port including resource files, w32 GUI back-end, installer scripts. | Removed on 2025-09-26; Windows support is no longer part of the target matrix. | Monitor for residual `WINDOWSNT` conditionals that can be simplified in subsequent refactors. |
| `java/` | Android port scaffolding and Gradle project. | Removed on 2025-09-26; Android packages are no longer built from this tree. | Continue auditing `--with-android` configure logic and `HAVE_ANDROID` code for retirement in future passes. |
| `cross/` | Cross-compilation helper configs for niche targets (e.g., MIPS, ARM). | Removed on 2025-09-26; cross-compilation scaffolding is no longer supported. | Double-check configuration help text (`--with-android`, `--with-ndk-*`) and contributor docs to reflect the narrower platform scope. |
| `lwlib/` | Lucid Widget library (Motif-style X toolkit). | Removed on 2025-09-26; GTK/PGTK now provide the supported X GUI paths. | Documentation pruning (e.g., `xresources` Lucid appendix) still pending; source code references guarded by `USE_LUCID`/`USE_MOTIF` were scrubbed on 2025-09-26. |
| `doc/misc/efaq-w32.texi`, `doc/misc/ntfaq.texi`, related w32 docs | Manuals for legacy platforms. | Once Windows support is removed, these manuals become obsolete clutter. | Delete the files, update `doc/misc/Makefile.in`, and scrub references from the Info directory map. |

## Additional Cleanup Opportunities
- Check `lisp/term/` for platform-specific terminal definitions (`pc-win.el`) and remove them alongside any remaining platform-specific back ends.
- Remove conditional compilation blocks guarded by `WINDOWSNT`, `DOS_NT`, `HAVE_ANDROID`, etc., once their directories disappear.
- Review `admin/` scripts that package legacy installers or Android artifacts (`admin/android/`).

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
