#!/usr/bin/env python3
"""Download pinned optional inference artifacts only after --download is supplied.

Requires Python 3.12+. Uses TLS verification, official source checksums,
a separate cache, and no keys.
This developer helper is optional; installed Atlas runs with AI disabled by default.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform
import stat
import sys
import tarfile
import tempfile
import time
from urllib.error import URLError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, Request, build_opener
import zipfile

MODEL_SOURCE = "https://huggingface.co/Qwen/"
MODELS = {
    "1.7b": {"name": "Qwen3-1.7B-Q8_0", "repo": "Qwen3-1.7B-GGUF", "revision": "90862c4b9d2787eaed51d12237eafdfe7c5f6077", "bytes": 1834426016, "sha256": "061b54daade076b5d3362dac252678d17da8c68f07560be70818cace6590cb1a"},
    "0.6b": {"name": "Qwen3-0.6B-Q8_0", "repo": "Qwen3-0.6B-GGUF", "revision": "23749fefcc72300e3a2ad315e1317431b06b590a", "bytes": 639446688, "sha256": "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031"},
}
RUNTIMES = {
    ("Linux", "x86_64"): ("llama-b11827-bin-ubuntu-x64.tar.gz", 22343386, "7c1de2b2a139aa90e28b88d44a75f5966485604f68aa0ccdc2339959bca9b5f6"),
    ("Windows", "AMD64"): ("llama-b11827-bin-win-cpu-x64.zip", 24074494, "2740eeac6ed9b727a3a1002f5a42e74defa3c4cbb9a8b42b11356a07f969cb13"),
    ("Darwin", "arm64"): ("llama-b11827-bin-macos-arm64.tar.gz", 16153601, "08e6c4157426dbaebaea8b49639fc982a502c39726f310ec8f3985ed683147d9"),
    ("Darwin", "x86_64"): ("llama-b11827-bin-macos-x64.tar.gz", 11888988, "9c4122caebacda10ae16d5cf7f437e172b6ff4def50c8b0fd6adf8824378906c"),
}


class HTTPSOnlyRedirect(HTTPRedirectHandler):
    """Reject downgrades before urllib creates or sends the redirected request."""

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        if urlsplit(newurl).scheme.lower() != "https":
            raise URLError("Refusing an artifact redirect to a non-HTTPS destination")
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def download(url: str, target: Path, size: int, expected: str) -> None:
    if urlsplit(url).scheme.lower() != "https":
        raise ValueError("Artifact downloads require HTTPS")
    if target.exists():
        if target.stat().st_size == size and digest(target) == expected:
            print(f"Reusing verified {target.name}")
            return
        raise ValueError(f"Existing {target.name} does not match the official checksum; preserve/investigate it before retrying.")
    started = time.monotonic()
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, prefix="atlas-download-", delete=False) as output:
            temporary = Path(output.name)
            received = 0
            hash_value = hashlib.sha256()
            # Default HTTPSHandler retains certificate/hostname verification.
            opener = build_opener(HTTPSOnlyRedirect())
            with opener.open(Request(url, headers={"User-Agent": "Birthright-Atlas-Developer-Setup/0.1"}), timeout=60) as response:
                if response.url.split(":", 1)[0] != "https":
                    raise ValueError("Refusing an insecure artifact redirect")
                while chunk := response.read(1024 * 1024):
                    received += len(chunk)
                    if received > size or time.monotonic() - started > 900:
                        raise ValueError("Artifact exceeded its expected size or download deadline")
                    hash_value.update(chunk)
                    output.write(chunk)
            if received != size or hash_value.hexdigest() != expected:
                raise ValueError(f"Official checksum/size verification failed for {target.name}")
        temporary.replace(target)
        print(f"Verified {target.name}: {expected}")
    finally:
        if temporary and temporary.exists():
            temporary.unlink()


def extract(archive: Path, destination: Path) -> None:
    if destination.exists():
        raise ValueError(f"Runtime directory already exists: {destination}; use the existing verified settings or choose an empty cache.")
    destination.mkdir(mode=0o700)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                path = Path(member.filename)
                if path.is_absolute() or ".." in path.parts or stat.S_ISLNK(member.external_attr >> 16):
                    raise ValueError("Unsafe path in verified runtime archive")
            bundle.extractall(destination)
    else:
        with tarfile.open(archive) as bundle:
            bundle.extractall(destination, filter="data")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.version_info < (3, 12):
        parser.error("Python 3.12 or newer is required for hashlib.file_digest and safe tar extraction filters")
    parser.add_argument("--model", choices=MODELS, default="1.7b")
    parser.add_argument("--cache", type=Path, required=True, help="Absolute private cache outside the source/program directory")
    parser.add_argument("--download", action="store_true", help="Explicitly download the displayed artifacts")
    args = parser.parse_args()
    model = MODELS[args.model]
    platform_key = (platform.system(), platform.machine())
    if platform_key not in RUNTIMES:
        raise ValueError(f"No pinned runtime catalog entry for {platform_key}")
    runtime_name, runtime_size, runtime_hash = RUNTIMES[platform_key]
    cache = args.cache.expanduser().resolve()
    checkout = Path(__file__).resolve().parent.parent
    if cache == checkout or checkout in cache.parents:
        raise ValueError("Keep inference artifacts outside the source checkout")
    model_url = f"{MODEL_SOURCE}{model['repo']}/resolve/{model['revision']}/{model['name']}.gguf?download=true"
    runtime_url = f"https://github.com/ggml-org/llama.cpp/releases/download/b11827/{runtime_name}"
    print(f"Model: {model['name']} — {model['bytes']:,} bytes; Apache-2.0")
    print(f"Runtime: {runtime_name} — {runtime_size:,} bytes; llama.cpp MIT")
    print(f"Total download: {model['bytes'] + runtime_size:,} bytes. Model size is not runtime RAM.")
    print(f"Cache: {cache}")
    if not args.download:
        print("Review these sizes, then add --download to install. No files were downloaded.")
        return
    cache.mkdir(parents=True, exist_ok=True, mode=0o700)
    model_file = cache / f"{model['name']}.gguf"
    runtime_archive = cache / runtime_name
    download(model_url, model_file, model["bytes"], model["sha256"])
    download(runtime_url, runtime_archive, runtime_size, runtime_hash)
    runtime_directory = cache / "llama-b11827"
    extract(runtime_archive, runtime_directory)
    executable_name = "llama-server.exe" if platform.system() == "Windows" else "llama-server"
    candidates = list(runtime_directory.rglob(executable_name))
    if len(candidates) != 1:
        raise ValueError("Runtime archive did not contain exactly one server executable")
    executable = candidates[0]
    inventory = {str(file.relative_to(runtime_directory)): digest(file) for file in runtime_directory.rglob("*") if file.is_file() and not file.is_symlink()}
    metadata = {"model": model, "modelSource": model_url, "modelLicense": "Apache-2.0", "runtimeSource": runtime_url, "runtimeArchiveSha256": runtime_hash, "runtimeCommit": "970b51ce7c9ba5fff0726c5e9994f1c2f0e4eedc", "runtimeFiles": inventory}
    (cache / "artifact-inventory.json").write_text(json.dumps(metadata, indent=2))
    settings = {"ai": {"enabled": True, "modelName": model["name"], "runtimePath": str(executable), "runtimeSha256": digest(executable), "modelPath": str(model_file), "modelSha256": model["sha256"], "modelSource": f"{MODEL_SOURCE}{model['repo']}/tree/{model['revision']}", "modelLicense": "Apache-2.0"}}
    (cache / "atlas-ai-settings.json").write_text(json.dumps(settings, indent=2))
    print(f"Verified artifact inventory and local Atlas settings saved under {cache}.")
    print("Run the synthetic benchmark before making performance claims. This helper does not modify application records.")


if __name__ == "__main__":
    main()
