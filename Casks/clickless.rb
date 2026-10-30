cask "clickless" do
  version "0.1.3"
  sha256 "abcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"

  url "https://github.com/smresponsibilities/clickless/releases/download/v#{version}/Clickless-macos.zip"
  name "Clickless"
  desc "Keyboard-driven pointer controller"
  homepage "https://github.com/smresponsibilities/clickless"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :big_sur"

  app "Clickless.app"
  binary "#{appdir}/Clickless.app/Contents/MacOS/clicklessctl"
  binary "#{appdir}/Clickless.app/Contents/MacOS/Clickless",
         target: "clickless"
  binary "#{appdir}/Clickless.app/Contents/MacOS/clickless-settings"

  postflight do
    system_command "/usr/bin/xattr",
                   args:         ["-dr", "com.apple.quarantine", "#{appdir}/Clickless.app"],
                   must_succeed: false,
                   sudo:         false
  end

  uninstall quit: "org.smresponsibilities.clickless"

  zap trash: [
    "~/.config/clickless",
    "~/Library/Application Support/Clickless",
    "~/Library/Caches/org.smresponsibilities.clickless",
    "~/Library/Preferences/org.smresponsibilities.clickless.plist",
  ]

  caveats <<~EOS
    Clickless requires macOS Accessibility permissions.
    Enable access under:
      System Settings -> Privacy & Security -> Accessibility -> Clickless

    Because Clickless is distributed as $0 open source software without Apple Developer ID notarization,
    if macOS Gatekeeper blocks opening, clear quarantine with:
      xattr -d com.apple.quarantine /Applications/Clickless.app
    Or install with:
      brew install --cask --no-quarantine smresponsibilities/tap/clickless
  EOS
end
