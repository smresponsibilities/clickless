# Crates.io publication

Date: 2026-10-03. Publication requested by owner. Not published yet.

## Status

- Registry API returned 404 for `clickless`, `clickless-cli`, `clickless-core`, `clickless-backend-api`, `clickless-config`, `clickless-output-enigo`, `clickless-windows`, `clickless-linux` and `clickless-macos`. Availability can change before publication. No separate name reservation exists.
- Owner selected installable package `clickless`. Source folder remains `crates/clickless-cli`; library identifier remains `clickless_cli`. Executables remain `clickless`, `clicklessctl`, and `clickless-settings`.
- Owner selected `MIT OR Apache-2.0`. Root and every crate archive include LICENSE-MIT and LICENSE-APACHE.
- Local Cargo credential file absent and `CARGO_REGISTRY_TOKEN` unset. No credential contents were read. Use `cargo login` privately, never paste the token in chat or commit it.
- All eight packages now carry description, repository and readme metadata. Internal path dependencies also specify registry version `0.1.0`.
- `cargo package -p clickless-core --allow-dirty --target-dir target/modern` packaged and compiled successfully, with license files included. This does not publish or reserve its name.
- Core `cargo publish --dry-run` passed without upload. All eight package file lists checked; no logs or credential files included. Windows resources and CLI source targets present.
- Baseline Windows/Linux/macOS CI passed. Linux/macOS backends remain experimental, as described in the platform plan. Cargo installation does not install Windows App SDK runtime or macOS/Linux desktop permissions.

## Publication sequence

1. Record owner license decision; add selected license text/files and matching SPDX field to each manifest. Check third-party asset attribution before upload.
2. Confirm GitHub repository contents match packaged source. Review package manifests/file lists, including Windows XAML, icon, tray RGBA, RC and manifest resources. Reject temporary logs or credentials in archives.
3. Run full repository gates. Commit release metadata after required two-axis review. Auth locally with publish-scoped credentials.
4. Publish dependency order below, verifying each version appears in registry before dependent package dry-run. Use `cargo publish -p NAME --dry-run` then `cargo publish -p NAME`. Cargo resolves registry dependencies even for dry-runs; dependent dry-runs cannot fully pass until prerequisite versions exist.

```text
clickless-core
clickless-backend-api
clickless-config
clickless-output-enigo
clickless-linux
clickless-macos
clickless-windows
clickless
```

5. Verify metadata/version on crates.io, ownership account, and fresh `cargo install clickless --version 0.1.0 --locked`. Check on each advertised OS. Windows default UI needs Microsoft Windows App SDK runtime. Installing successfully is separate from native input usability.
6. Record publication URLs and exact versions. Add release tag only after actual upload. If a published package is broken, publish a corrected version; crates.io versions cannot be overwritten. Yank only with owner authorization and awareness of downstream installs.

Publishing backend packages is required by the current CLI dependency graph. Do not advertise independent backend stability or complete desktop parity merely because packages exist on crates.io. No placeholder crate is published to squat on `clickless`.

## Commands after authentication

Run `cargo login` privately, then run `powershell -File scripts/publish.ps1` from repository root. Script dry-runs and publishes in dependency order, stopping on failure. A later failure can leave earlier packages published; inspect registry receipts before retrying. No upload has occurred in this session yet.
