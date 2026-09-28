# Wheel — Dependency Versions

Date checked: 2026-09-28

## JavaScript / Node

| Package | Version | Notes |
|---------|---------|-------|
| Node.js | 22.19.0 (LTS) | via system |
| pnpm | 10.21.0 | (12.6.0 available) |
| react | 19.3.0 | |
| react-dom | 19.3.0 | |
| @tauri-apps/api | 2.12.0 | pinned to match Rust |
| @tauri-apps/cli | 2.12.0 | pinned to match Rust |
| @tauri-apps/plugin-opener | 2.6.0 | |
| motion | 13.4.4 | (formerly framer-motion) |
| zustand | 5.0.15 | |
| @radix-ui/react-dialog | 1.1.23 | |
| @radix-ui/react-slot | 1.3.3 | |
| tailwindcss | 4.3.3 | v4 Vite plugin |
| @tailwindcss/vite | 4.3.3 | |
| vite | 8.3.1 | |
| typescript | 6.0.3 | (7.0.2 available) |
| @vitejs/plugin-react | 6.1.1 | |

## Rust

| Crate | Version | Notes |
|-------|---------|-------|
| rustc | 1.97.0 | stable-x86_64-pc-windows-msvc |
| tauri | 2.12.0 | |
| tauri-build | 2.7.0 | |
| tauri-plugin-opener | 2.6.0 | |
| tauri-plugin-single-instance | 2.x | |
| tauri-plugin-autostart | 2.6.0 | |
| tauri-plugin-notification | 2.5.0 | |
| tauri-plugin-store | 2.5.0 | |
| tauri-plugin-window-state | 2.5.0 | |
| tauri-plugin-global-shortcut | 2.4.0 | |
| tauri-plugin-process | 2.4.0 | |
| windows | 0.61.3 | Win32 APIs |
| window-vibrancy | 0.6.0 | pinned (0.8.1 also available) |
| tokio | 1.53.1 | |
| serde | 1.0.229 | |
| tracing | 0.1.44 | |
| image | 0.25.x | |
| uuid | 1.26.1 | |
| chrono | 0.4.45 | |
| lopdf | 0.45.0 | PDF generation (embed_image) |
| anyhow | 1.0.104 | |

## Update Commands

```sh
# Check outdated JS packages
pnpm --filter desktop outdated

# Update JS packages
pnpm --filter desktop update --latest

# Check outdated Rust crates
cargo outdated  # requires: cargo install cargo-outdated

# Update Rust crates
cargo update
```

## Renovate / Dependabot

Renovate config at `.github/renovate.json` (created in M7):
- Weekly updates for npm packages
- Weekly updates for Cargo dependencies
- Grouped Tauri updates (all must match versions)
