name := 'cosmic-ext-applet-pomodoro'
appid := 'com.github.bgub.CosmicExtAppletPomodoro'

rootdir := ''
prefix := '/usr'

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name

metainfo-dst := base-dir / 'share' / 'metainfo' / appid + '.metainfo.xml'
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / appid + '.desktop'
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'

# User-local installation paths (~/.local)
user-base := env('HOME') / '.local'
user-bin-dst := user-base / 'bin' / name
user-desktop-dst := user-base / 'share' / 'applications' / appid + '.desktop'
user-metainfo-dst := user-base / 'share' / 'metainfo' / appid + '.metainfo.xml'
user-icon-dst := user-base / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features {{args}} -- -W clippy::pedantic

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Installs files
install: build-release
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 resources/app.desktop {{desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{metainfo-dst}}
    install -Dm0644 resources/icon.svg {{icon-dst}}

# Installs to ~/.local (no root needed, works on NixOS)
install-user: (build-release)
    install -Dm0755 {{bin-src}} {{user-bin-dst}}
    install -Dm0644 resources/app.desktop {{user-desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{user-metainfo-dst}}
    install -Dm0644 resources/icon.svg {{user-icon-dst}}

# Symlinks build output into ~/.local for iterative development.
# After running once, `just dev-reload` rebuilds and restarts the panel.
dev-install: (build-release)
    mkdir -p {{user-base}}/bin
    ln -sf {{absolute_path(bin-src)}} {{user-bin-dst}}
    install -Dm0644 resources/app.desktop {{user-desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{user-metainfo-dst}}
    install -Dm0644 resources/icon.svg {{user-icon-dst}}

# Rebuild and restart cosmic-panel to reload applets
dev-reload: (build-release)
    pkill cosmic-panel || true

# Uninstalls files
uninstall:
    rm {{bin-dst}} {{desktop-dst}} {{icon-dst}}

# Uninstalls user-local files
uninstall-user:
    rm -f {{user-bin-dst}} {{user-desktop-dst}} {{user-metainfo-dst}} {{user-icon-dst}}

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''
