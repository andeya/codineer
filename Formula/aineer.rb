# Template only — the published formula lives in
# https://github.com/andeya/homebrew-aineer
# and is auto-updated by release CI.
class Aineer < Formula
  desc "Local coding-agent CLI implemented in safe Rust"
  homepage "https://github.com/andeya/aineer"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/andeya/aineer/releases/latest/download/aineer-#{version}-aarch64-apple-darwin.tar.gz"
    else
      url "https://github.com/andeya/aineer/releases/latest/download/aineer-#{version}-x86_64-apple-darwin.tar.gz"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/andeya/aineer/releases/latest/download/aineer-#{version}-aarch64-unknown-linux-gnu.tar.gz"
    else
      url "https://github.com/andeya/aineer/releases/latest/download/aineer-#{version}-x86_64-unknown-linux-gnu.tar.gz"
    end
  end

  def install
    bin.install "aineer"
  end

  test do
    assert_match "Aineer CLI", shell_output("#{bin}/aineer --version")
  end
end
