# Crates.io publication

Date: 2026-10-04. All eight packages, including installable CLI, published as 0.1.1.

## Status

Current registry receipts are in the 0.1.1 section below. Earlier observations describe preparation history, not current availability or authentication.

The unpublished 0.1.2 candidate adds `clickless-ui`. Its publication order is after `clickless-config` and before `clickless-windows`; the publication script includes it. Published 0.1.1 receipts below remain unchanged.

- Registry API returned 404 for `clickless`, `clickless-cli`, `clickless-core`, `clickless-backend-api`, `clickless-config`, `clickless-output-enigo`, `clickless-windows`, `clickless-linux` and `clickless-macos`. Availability can change before publication. No separate name reservation exists.
- Owner selected installable package `clickless`. Source folder remains `crates/clickless-cli`; library identifier remains `clickless_cli`. Executables remain `clickless`, `clicklessctl`, and `clickless-settings`.
- Owner selected `MIT OR Apache-2.0`. Root and every crate archive include LICENSE-MIT and LICENSE-APACHE.
- Local Cargo credential file absent and `CARGO_REGISTRY_TOKEN` unset. No credential contents were read. Use `cargo login` privately, never paste the token in chat or commit it.
- All eight packages carry description, repository and readme metadata. Current package and internal dependency minimum versions are `0.1.1`.
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

5. Verify metadata/version on crates.io, ownership account, and fresh `cargo install clickless --version 0.1.1 --locked`. Check on each advertised OS. Windows default UI needs Microsoft Windows App SDK runtime. Installing successfully is separate from native input usability.
6. Record publication URLs and exact versions. Add release tag only after actual upload. If a published package is broken, publish a corrected version; crates.io versions cannot be overwritten. Yank only with owner authorization and awareness of downstream installs.

Publishing backend packages is required by the current CLI dependency graph. Do not advertise independent backend stability or complete desktop parity merely because packages exist on crates.io. No placeholder crate is published to squat on `clickless`.

## Commands after authentication

Run `cargo login` privately if authentication is required, then run `powershell -File scripts/publish.ps1` from repository root after release gates pass. Script dry-runs and publishes in dependency order, stopping on failure. A later failure can leave earlier packages published; inspect registry receipts before retrying.

## Actual publication attempt

2026-10-03: `cargo publish -p clickless-core --locked` packaged and verified successfully, then crates.io rejected upload with HTTP 400: "A verified email address is required to publish crates to crates.io." No crate published. Verify account email at https://crates.io/settings/profile, then rerun scripts/publish.ps1. Cargo reached account validation despite no credentials.toml or token environment variable found earlier; the registry response is the current blocker. Run cargo login only if Cargo requests authentication.

## Rate-limit resume, 2026-10-03

Registry confirmed 0.1.0 published and not yanked for core, backend-api, config, output-enigo and linux. macos, windows and clickless are not published. User's upload received HTTP429 with retry after 2026-10-03 08:29:06 UTC, 13:59:06 IST. Wait until that time before rerunning script. Script now reads workspace versions and skips already-published non-yanked versions. Only a registry404 permits publication; rate limits/network failures stop. Repeated uploads do not overwrite immutable versions.

## Release graph repair, 2026-10-04

Registry recheck confirmed those five 0.1.0 packages remain published and not yanked. macos, windows and clickless 0.1.0 still return 404. License choice is settled as MIT OR Apache-2.0; earlier tracker entries listing that choice as missing are stale.

`cargo publish -p clickless-windows --dry-run --locked` failed with unresolved `clickless_config::editor`: published config 0.1.0 predates editor extraction. Workspace tests cannot catch this registry-source mismatch. macOS dry-run passed on Windows, which does not prove native macOS compilation.

All eight publishable packages and internal dependency minimums now use 0.1.1. Existing 0.1.0 uploads remain untouched. This is a prepared candidate, not a published release. Publish dependency order above only after required gates pass.

Current Cargo supports `cargo package --workspace --exclude clickless-gpui-sample --locked`. It stages workspace archives in a temporary local registry and verifies each archive against those packaged dependencies before any upload. Native CI now runs this check on all three platforms. Individual dependent registry dry-runs still require prerequisite versions to be published. Workspace source tests and archive verification remain distinct gates.

## 0.1.1 publication receipts, 2026-10-04

Reviewed publication script completed with exit 0. Each package passed its individual registry dry-run before upload. Published source is Git commit `6d2c463`, including icon attribution in the packaged README. Native code/manifests/lockfile are identical to candidate `c349298`, whose [CI run](https://github.com/smresponsibilities/clickless/actions/runs/37150917384) passed all three runners, full tests and eight archive builds. Local gates and archive-attribution inspection passed after README update.

| Package | Registry version |
| --- | --- |
| clickless-core | [0.1.1](https://crates.io/crates/clickless-core/0.1.1) |
| clickless-backend-api | [0.1.1](https://crates.io/crates/clickless-backend-api/0.1.1) |
| clickless-config | [0.1.1](https://crates.io/crates/clickless-config/0.1.1) |
| clickless-output-enigo | [0.1.1](https://crates.io/crates/clickless-output-enigo/0.1.1) |
| clickless-linux | [0.1.1](https://crates.io/crates/clickless-linux/0.1.1) |
| clickless-macos | [0.1.1](https://crates.io/crates/clickless-macos/0.1.1) |
| clickless-windows | [0.1.1](https://crates.io/crates/clickless-windows/0.1.1) |
| clickless | [0.1.1](https://crates.io/crates/clickless/0.1.1) |

Registry API independently confirmed all eight versions available and not yanked. Earlier five 0.1.0 uploads remain untouched. No release tag, signed application bundle or complete desktop parity is claimed.

At this publication snapshot, fresh registry-install matrix is pending. The [registry-install workflow](https://github.com/smresponsibilities/clickless/actions/workflows/registry-install.yml) installs exact 0.1.1 from crates.io on Windows, Linux and macOS without a source checkout. Its job conclusions prove fresh installation and console startup; they do not prove pointer capture, native UI or permissions. Track these conclusions in the ticket handoff before closing 052.
