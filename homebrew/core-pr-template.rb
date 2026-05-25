class Omk < Formula
  desc "Multi-agent orchestration for Kimi CLI"
  homepage "https://github.com/ekhodzitsky/oh-my-kimi"
  version "0.5.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "9941d27c86cc9ee166d8ff20415ae0d0a4ff432837df282dbb2e36baafe2bf9e"
    end
    on_intel do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "8db70218460a8353e15cd52bf2e2d857167493ff4eb3765e592a9c68a1ac6aca"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/ekhodzitsky/oh-my-kimi/releases/download/v#{version}/omk-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "9c7639294b85f20a3d7507ce696c20fded2cec6c8c149df129b1dc02b09148a5"
    end
  end

  def install
    bin.install "omk"

    bash_completion.install Utils.safe_popen_read(bin/"omk", "completions", "bash") => "omk"
    zsh_completion.install Utils.safe_popen_read(bin/"omk", "completions", "zsh") => "_omk"
    fish_completion.install Utils.safe_popen_read(bin/"omk", "completions", "fish") => "omk.fish"

    man1.install Utils.safe_popen_read(bin/"omk", "man") => "omk.1"
  end

  test do
    system bin/"omk", "--version"
  end
end
