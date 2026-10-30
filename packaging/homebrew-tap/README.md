# Homebrew Tap for Clickless

Homebrew formulas and casks for Clickless, a keyboard-driven pointer controller written in Rust.

## Install via Cask (Recommended)

Installs the pre-built application bundle (`Clickless.app`) and links `clicklessctl`, `clickless`, and `clickless-settings` into your path:

```bash
brew tap smresponsibilities/tap
brew install --cask clickless
```

### Gatekeeper and Accessibility ($0 Open Source Distribution)

Clickless is distributed as $0 open source software without Apple Developer Program notarization fees ($99/year). Application bundles are ad-hoc signed (`codesign -s -`) to satisfy Apple Silicon kernel execution requirements.

When opening Clickless for the first time:

1. **Accessibility permissions**: Clickless requires Accessibility access to control the cursor. Grant permission in:
   `System Settings` -> `Privacy & Security` -> `Accessibility` -> enable `Clickless`.
2. **Gatekeeper quarantine**: If macOS displays an untrusted developer warning, clear the quarantine attribute:
   ```bash
   xattr -d com.apple.quarantine /Applications/Clickless.app
   ```
   Or install with `--no-quarantine`:
   ```bash
   brew install --cask --no-quarantine smresponsibilities/tap/clickless
   ```

## Install via Formula (Build from Source)

Builds the CLI binaries from source using Cargo:

```bash
brew install --formula smresponsibilities/tap/clickless
```

## Update

```bash
brew update
brew upgrade --cask clickless
```

## Uninstall

```bash
brew uninstall --cask clickless
```

To remove configuration files and caches:

```bash
brew zap clickless
```
