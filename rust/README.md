# Emacs Rust Workspace

This directory hosts the Rust workspace used for the gradual replacement of
selected `lib-src` utilities and leaf runtime modules.  The workspace relies on
`cargo` and the stable Rust toolchain.  Binaries are built with the `release`
profile and staged under `build/rust/` to mirror the existing out-of-tree build
layout used by Autotools.

Current members:

- `hexl`: Hexl mode converter, providing the existing CLI switches for encoding
  and decoding.
- `update-game-score`: Updates the shared game score files with the same locking
  semantics as the former C implementation.
- `make-fingerprint`: Computes the build fingerprint for `temacs` or rewrites
  the placeholder hash in place.

Future Rust crates should be added to `Cargo.toml` and follow the same layout so
that both Autotools and the forthcoming CMake build can share the workspace.
