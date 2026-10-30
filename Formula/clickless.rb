class Clickless < Formula
  desc "Keyboard-driven pointer controller in Rust"
  homepage "https://github.com/smresponsibilities/clickless"
  url "https://github.com/smresponsibilities/clickless/archive/refs/tags/v0.1.3.tar.gz"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/smresponsibilities/clickless.git", branch: "main"

  livecheck do
    url :stable
    strategy :github_latest
  end

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/clickless-cli")
  end

  test do
    assert_match "clickless", shell_output("#{bin}/clicklessctl --version")
  end
end
