# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

A pomodoro timer applet for the COSMIC desktop. This repo also serves as a **reference implementation** for building COSMIC applets — patterns here apply to new applets too.

## Build Commands

All commands use `just`. On NixOS, prefix with `direnv exec .` (or enter the direnv shell) since the toolchain comes from nix:

```sh
direnv exec . just          # build release
direnv exec . just check    # clippy with pedantic warnings
direnv exec . just dev-reload  # rebuild + restart cosmic-panel
```

Other targets:
- `just run` — build and run standalone (Wayland errors are normal — applets need the panel)
- `just dev-install` — one-time setup: symlinks binary to `~/.local/bin`, copies desktop/icon/metadata
- `just install-user` — copies binary to `~/.local` (no root needed)
- `just tag <version>` — bump version, commit, and git tag

## NixOS Development

A shared nix-shell at `~/.config/nix/cosmic-shell.nix` provides native deps and linker flags. The `.envrc` activates it:

```
use nix ~/.config/nix/cosmic-shell.nix
```

Key details:
- **Always use `direnv exec .`** when running build/check/run commands outside the direnv shell — without it, `cc` linker and native libs won't be found
- `RUSTFLAGS` must force-link dlopen'd libraries (EGL, wayland, vulkan, xkbcommon, X11) — same approach as `libcosmicAppHook` in nixos-cosmic
- `LD_LIBRARY_PATH` is needed at runtime for those same libraries
- `~/.local/bin` must be in PATH (`environment.localBinInPath = true` in NixOS config)
- On NixOS, `/usr/share` is NOT in `XDG_DATA_DIRS` — use `install-user` or `dev-install` instead of `just install`

## Project Structure

```
src/
  main.rs       — entry point, i18n init, launches cosmic::applet::run::<AppModel>()
  app.rs        — core: AppModel state, Message enum, view/update/subscription
  config.rs     — Config struct with #[derive(CosmicConfigEntry)], persisted via cosmic-config
  i18n.rs       — fluent localization via i18n-embed, fl!("key") macro
resources/
  tomato.svg    — outline-only SVG using currentColor + stroke (no fill)
  pause.svg     — pause icon, same stroke style as tomato
  icon.svg      — app icon for desktop entry
  app.desktop   — desktop entry with applet keys
  app.metainfo.xml
i18n/en/
  cosmic_ext_applet_pomodoro.ftl  — English fluent strings
```

## Architecture

COSMIC applets follow an Elm-like architecture via `cosmic::Application`.

### Panel Button (`view()`)

- `view()` renders the panel button (icon/text in the top bar)
- Uses `widget::button::custom(autosize_window(content)).class(cosmic::theme::Button::AppletIcon)` for custom panel content
- Wrap in `widget::mouse_area(btn).on_right_press(Message)` to handle right-click (e.g., open popup on right-click)
- Left-click handled via `.on_press()` on the button itself

### Popup (`view_window()`)

- `view_window()` renders the popup dropdown
- Wrap content with `self.core.applet.popup_container(content)`
- Popups use `get_popup()` / `destroy_popup()` from `cosmic::iced_winit::commands::popup`

### Subscriptions

Timer/background tasks use `Subscription::run_with_id` with channel-based async. Subscriptions are active only while present in `Subscription::batch` — removing them cancels them. The pattern:

```rust
struct TimerTick;
Subscription::run_with_id(
    std::any::TypeId::of::<TimerTick>(),
    cosmic::iced::stream::channel(1, move |mut channel| async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            _ = channel.send(Message::Tick).await;
        }
    }),
)
```

### Config Persistence

- `Config` struct derives `CosmicConfigEntry` with `#[version = N]`
- Store `config_handler: Option<cosmic_config::Config>` in the model to write changes back
- Read in `init()` via `cosmic_config::Config::new(APP_ID, Config::VERSION)` then `Config::get_entry(&handler)`
- Write with `self.config.write_entry(&handler)`
- Watch for external changes via `self.core().watch_config::<Config>(APP_ID)` in `subscription()`

### Desktop Entry

The `.desktop` file must have these keys for COSMIC to recognize it as an applet:
- `X-CosmicApplet=true`
- `X-CosmicHoverPopup=Auto`
- `NoDisplay=true`
- `Categories=COSMIC`

## Widget Patterns

### Custom SVG Icons

Use outline-only SVGs with `currentColor` so they adapt to the theme:

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
     stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
  ...
</svg>
```

Embed and mark as symbolic so libcosmic recolors to match panel text color:

```rust
const MY_SVG: &[u8] = include_bytes!("../resources/icon.svg");
widget::icon(widget::icon::from_svg_bytes(MY_SVG).symbolic(true))
```

For named system icons: `widget::icon::from_name("icon-symbolic").size(18).symbolic(true).into()`

### Theme-Aware Styling

Use the `theme` parameter in style closures to read COSMIC theme settings:

```rust
.style(move |theme: &Theme| container::Style {
    border: cosmic::iced::Border {
        radius: theme.cosmic().corner_radii.radius_xl.into(), // matches AppletIcon button
        ..Default::default()
    },
    ..container::Style::default()
})
```

Key theme values:
- `theme.cosmic().corner_radii.radius_xl` — the border radius used by `Button::AppletIcon`
- `theme.cosmic().corner_radii.radius_s` / `radius_m` / `radius_l` — smaller radii
- Colors via `cosmic::iced::Color::from_rgba()`

### Spin Buttons

For numeric settings, use `widget::spin_button`. Note: with `a11y` feature (enabled by default), it takes 7 args — the second is an accessibility name:

```rust
widget::spin_button(
    format!("{value}"),    // display label
    format!("{value}"),    // a11y name
    value,                 // current value
    1,                     // step
    min,                   // min
    max,                   // max
    Message::SetValue,     // on_change (receives new value directly)
)
```

### Popup Layout Widgets

- `widget::divider::horizontal::default()` — horizontal separator line
- `widget::button::suggested("text")` / `::standard()` / `::destructive()` — styled buttons
- `widget::text::heading()` / `::title1()` — text size helpers

## Cargo Features

Minimal libcosmic features for an applet (no wgpu — uses software renderer):

```toml
[dependencies.libcosmic]
git = "https://github.com/pop-os/libcosmic.git"
features = ["applet", "applet-token", "dbus-config", "multi-window", "tokio", "wayland", "winit"]
```

Only pull the tokio features you need (avoid `"full"`):

```toml
tokio = { version = "1", features = ["time"] }
```

## Publishing

### GitHub

- **Repo**: https://github.com/bgub/cosmic-ext-applet-pomodoro
- Releases use `just tag <version>` then `git push origin main --tags`
- Note: `just tag` fails if the version is already current — in that case just `git tag -a <version> -m 'release: <version>'` and push manually

### NixOS (nixpkgs)

- **PR**: https://github.com/NixOS/nixpkgs/pull/495307
- COSMIC is a builtin in nixpkgs (`services.desktopManager.cosmic.enable = true`), NOT using the nixos-cosmic flake
- Package lives at `pkgs/by-name/co/cosmic-ext-applet-pomodoro/package.nix` in nixpkgs
- A local copy is at `package.nix` in this repo for reference
- Uses `rustPlatform.buildRustPackage` + `libcosmicAppHook` + `just` (same pattern as other `cosmic-ext-applet-*` packages)
- Key nix build details: `dontUseJustBuild = true`, `dontUseJustCheck = true` — cargo builds, just only installs
- `justFlags` must override `prefix` and `bin-src` (with cross-compilation-aware target triple)
- Maintainer entry for `bgub` added to `maintainers/maintainer-list.nix`
- When updating: bump version/hashes in nixpkgs, use `nix-update` or manually update `tag`, `hash`, and `cargoHash`

### Flatpak (cosmic-flatpak)

- **PR**: https://github.com/pop-os/cosmic-flatpak/pull/116
- Flathub rejects panel applets ("desktop environment extensions") — COSMIC applets go through `pop-os/cosmic-flatpak` instead
- Manifest at `app/com.github.bgub.CosmicExtAppletPomodoro/com.github.bgub.CosmicExtAppletPomodoro.json`
- Uses `com.system76.Cosmic.BaseApp` (provides `just`, COSMIC icons) on `org.freedesktop.Platform` 24.08
- Rust toolchain via `org.freedesktop.Sdk.Extension.rust-stable`
- `cargo-sources.json` generated with `flatpak-cargo-generator.py` from Cargo.lock (8800+ lines, vendoring all deps for offline build)
- Build uses `just build-release --verbose --offline` then `just prefix=/app install`
- When updating: regenerate `cargo-sources.json` from new Cargo.lock, update tag in manifest
