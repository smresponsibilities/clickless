# macOS Homebrew Distribution ($0 Open Source Path)

This document describes the Homebrew packaging, signing, and distribution workflow for Clickless on macOS.

## Distribution Architecture

Clickless adopts a zero-fee open source distribution path on macOS, avoiding the annual Apple Developer Program fee ($99/year).

```
+--------------------------------------------------------------------+
| 1. Build & Ad-hoc Sign                                             |
|    cargo build --release -p clickless --features gpui-settings     |
|    scripts/bundle-macos.sh -> target/release/Clickless.app         |
|    codesign --force -s - Clickless.app                             |
+--------------------------------------------------------------------+
                                  |
                                  v
+--------------------------------------------------------------------+
| 2. Distribution Archive & SHA256 Checksum                          |
|    ditto -c -k --keepParent Clickless.app Clickless-macos.zip      |
|    shasum -a 256 Clickless-macos.zip > Clickless-macos.zip.sha256  |
+--------------------------------------------------------------------+
                                  |
                                  v
+--------------------------------------------------------------------+
| 3. GitHub Releases & Homebrew Tap Synchronization                  |
|    Upload Clickless-macos.zip to GitHub Release tag                |
|    Update Casks/clickless.rb (version & sha256)                    |
|    Commit & push to github.com/smresponsibilities/homebrew-tap     |
+--------------------------------------------------------------------+
                                  |
                                  v
+--------------------------------------------------------------------+
| 4. User Installation                                               |
|    brew tap smresponsibilities/tap                                 |
|    brew install --cask clickless                                   |
|    Clear quarantine: xattr -d com.apple.quarantine ...             |
|    Grant Accessibility permission in System Settings               |
+--------------------------------------------------------------------+
```

### Technical Mechanisms

1. **Kernel Execution (Apple Silicon / arm64)**:
   macOS kernels on Apple Silicon require all executable binaries to have a code signature. Without a signature, execution fails with `SIGKILL (COREDUMP)`. Ad-hoc code signing (`codesign --force -s -`) satisfies this kernel requirement without an Apple-issued certificate.

2. **Accessibility (TCC) Permission Stability**:
   macOS tracks accessibility permissions via designated code requirements. Ad-hoc signing provides a stable binary hash requirement so accessibility permissions persist between launches of the same build.

3. **Gatekeeper Quarantine Bypass**:
   Downloaded archives receive the `com.apple.quarantine` extended attribute.
   - Option A: Install via Homebrew with `--no-quarantine`:
     `brew install --cask --no-quarantine smresponsibilities/tap/clickless`
   - Option B: Remove the quarantine flag post-install:
     `xattr -d com.apple.quarantine /Applications/Clickless.app`
   - The cask includes postflight cleanup and terminal caveats instructing users on both options.

---

## File Layout

| Path | Purpose |
|---|---|
| `Casks/clickless.rb` | Homebrew Cask formula for `Clickless.app` |
| `Formula/clickless.rb` | Homebrew Formula for building CLI binaries from source |
| `scripts/bundle-macos.sh` | Compiles binaries and constructs the `Clickless.app` bundle |
| `scripts/package-macos.sh` | Ad-hoc signs bundle, creates `.zip` archive, and generates `.sha256` |
| `scripts/update-homebrew.sh` | Updates version and SHA256 in Casks and Formula |
| `scripts/update-homebrew.ps1` | PowerShell equivalent for updating formulas from Windows |
| `scripts/test-package-macos.sh` | Deterministic unit tests for the packaging pipeline |
| `packaging/homebrew-tap/` | Standalone template for `github.com/smresponsibilities/homebrew-tap` |

---

## Setting Up the Tap Repository

To initialize the public tap repository:

1. Create a public repository named `homebrew-tap` under the `smresponsibilities` organization:
   `https://github.com/smresponsibilities/homebrew-tap`
2. Initialize with the contents of `packaging/homebrew-tap/`:
   ```bash
   cd packaging/homebrew-tap
   git init -b main
   git remote add origin https://github.com/smresponsibilities/homebrew-tap.git
   git add .
   git commit -m "feat: initialize Clickless Homebrew tap"
   git push -u origin main
   ```
3. Once pushed, users can tap and install via:
   ```bash
   brew tap smresponsibilities/tap
   brew install --cask clickless
   ```

---

## Packaging and Release Runbook

### Step 1: Package Release Bundle

On macOS:

```bash
bash scripts/package-macos.sh
```

This creates:
- `target/release/Clickless.app` (ad-hoc signed)
- `target/release/Clickless-macos.zip`
- `target/release/Clickless-macos.zip.sha256`

### Step 2: Publish to GitHub Releases

1. Create a release tag matching the workspace version (e.g., `v0.1.3`):
   ```bash
   git tag v0.1.3
   git push origin v0.1.3
   ```
2. Upload `target/release/Clickless-macos.zip` as a release asset:
   `https://github.com/smresponsibilities/clickless/releases/download/v0.1.3/Clickless-macos.zip`

### Step 3: Update Cask and Formula

Run the automated updater:

```bash
# On macOS / Linux (updates Cask from zip.sha256, optionally updates Formula SHA256)
bash scripts/update-homebrew.sh --version 0.1.3 --formula-sha256 <source_tar_sha256> --tap-dir path/to/homebrew-tap

# On Windows
powershell -ExecutionPolicy Bypass -File scripts/update-homebrew.ps1 -Version 0.1.3 -FormulaSha256 <source_tar_sha256> -TapDir path/to/homebrew-tap
```

Commit and push the updated Cask in `homebrew-tap`:

```bash
cd path/to/homebrew-tap
git add Casks/clickless.rb Formula/clickless.rb
git commit -m "feat(cask): update clickless to v0.1.3"
git push
```

---

## End-User Installation Guide

### Option 1: Pre-built App Bundle via Cask

```bash
# Add the tap
brew tap smresponsibilities/tap

# Install Clickless
brew install --cask clickless
```

If macOS displays a warning that the developer cannot be verified:

```bash
xattr -d com.apple.quarantine /Applications/Clickless.app
```

Or install directly bypassing quarantine:

```bash
brew install --cask --no-quarantine smresponsibilities/tap/clickless
```

### Option 2: Build from Source via Formula

For users with Rust toolchains who prefer building from source:

```bash
brew install --formula smresponsibilities/tap/clickless
```

### Post-Install Configuration

1. Open `System Settings` -> `Privacy & Security` -> `Accessibility`.
2. Enable `Clickless` in the allowed applications list.
3. Launch Clickless from Applications or run `clickless` in terminal.
4. Verify CLI tools:
   ```bash
   clicklessctl --version
   clicklessctl --help
   ```

### Upgrades

```bash
brew update
brew upgrade --cask clickless
```

### Uninstallation

```bash
brew uninstall --cask clickless

# To remove configuration and state:
brew zap clickless
```
