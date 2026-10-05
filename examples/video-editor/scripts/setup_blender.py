#!/usr/bin/env python3
"""Fetch the pinned official Blender runtime into the ignored project target directory."""
import hashlib
import platform
import shutil
import subprocess
import tempfile
import urllib.request
from pathlib import Path

VERSION = '4.5.14'
BASE = f'https://download.blender.org/release/Blender4.5/'
ROOT = Path(__file__).resolve().parents[3] / 'target' / 'blender-runtime'


def fetch(name, destination):
    request = urllib.request.Request(BASE + name, headers={'User-Agent': 'Mozilla/5.0 FlowCut'})
    with urllib.request.urlopen(request, timeout=60) as response, destination.open('wb') as output:
        shutil.copyfileobj(response, output)


def main():
    if platform.system() != 'Darwin':
        raise SystemExit('Install Blender 4.5 LTS and set FLOWCUT_BLENDER to its executable. Automatic setup currently supports macOS.')
    arch = 'arm64' if platform.machine() == 'arm64' else 'x64'
    executable = ROOT / 'Blender.app' / 'Contents' / 'MacOS' / 'Blender'
    if executable.exists():
        subprocess.run([str(executable), '--version'], check=True)
        return
    ROOT.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='setup-', dir=ROOT) as directory:
        directory = Path(directory)
        filename = f'blender-{VERSION}-macos-{arch}.dmg'
        archive = directory / filename
        print(f'Downloading official Blender {VERSION} for macOS {arch}…', flush=True)
        fetch(filename, archive)
        manifest = directory / 'checksums.txt'
        fetch(f'blender-{VERSION}.sha256', manifest)
        expected = next(line.split()[0] for line in manifest.read_text().splitlines() if line.split()[-1].lstrip('*') == filename)
        digest = hashlib.sha256()
        with archive.open('rb') as file:
            for block in iter(lambda: file.read(1024 * 1024), b''): digest.update(block)
        if digest.hexdigest() != expected: raise SystemExit('Official download checksum mismatch.')
        mount = directory / 'mount'; mount.mkdir()
        subprocess.run(['hdiutil', 'attach', '-readonly', '-nobrowse', '-mountpoint', str(mount), str(archive)], check=True)
        try: shutil.copytree(mount / 'Blender.app', ROOT / 'Blender.app', symlinks=True)
        finally: subprocess.run(['hdiutil', 'detach', str(mount)], check=True)
    subprocess.run([str(executable), '--version'], check=True)
    print(f'Installed: {executable}')


if __name__ == '__main__': main()
