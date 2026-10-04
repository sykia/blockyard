"""Build Tauri's signed static update feed from release artifacts."""
import json
import pathlib
import sys
from urllib.parse import quote

assets = pathlib.Path(sys.argv[1])
tag = sys.argv[2]
version = tag.removeprefix("v")
patterns = {
    "windows-x86_64-nsis": "*-setup.exe",
    "linux-x86_64-appimage": "*.AppImage",
    "linux-x86_64-deb": "*.deb",
    "linux-x86_64-rpm": "*.rpm",
    "linux-x86_64-arch": "*.pkg.tar.zst",
}
platforms = {}
for target, pattern in patterns.items():
    matches = list(assets.glob(pattern))
    if len(matches) != 1:
        raise SystemExit(f"Expected exactly one {pattern} artifact, found {len(matches)}")
    artifact = matches[0]
    signature = artifact.with_name(artifact.name + ".sig")
    if not signature.is_file():
        raise SystemExit(f"Missing update signature: {signature.name}")
    platforms[target] = {
        "url": f"https://github.com/sykia/blockyard/releases/download/{quote(tag)}/{quote(artifact.name)}",
        "signature": signature.read_text().strip(),
    }
(assets / "latest.json").write_text(json.dumps({"version": version, "platforms": platforms}, indent=2) + "\n")
