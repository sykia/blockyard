# Blockyard 0.4.0

Blockyard is a free, open source Minecraft Java Edition launcher for Windows, Arch Linux and other Linux distributions.

## New in 0.4.0

- Removed window dragging from the app header. Move the window with the operating system title bar; Hyprland still opens it floating and centered.
- Faster repeat launches: the Mojang version manifest is cached briefly, a working Java runtime is found sooner, and an installed NeoForge profile is reused without another Maven lookup or installer run.
- Libraries and assets now use one parallel verification and download queue. Cached files are still SHA-1 checked before the game starts.
- Added Dark, Light and System themes, plus an interface animation setting that respects Reduce Motion.
- Discover now loads project icons, full descriptions, screenshots and files from Modrinth and CurseForge. Mod and modpack installation includes required dependencies and separate instances.

## Included features

- Multiple independent instances with Vanilla, Fabric and NeoForge; shared game file and Java caches.
- Automatic Minecraft, asset, library, native and Java runtime installation; per-instance Java and memory settings.
- Microsoft sign-in through the browser based device flow and offline profiles. Microsoft sign-in requires your own approved application client ID; offline profiles need no client ID.
- Mod management, game logs and crash reports, signed launcher updates, and Windows, AppImage, DEB, RPM and Arch packages.

## Downloads and updates

Choose the Windows NSIS installer, the Arch `pkg.tar.zst` package, or the AppImage, DEB or RPM for your Linux distribution below. Checksums are in `SHA256SUMS`. Packaged 0.3.1+ installations can update through **Settings**. Arch, DEB and RPM updates open a terminal for the system package manager and sudo; the Windows installer shows its install progress.

## Known limits

- CurseForge requires a personal API key obtained from CurseForge. Some files cannot be downloaded through third-party launchers because of project restrictions.
- Microsoft authentication requires an application registration approved for Minecraft Services. Offline profiles cannot join online-mode servers.
- Forge and Quilt modpacks are not supported. Windows builds are not Authenticode signed, so Windows may display an Unknown Publisher prompt.
- NeoForge and Windows game launches have not been exercised end to end in this environment. Please report failures with the diagnostic text from the Logs screen.
