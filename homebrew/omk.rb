class Omk < Formula
  desc "Multi-agent orchestration for Kimi CLI"
  homepage "https://github.com/ekhodzitsky/oh-my-kimi"
  version "0.1.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_AARCH64_MACOS"
    end
    on_intel do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_MACOS"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_X86_64_LINUX"
    end
  end

  def install
    bin.install "omk"
    
    # Install shell completions
    bash_completion.install "omk completions bash" => "omk" if build.with? "completions"
    zsh_completion.install "omk completions zsh" => "_omk" if build.with? "completions"
    fish_completion.install "omk completions fish" => "omk.fish" if build.with? "completions"
    
    # Install man page
    man1.install "omk man" => "omk.1" if build.with? "man"
  end

  test do
    system "#{bin}/omk", "--version"
  end
end
