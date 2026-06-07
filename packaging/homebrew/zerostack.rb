class Zerostack < Formula
  desc "Minimalistic coding agent written in Rust, optimized for memory footprint and performance"
  homepage "https://github.com/gi-dellav/zerostack"
  version "1.4.5"
  license "GPL-3.0-only"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.4.5/zerostack-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.4.5/zerostack-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.4.5/zerostack-x86_64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    else
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.4.5/zerostack-aarch64-unknown-linux-musl.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    # darwin tarballs contain "zerostack", musl tarballs contain "zerostack-<target>"
    bin.install Dir["zerostack*"].first => "zerostack"
  end

  test do
    assert_match(/^zerostack /, shell_output("#{bin}/zerostack --version"))
  end
end
