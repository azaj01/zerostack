class Zerostack < Formula
  desc "Minimalistic coding agent written in Rust, optimized for memory footprint and performance"
  homepage "https://github.com/gi-dellav/zerostack"
  version "1.6.1"
  license "GPL-3.0-only"

  on_macos do
    if Hardware::CPU.intel?
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.6.1/zerostack-x86_64-apple-darwin.tar.gz"
      sha256 "fdd814adeb5988f40106eaed7720c53bd4e0d2cf26d9b0b48e51cda573a6faa1"
    else
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.6.1/zerostack-aarch64-apple-darwin.tar.gz"
      sha256 "196595df1b9e3ad19fe54ce891606634ab0fab4e900d12e92078642471e77fb9"
    end
  end

  on_linux do
    if Hardware::CPU.intel?
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.6.1/zerostack-x86_64-unknown-linux-musl.tar.gz"
      sha256 "854232371f44f1493c9b5f0046656a9c4f1a9376b2c7a050f5555657708d5ec3"
    else
      url "https://github.com/gi-dellav/zerostack/releases/download/v1.6.1/zerostack-aarch64-unknown-linux-musl.tar.gz"
      sha256 "d23fd7e51d582773a7cae4804767c547edcc69ea04e3f545d53210811b474237"
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
