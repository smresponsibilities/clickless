# macOS Homebrew Distribution ($0 Open Source Path)

Status correction from the resumed audit: Homebrew installation is not available. The public repository has no release archives. Checked-in Cask and Formula checksums are placeholders, so both packages are disabled. Publishing archives and running the update script with both verified hashes removes this release-specific disable marker. Native macOS installation and update checks remain unperformed.

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
|    Update the existing smresponsibilities/clickless repository     |
+--------------------------------------------------------------------+
                                  |
                                  v
+--------------------------------------------------------------------+
| 4. User Installation                                               |
|    brew tap smresponsibilities/clickless https://github.com/smresponsibilities/clickless                                 |
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
     `brew install --cask --no-quarantine smresponsibilities/clickless/clickless`
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

## Primary tap and optional template

The existing Clickless repository contains the Cask and Formula used by installations:

```bash
brew tap smresponsibilities/clickless https://github.com/smresponsibilities/clickless
```

No separate repository is required. `packaging/homebrew-tap/` is an optional standalone template. If maintained separately, its repository name determines its tap name. Update its files explicitly with `--tap-dir` or `-TapDir`; the primary release procedure below also synchronizes the checked-in template.

## Packaging and release runbook

These steps require release approval and completed native acceptance. Current audit approval covers branch updates only.

### 1. Download the verified universal candidate

Choose a successful CI run whose full commit SHA matches the intended release. All matrix jobs and the universal-macos job must pass.

```bash
gh run view RUN_ID --repo smresponsibilities/clickless --json headSha,status,conclusion
gh run download RUN_ID --repo smresponsibilities/clickless --name clickless-macos-universal --dir .release-candidate
(cd .release-candidate && shasum -a 256 -c Clickless-macos.zip.sha256)
```

Replace RUN_ID with that run's ID. Use a fresh destination directory. The candidate contains `Clickless-macos.zip` and its checksum. Do not substitute a native matrix artifact or the output of a local host-only build. Complete physical permissions, input and install/update acceptance before release.

### 2. Publish the approved candidate

Tag the verified commit with the workspace version, such as `v0.1.3`. Create its GitHub release and upload the verified universal ZIP and checksum. Obtain and hash the actual tagged source archive used by the Formula. A branch archive or a hash from another tag cannot supply that checksum.

### 3. Update the primary repository and template

Both verified hashes are required before either package changes. Replace SOURCE_ARCHIVE_SHA256 with the tagged source archive's SHA256.

```bash
bash scripts/update-homebrew.sh --version 0.1.3 --zip .release-candidate/Clickless-macos.zip --formula-sha256 SOURCE_ARCHIVE_SHA256 --tap-dir packaging/homebrew-tap
```

PowerShell equivalent:

```powershell
./scripts/update-homebrew.ps1 -Version 0.1.3 -ZipPath .release-candidate/Clickless-macos.zip -FormulaSha256 SOURCE_ARCHIVE_SHA256 -TapDir packaging/homebrew-tap
```

Review all four metadata files, verify a fresh Homebrew install and upgrade, then commit and push them in the primary Clickless repository. A separately hosted optional tap needs its own reviewed synchronization. The scripts remove the release-specific disable marker only after validating release inputs.

---

## End-User Installation Guide

### Option 1: Pre-built App Bundle via Cask

```bash
# Add the tap
brew tap smresponsibilities/clickless https://github.com/smresponsibilities/clickless

# Install Clickless
brew install --cask clickless
```

If macOS displays a warning that the developer cannot be verified:

```bash
xattr -d com.apple.quarantine /Applications/Clickless.app
```

Or install directly bypassing quarantine:

```bash
brew install --cask --no-quarantine smresponsibilities/clickless/clickless
```

### Option 2: Build from Source via Formula

For users with Rust toolchains who prefer building from source:

```bash
brew install --formula smresponsibilities/clickless/clickless
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
brew uninstall --zap --cask clickless
```


## Universal release candidate

CI retains each native macOS archive after its runner passes. The universal-macos job waits for the complete Windows/Linux/macOS matrix, verifies both archive checksums and matching bundle resources, then combines arm64 and x86_64 slices for every executable. It signs the combined bundle again, extracts the final ZIP, verifies signatures and both slices, and retains the clickless-macos-universal artifact.

Only that artifact supplies the single Clickless-macos.zip release URL used by the Cask. Native matrix artifacts contain one architecture and must not be published at that URL. Download the universal artifact and its checksum from the successful run for the intended commit. Native permissions and install/update acceptance remain separate release gates. This workflow stages candidates; it does not publish releases or enable Homebrew packages.
