# Maintainer: Thomas Q <thomasqsa@gmail.com>
pkgname=anibuddy
pkgver=0.1.1
pkgrel=1
pkgdesc="An overlay for animated gifs and apngs for the wayland desktop"
arch=('x86_64')
url="https://github.com/thmasq/anibuddy"
license=('GPL3')
depends=(
    'wayland'
    'vulkan-icd-loader'
    'gcc-libs'
    'glibc'
)
makedepends=(
    'rust'
    'cargo'
    'git'
)
optdepends=(
    'vulkan-intel: Intel GPU support'
    'vulkan-radeon: AMD GPU support' 
    'nvidia-utils: NVIDIA GPU support'
)
install="$pkgname.install"
source=("git+https://github.com/thmasq/$pkgname.git#tag=v$pkgver")
sha256sums=('SKIP')

prepare() {
    cd "$pkgname"
    
    # Update Cargo.lock if needed
    cargo fetch --locked --target "$CARCH-unknown-linux-gnu"
}

build() {
    cd "$pkgname"
    
    # Set Rust environment variables for optimization
    export RUSTUP_TOOLCHAIN=stable
    export CARGO_TARGET_DIR=target
    
    # Build with release optimizations
    cargo build --frozen --release --all-features
}

package() {
    cd "$pkgname"
    
    # Install binary
    install -Dm755 "target/release/$pkgname" "$pkgdir/usr/bin/$pkgname"
    
    # Create config directory structure
    install -dm755 "$pkgdir/usr/share/doc/$pkgname"
    
    # Install documentation if README exists
    if [ -f README.md ]; then
        install -Dm644 README.md "$pkgdir/usr/share/doc/$pkgname/README.md"
    fi
    
    # Install example config
    if [ -f config.toml.example ]; then
        install -Dm644 config.toml.example "$pkgdir/usr/share/doc/$pkgname/config.toml.example"
    else
        # Create a basic example config
        cat > "$pkgdir/usr/share/doc/$pkgname/config.toml.example" << 'EOF'
# Example configuration for anibuddy
# Place this file at ~/.config/anibuddy/config.toml

[default]
path = "/path/to/your/default/animation"
fps = 30

[example]
path = "/path/to/animation.gif"
fps = 24
EOF
    fi
    
    # Install license if it exists
    if [ -f LICENSE ]; then
        install -Dm644 LICENSE "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    elif [ -f LICENSE-MIT ]; then
        install -Dm644 LICENSE-MIT "$pkgdir/usr/share/licenses/$pkgname/LICENSE"
    fi
}

