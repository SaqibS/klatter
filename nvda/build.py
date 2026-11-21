"""Builds klatter.nvda-addon: NVDA is a 64-bit x86 process, so it needs an x64 DLL
even on ARM64 Windows."""
import configparser
import os
import pathlib
import shutil
import subprocess
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
ADDON = ROOT / "nvda" / "addon"
# MSVC on Windows; x86_64-pc-windows-gnu (MinGW) cross-builds from macOS or Linux.
TARGET = os.environ.get("KLATTER_TARGET", "x86_64-pc-windows-msvc")


def main():
    subprocess.run(["cargo", "build", "--release", "-q", "-p", "klatter-ffi", "--target", TARGET],
                   cwd=ROOT, check=True)
    shutil.copy(ROOT / "target" / TARGET / "release" / "klatter.dll",
                ADDON / "synthDrivers" / "klatter" / "klatter.dll")
    manifest = configparser.ConfigParser()
    manifest.read_string("[addon]\n" + (ADDON / "manifest.ini").read_text(encoding="utf-8"))
    out = ROOT / "target" / f"klatter-{manifest['addon']['version'].strip('\"')}.nvda-addon"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(ADDON.rglob("*")):
            if "__pycache__" in path.parts:
                continue
            # Folders get entries of their own, as the zip tool writes them: an
            # add-on without them was once refused by NVDA as invalid.
            if path.is_dir():
                z.writestr(path.relative_to(ADDON).as_posix() + "/", "")
            elif path.is_file():
                z.write(path, path.relative_to(ADDON).as_posix())
        z.write(ROOT / "data" / "cmudict" / "LICENSE.txt", "synthDrivers/klatter/cmudict-LICENSE.txt")
    print(out)


if __name__ == "__main__":
    main()
