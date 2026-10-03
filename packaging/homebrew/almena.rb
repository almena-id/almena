# Homebrew formula of the almena CLI, published in almena-id/homebrew-tap as
# Formula/almena.rb. This is its template: the release workflow fills in
# @VERSION@ and the checksum of each archive, and nothing else changes it.
# It installs the signed binary of the GitHub release untouched.
#
# Only macOS on Apple silicon and Linux on x86_64 have a binary. Homebrew reads
# a formula on every platform and wants a URL on each, so the macOS archive is
# the default URL and the arch dependency refuses every other combination.

class Almena < Formula
  desc "Command-line client of the Almena ID platform"
  homepage "https://github.com/almena-id/almena"
  url "https://github.com/almena-id/almena/releases/download/v@VERSION@/almena-@VERSION@-aarch64-apple-darwin.tar.gz"
  sha256 "@SHA256_AARCH64_APPLE_DARWIN@"
  license "Apache-2.0"

  on_macos do
    depends_on arch: :arm64
  end

  on_linux do
    depends_on arch: :x86_64

    on_intel do
      url "https://github.com/almena-id/almena/releases/download/v@VERSION@/almena-@VERSION@-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "@SHA256_X86_64_UNKNOWN_LINUX_GNU@"
    end
  end

  def install
    bin.install "almena"
    generate_completions_from_executable(bin/"almena", "completions")
  end

  test do
    assert_equal "almena #{version}", shell_output("#{bin}/almena --version").strip
  end
end
