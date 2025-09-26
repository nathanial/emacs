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
- [ ] Audited remaining `HAVE_ANDROID`/`HAVE_HAIKU` references across the tree; removal work tracked in Phase 2 (inventory captured 2025-09-26).

## Remaining Legacy Surfaces
| Area | Primary Purpose | Status | Follow-Up Tasks & Risks |
|------|-----------------|--------|-------------------------|
| Android-specific conditionals (`HAVE_ANDROID`, `ANDROID_STUBIFY`, `android.h`) | Alternative code paths for Android runtime, fonts, loader, and UI. | Inventory complete; removal planned in Phase 2. | Strip macros/includes across C/Lisp, align configure help, confirm builds succeed after guard removal. |
| Haiku conditionals (`HAVE_HAIKU`, `haiku_*` code) | Legacy Haiku window-system integration. | Inventory complete; removed files but guards remain. | Delete Haiku branches alongside Android cleanup, update docs/tooling to drop references. |
| Configure / doc references (`configure.ac`, `INSTALL*`, `etc/NEWS`, `admin/CPP-DEFINES`) | Advertise unsupported platforms/options. | Scope messaging landed in Phase 1; fine-grained references remain. | Remove obsolete flags, rerun Autotools, refresh docs once Android/Haiku code is gone. |
| Residual Windows/MS-DOS guards (`WINDOWSNT`, `MSDOS`, `GNUSTEP`) | Dead branches in C/Lisp and helper scripts. | Major files deleted; scattered guards persist. | Fold cleanup into Phase 2 sweeps; keep list of remaining symbols for final confirmation. |

## Additional Cleanup Opportunities
- Fold removal of remaining `WINDOWSNT`/`MSDOS` conditionals into the Android/Haiku sweeps so touched files end up fully platform-neutral.
- Capture a canonical list of external tool dependencies (`rust-analyzer`, `clangd`, etc.) needed for `make check` and document skip strategies in CI.
- Consider archiving deleted platform content (e.g., tarball or git tag) for historical reference before stripping final guard macros.

## Phased Execution Plan
- **Phase 1 – Scope Lockdown (Completed 2025-09-26):** Updated INSTALL/README/CONTRIBUTE to clarify the macOS Cocoa + Linux GTK/PGTK focus, made `./configure` reject unsupported switches (Android, Windows, GNUstep, etc.), regenerated Autotools artifacts via `./autogen.sh all`, refreshed NEWS/CI matrices, and verified GUI/TTY builds on macOS.
- **Phase 2 – Platform Residue Purge (Target start 2025-10-06):**
  - Remove all `HAVE_ANDROID`/`ANDROID_STUBIFY`/`android.h` usage across core subsystems (fonts, GC, loader/doc readers, terminal/keyboard/image code, build helpers).
  - Eliminate remaining `HAVE_HAIKU` guards now that the backend is gone, collapsing branches into the supported macOS/Linux paths.
  - Strip obsolete configure/help text, rerun `./autogen.sh all`, and validate `./configure --with-ns --with-modules && make -j` on macOS/Linux after each subsystem sweep.
  - Track residual `WINDOWSNT`/`MSDOS` guards encountered during the sweep for cleanup in the same pass where feasible.
- **Phase 3 – Documentation & CI Alignment (Target start 2025-10-13):**
  - Prune Texinfo nodes and manual sections referencing Lucid, Motif, GNUstep, Android, Haiku, or Windows; ensure `make info` succeeds without missing includes.
  - Update `INSTALL`, `INSTALL.REPO`, `etc/NEWS`, `admin/CPP-DEFINES`, and contributor docs to match the final platform matrix post-sweep.
  - Simplify CI workflows by removing Android/Haiku jobs or cache paths; document external tool requirements (e.g., `rust-analyzer`) and adjust skips where necessary.
  - Run full `make bootstrap`, `make -j`, and targeted `make check` on macOS Cocoa and Linux GTK/PGTK, capturing any new issues introduced by the purge.
- **Phase 4 – Polishing & Historical Cleanup (Optional, 2025-10-20+):**
  - Remove now-redundant comments/notes about retired ports, tidy `ChangeLog` references where appropriate, and archive deleted platform trees in a separate branch/tag if desired.
  - Review optional tooling (packaging scripts, dist targets) for legacy assumptions.

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
- Queue Phase 2 work: start by deleting the shared `android.h` include and collapsing `HAVE_ANDROID` guards in headers/runtime, then iterate through subsystem checklists (fonts, loader, image back-ends, term/keyboard).
- As each subsystem lands, regenerate Autotools, reconfigure, and smoke-test on macOS/Linux; log any persistent `WINDOWSNT`/`MSDOS` guards for follow-up in the same patchset.
- Once code paths are clean, begin Phase 3 documentation/CI updates so manuals and automation stay in sync with the supported platform story.
