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
- [x] Audited remaining `HAVE_ANDROID`/`HAVE_HAIKU` references across the tree and removed them on 2025-09-26 (Phase 2).
- [x] Pruned optional tooling that referenced legacy platforms (git hooks, maintainer metadata, merge helpers) on 2025-09-26 (Phase 4).

## Remaining Legacy Surfaces
| Area | Primary Purpose | Status | Follow-Up Tasks & Risks |
|------|-----------------|--------|-------------------------|
| Android-specific conditionals (`HAVE_ANDROID`, `ANDROID_STUBIFY`, `android.h`) | Alternative code paths for Android runtime, fonts, loader, and UI. | Removed 2025-09-26. | Monitor downstream tooling for leftover Android assumptions; flag any external docs that still promise the port. |
| Haiku conditionals (`HAVE_HAIKU`, `haiku_*` code) | Legacy Haiku window-system integration. | Removed 2025-09-26. | Historical notes only; keep an eye on archival scripts that might still enumerate Haiku artefacts. |
| Configure / doc references (`configure.ac`, `INSTALL*`, `etc/NEWS`) | Release messaging & build docs. | Updated 2025-09-26 (Phase 3). | Spot-check remaining ChangeLogs/README snippets during final edit pass. |
| Residual Windows/MS-DOS guards (`WINDOWSNT`, `MSDOS`, `GNUSTEP`) | Dead branches in C/Lisp and helper scripts. | In progress. | Schedule one more grep-assisted sweep once doc/CI updates settle. |
| CI/test tooling coverage | Document external dependencies & coverage gaps. | Pending. | Capture Linux PGTK results and record tool requirements (`rust-analyzer`, `clangd`, etc.) before sign-off. |

## Additional Cleanup Opportunities
- Continue pruning remaining `WINDOWSNT`/`MSDOS` conditionals alongside upcoming documentation/test passes so touched files end up fully platform-neutral.
- Capture a canonical list of external tool dependencies (`rust-analyzer`, `clangd`, etc.) needed for `make check` and document skip strategies in CI.
- Consider archiving deleted platform content (e.g., tarball or git tag) for historical reference before stripping final guard macros.

## Phased Execution Plan
- **Phase 1 – Scope Lockdown (Completed 2025-09-26):** Updated INSTALL/README/CONTRIBUTE to clarify the macOS Cocoa + Linux GTK/PGTK focus, made `./configure` reject unsupported switches (Android, Windows, GNUstep, etc.), regenerated Autotools artifacts via `./autogen.sh all`, refreshed NEWS/CI matrices, and verified GUI/TTY builds on macOS.
- **Phase 2 – Platform Residue Purge (Completed 2025-09-26):** Removed every `HAVE_ANDROID`/`ANDROID_STUBIFY`/`android.h` and `HAVE_HAIKU` guard across C/Lisp sources (fonts, GC, loader, display back-ends), dropped the Android epaths overrides, regenerated `configure` via `./configure --with-ns --with-modules`, and rebuilt with `make -j8` on macOS to confirm a clean tree. Residual `WINDOWSNT`/`MSDOS` guards observed during the sweep are marked for follow-up in later passes.
- **Phase 3 – Documentation & CI Alignment (Completed 2025-09-26):**
  - Excised dedicated Texinfo nodes for Android, Haiku, Windows, Lucid, Motif, and GNUstep across `doc/lispref`, `doc/misc/tramp.texi`, and supporting manuals; regenerated manuals via `make info` (which triggered a successful macOS bootstrap build in the process).
  - Refreshed top-level collateral (`INSTALL`, `etc/NEWS`, `LegacyCleanupPlan.md`) to describe the macOS Cocoa + Linux GTK/PGTK scope only.
  - Verified GitLab pipeline definitions already ignore legacy ports; follow-up documentation of external tool requirements (e.g., `rust-analyzer`, `clangd`) remains to be captured alongside Linux test coverage.
- **Phase 4 – Polishing & Historical Cleanup (Completed 2025-09-26):**
  - Reviewed optional tooling (git hooks, merge helpers, maintainer rosters) and removed dead references to Android/Haiku/Windows-era assets.
  - Remaining optional follow-ups: archive historical material separately if desired and prune obsolete code comments when convenient.

## Sequencing Recommendations
1. **Subsystem sweep order**: Work top-down—start with shared headers and low-level runtime (file I/O, fonts, GC), then move to UI/image back-ends, and finally documentation/CI. This minimizes merge pain and keeps buildability high.
2. **Autotools cadence**: After each major removal batch, run `./autogen.sh all`, reconfigure, and rebuild on macOS and Linux to catch regressions before they pile up.
3. **Guard verification**: Use `rg 'HAVE_ANDROID|HAVE_HAIKU|WINDOWSNT|MSDOS'` after every sweep to ensure no stray conditionals remain except in historical notes.
4. **Documentation alignment**: Update manuals and `INSTALL*` files in lockstep with code deletions so Texinfo indices never reference removed nodes.
5. **Test coverage**: Maintain `make bootstrap`, `make -j`, and targeted `make check` (with documented skips for missing external tools) on both supported platforms before closing out each phase.

## Risks & Mitigations
- **Hidden dependencies**: Some Lisp packages may still reference Windows- or GNUstep-specific features. Run `make check` and grep for platform guards after removal.
- **Test flakiness**: `make check` currently fails when Rust tooling (`rust-analyzer`) is unavailable; capture these as expected in CI or provide skip hooks before shipping Phase 3 deletions.
- **Community patches**: External contributors might expect Windows/Android support; clearly communicate the narrower scope in `CONTRIBUTE` and release notes.
- **Build system brittleness**: Aggressive pruning can break Autotools logic. Maintain incremental commits with CI on macOS/Linux to catch regressions early.

## Next Steps
- Mirror the `./configure --with-pgtk --with-modules && make -j8` cycle on a Linux host, followed by `make check`, and capture any required skips or tooling notes (especially missing language servers).
- Record external tool requirements for CI/test runs (e.g., `rust-analyzer`, `clangd`, image converters) in `INSTALL.REPO` or the CI README so new environments bootstrap cleanly.
- Schedule a final `WINDOWSNT`/`MSDOS` guard audit once the documentation dust settles, removing or annotating the remaining dead branches.
