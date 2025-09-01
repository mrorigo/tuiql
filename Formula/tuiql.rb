class Tuiql < Formula
  desc "A blazing-fast, terminal-native, keyboard-centric SQLite client"
  homepage "https://github.com/tuiql/tuiql"
  version "0.1.0"

  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/tuiql/tuiql/releases/download/v#{version}/tuiql-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_AARCH64_MACOS"
    else
      url "https://github.com/tuiql/tuiql/releases/download/v#{version}/tuiql-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "REPLACE_WITH_ACTUAL_SHA256_X86_64_MACOS"
    end
  elsif OS.linux?
    url "https://github.com/tuiql/tuiql/releases/download/v#{version}/tuiql-v#{version}-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "REPLACE_WITH_ACTUAL_SHA256_LINUX_X86_64"
  end

  def install
    bin.install "tuiql"
  end

  test do
    # Create a test database
    test_db = testpath/"test.db"
    system "sqlite3", test_db, "CREATE TABLE users (id INTEGER, name TEXT);"
    system "sqlite3", test_db, "INSERT INTO users VALUES (1, 'Alice');"

    # Verify the binary exists and is executable
    assert_predicate bin/"tuiql", :exist?
    assert_predicate bin/"tuiql", :executable?
  end
end