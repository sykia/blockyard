# Blockyard

Blockyard is a Tauri 2 desktop launcher for Minecraft Java Edition. It keeps game directories independent while sharing downloaded game files, libraries, assets and Java runtimes.

## Screenshots

Screenshots use a separate sample offline profile. No personal account data is included.

![Launcher overview with Vanilla and Fabric instances](docs/screenshots/overview.png)

![Instance settings](docs/screenshots/instances.png)

![Offline accounts](docs/screenshots/accounts.png)

![Launcher settings](docs/screenshots/settings.png)

## Build and run

Requirements: Rust stable, Node.js 20+, npm, and the [Tauri 2 system prerequisites](https://v2.tauri.app/start/prerequisites/). On Arch Linux:

```bash
sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator librsvg xdotool
```

```bash
npm install
npm run tauri dev
npm run build
npm run tauri build
```

On Linux, the launcher disables WebKitGTK's DMA-BUF renderer to avoid a blank window on affected graphics drivers. If XWayland is available in a Wayland session, it also selects GTK's X11 backend. Explicit `WEBKIT_DISABLE_DMABUF_RENDERER` and `GDK_BACKEND` environment values take precedence. If your compositor does not provide XWayland, start with `GDK_BACKEND=wayland`.

To apply the same workaround when launching an older build that still shows a blank window, use:

```bash
GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 npm run tauri dev
```

### Hyprland window placement

On Hyprland, Blockyard asks the compositor to float and center its main window at startup. This works in packaged builds too and does not edit the user's Hyprland configuration. It uses `hyprctl` with the Lua dispatcher syntax on Hyprland 0.55+ and falls back to the legacy syntax on older releases. The initial 1180×760 size comes from `src-tauri/tauri.conf.json`.

The app data directory is chosen by Tauri (`app.blockyard.launcher`). It contains `state.json`, `instances/<id>/game`, a shared `libraries` directory, `assets`, `cache`, and managed `runtimes`. The operating system keyring stores Microsoft refresh and Minecraft access tokens. Do not copy an instance's `game` directory while Minecraft is running.

## Account setup

**Offline profiles:** Open Accounts → Add offline profile and choose a 3–16 character Minecraft player name. The launcher derives Minecraft's stable offline UUID from that name. Offline profiles need no Microsoft client ID and work for singleplayer and servers that permit offline players. They do not prove Minecraft ownership and cannot join online-mode servers. Changing the name creates a different UUID, so existing worlds may treat it as a different player.

**Microsoft accounts:** Blockyard does not ship a Microsoft application ID. Register your own public desktop application in Microsoft Entra, allow personal Microsoft accounts and public client/device code flow, then enter its **client ID** in Launcher Settings. No client secret belongs in a desktop app. The sign-in button starts Microsoft's device authorization flow, opens the verification URL in the system browser, then exchanges the Microsoft token through Xbox Live, XSTS and Minecraft Services. A Minecraft Java profile is required. Refresh tokens are kept in the system keyring, never in `state.json`.

Minecraft Services may require separate approval for the application ID. An unapproved ID can authenticate with Microsoft but fail at `login_with_xbox` with HTTP 403. This approval is external to Blockyard; do not reuse another launcher's ID. See the [Microsoft device flow documentation](https://learn.microsoft.com/entra/identity-platform/v2-oauth2-device-code) and the [Minecraft application registration discussion](https://learn.microsoft.com/en-gb/answers/questions/5984225/minecraft-services-http-403-invalid-app-registrati).

## Launch flow

1. Read Mojang's [version manifest v2](https://piston-meta.mojang.com/mc/game/version_manifest_v2.json), select a release or enabled snapshot, and download its version JSON.
2. For Fabric, read its [Meta API launcher profile](https://github.com/FabricMC/fabric-meta/blob/master/README.md) and merge its libraries and arguments with the inherited vanilla version. For NeoForge, map both legacy `1.x` and current `26.x` version schemes, download the official installer from its Maven repository, verify its published SHA-1 and run `--install-client` with a private launcher root; then read the installed profile.
3. Resolve rule filtered libraries and natives; verify SHA-1 and size where available. Download the client JAR, logging configuration, asset index and asset objects into a shared cache. Downloads run concurrently and use temporary files and retries.
4. Select Java by the version JSON's `javaVersion.majorVersion`, inspect installed Java, or download Eclipse Temurin JRE via the [Adoptium API](https://adoptium.net/installation/ci-scripts/) and verify SHA-256. An instance can override Java explicitly.
5. Extract only native library files, build arguments as a process argument vector, and launch in the instance's private game directory. The launcher streams stdout/stderr and records exit status, latest.log and crash reports.

## Architecture

`src-tauri/src/metadata.rs` parses Mojang metadata and rule based arguments. `download.rs` owns verified file acquisition. `engine.rs` assembles the launch. `auth.rs` owns the Microsoft/Xbox/Minecraft token chain; `offline.rs` creates local identities. `neoforge.rs` owns the official installer integration. `java.rs` finds or provisions runtimes. `store.rs` persists nonsecret state atomically. `hyprland.rs` handles optional floating window placement. React in `src/main.tsx` handles navigation and live status; `src/views/` contains the instance, account, mod, settings and log screens.

## Current limits

- A project owned Microsoft client ID with Minecraft Services permission is required for Microsoft sign in. Offline profiles work without one.
- NeoForge support is implemented through the official installer, but has not been exercised end to end against a signed in account in this environment.
- Signed self update is implemented but inactive in ordinary development builds. The publisher must provide `BLOCKYARD_UPDATE_PUBKEY` (the public key contents) and `BLOCKYARD_UPDATE_ENDPOINT` (an HTTPS Tauri update feed) at compile time, then build with `npm run tauri build -- --config src-tauri/updater.release.conf.json` and sign the artifacts with `TAURI_SIGNING_PRIVATE_KEY`. The private key must stay outside the repository. The Settings screen only offers update checks when a build includes both values. See [Tauri updater](https://v2.tauri.app/plugin/updater/).
- Older release asset layouts may require additional compatibility work. Current release metadata is the primary target. Windows and macOS builds have not been exercised end to end; the current OS version range probe is implemented for Unix and needs a native Windows version provider.
- Mod management is local JAR import, enable/disable and removal. There is no catalog or dependency resolver yet.
- On Arch Linux, the production binary was built and launched with WebKitGTK 4.1 through XWayland. Its Hyprland window opened floating at 1180×760 without a per-user rule. A complete game session has not yet been verified here.

## License

MIT. See [LICENSE](LICENSE).
