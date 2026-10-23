# Blockyard 0.5.0

A full update of the launcher interface, instance handling and account flow.

## Interface and branding
- Redesigned compact interface with Black and White themes, sidebar theme switching and clearer navigation.
- Updated instance, catalog, account and settings screens.
- New dark cube B logo with a transparent background for the app, installers and shortcuts.

## Instances and accounts
- Create and restore separate mods, config, saves, resource packs, shaders, screenshots and log folders for each instance without overwriting existing files.
- Improved Microsoft sign-in error handling and account refresh.
- Skip unavailable native library classifiers and hide the extra console window in Windows release builds.
- Keep database updates serialized while saving instance, account and launcher preferences.

- Updated DOMPurify and Vite to address dependency audit findings.

## Downloads
Windows NSIS installer; Linux AppImage, DEB, RPM and Arch packages; signed updater artifacts; latest.json and SHA256SUMS. Source and icon archives are included.

Windows installers use Tauri updater signatures but are not Authenticode signed. Minecraft gameplay and Microsoft sign-in still require verification with a real account and game installation.
