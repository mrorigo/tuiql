# TUIQL AUR Package Documentation

This document provides information for maintaining the AUR (Arch User Repository) package for TUIQL.

## AUR Package Structure

### Package: `tuiql-bin`

This is a binary AUR package that downloads pre-compiled binaries from GitHub releases.

#### PKGBUILD Structure

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=tuiql-bin
pkgver=0.1.0
_pkgver=${pkgver}
pkgrel=1
pkgdesc="A blazing-fast, terminal-native SQLite client"
arch=('x86_64' 'aarch64')
url="https://github.com/tuiql/tuiql"
license=('MIT')
depends=('glibc')
optdepends=('sqlite: for additional SQLite extensions')
provides=('tuiql')
conflicts=('tuiql')

source_x86_64=("$pkgname-$pkgver-x86_64-unknown-linux-gnu.tar.gz::${url}/releases/download/v${_pkgver}/${pkgname}-v${_pkgver}-x86_64-unknown-linux-gnu.tar.gz")
source_aarch64=("$pkgname-$pkgver-aarch64-unknown-linux-gnu.tar.gz::${url}/releases/download/v${_pkgver}/${pkgname}-v${_pkgver}-aarch64-unknown-linux-gnu.tar.gz")

sha256sums_x86_64=('REPLACE_WITH_ACTUAL_SHA256_X86_64')
sha256sums_aarch64=('REPLACE_WITH_ACTUAL_SHA256_AARCH64')

package() {
    install -Dm755 "tuiql" "${pkgdir}/usr/bin/tuiql"
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
```

### Alternative: `tuiql`

Source-based AUR package that compiles from git.

```bash
# Maintainer: Your Name <your.email@example.com>
pkgname=tuiql
pkgver=0.1.0
pkgrel=1
pkgdesc="A blazing-fast, terminal-native SQLite client"
arch=('x86_64' 'aarch64')
url="https://github.com/tuiql/tuiql"
license=('MIT')
depends=('glibc')
makedepends=('rust' 'cargo')
source=("$pkgname-$pkgver.tar.gz::${url}/archive/refs/tags/v${pkgver}.tar.gz")
sha256sums=('REPLACE_WITH_ACTUAL_SHA256')

prepare() {
    cd "$srcdir/$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$srcdir/$pkgname-$pkgver"
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    cargo build --release --frozen --locked
}

package() {
    cd "$srcdir/$pkgname-$pkgver"
    install -Dm755 "target/release/tuiql" "${pkgdir}/usr/bin/tuiql"
    install -Dm644 "LICENSE" "${pkgdir}/usr/share/licenses/${pkgname}/LICENSE"
}
```

## AUR Package Maintenance

### Updating the Package

1. Bump the `pkgver` in both PKGBUILD files
2. Update the `sha256sums` with the new source hashes
3. Update the release archive hashes
4. Commit and push changes to AUR

### Testing the Package

```bash
# Test compilation
makepkg -c

# Test installation
makepkg -i

# Verify functionality
tuiql --help
```

## Automated Release Process

When a new GitHub release is published:

1. GitHub Actions automatically builds binaries for x86_64 and aarch64
2. Automated process should update PKGBUILD files and submit to AUR
3. AUR users get notified of updates

## Package Information

- **License**: MIT
- **Dependencies**: glibc (statically linked SQLite)
- **Optional Dependencies**: sqlite (for additional extensions if needed)
- **Provides**: tuiql
- **Conflicts**: tuiql
- **Architecture**: x86_64, aarch64