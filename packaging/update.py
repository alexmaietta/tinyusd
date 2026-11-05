#!/usr/bin/env python3
import argparse
import hashlib
import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

REPO = "alexmaietta/tinyusd"
PLATFORMS = [f"{os}-{arch}" for os in ("linux", "macos", "windows") for arch in ("x86_64", "aarch64")]
KINDS = ["shared", "static"]
HERE = Path(__file__).resolve().parent
PORT = HERE / "vcpkg" / "ports" / "tinyusd"
CONDA = HERE / "conda" / "recipe.yaml"


def archive_hash(version, name, archives):
    digest = hashlib.sha512()
    if archives:
        path = archives / name
        if not path.exists():
            return None
        digest.update(path.read_bytes())
    else:
        url = f"https://github.com/{REPO}/releases/download/v{version}/{name}"
        try:
            with urllib.request.urlopen(url) as response:
                for chunk in iter(lambda: response.read(1 << 20), b""):
                    digest.update(chunk)
        except urllib.error.HTTPError as error:
            if error.code == 404:
                return None
            raise
    return digest.hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("version", help="release version without the leading v, such as 0.2.0")
    parser.add_argument("--archives", type=Path, help="directory holding the archives, instead of downloading them")
    parser.add_argument("--allow-missing", action="store_true", help="leave the hash of a missing archive as 0")
    args = parser.parse_args()

    lines = []
    missing = []
    for platform in PLATFORMS:
        for kind in KINDS:
            name = f"tinyusd-{args.version}-{platform}-{kind}.tar.gz"
            sha = archive_hash(args.version, name, args.archives)
            if sha is None:
                missing.append(name)
                sha = "0"
            lines.append(f"set(TINYUSD_SHA512_{platform}-{kind} {sha})")
    if missing and not args.allow_missing:
        sys.exit("missing archives:\n  " + "\n  ".join(missing))

    portfile = PORT / "portfile.cmake"
    text = portfile.read_text()
    block = "# BEGIN HASHES\n" + "\n".join(lines) + "\n# END HASHES"
    text, count = re.subn(r"# BEGIN HASHES\n.*?# END HASHES", lambda _: block, text, flags=re.S)
    if count != 1:
        sys.exit(f"{portfile} has no hash block")
    portfile.write_text(text)

    manifest = PORT / "vcpkg.json"
    data = json.loads(manifest.read_text())
    data["version"] = args.version
    data.pop("port-version", None)
    manifest.write_text(json.dumps(data, indent=2) + "\n")

    recipe = CONDA.read_text()
    recipe, count = re.subn(r'^(  version: )"[^"]*"', rf'\g<1>"{args.version}"', recipe, count=1, flags=re.M)
    if count != 1:
        sys.exit(f"{CONDA} has no version in its context")
    CONDA.write_text(recipe)

    for name in missing:
        print(f"missing {name}, its hash is left as 0", file=sys.stderr)


if __name__ == "__main__":
    main()
