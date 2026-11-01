"""Writes the licenses of every library compiled into the Windows app to
src-tauri/resources/licenses/THIRD-PARTY-DEPENDENCIES.txt (shipped with the installer).

Rust: the crates `v2raysybau` depends on for x86_64-pc-windows-msvc (normal dependencies only,
build tools are not shipped). JavaScript: the production dependencies and the Svelte runtime
bundled into the UI.
Run `cargo fetch --target x86_64-pc-windows-msvc` first so every crate's sources are present.
"""

import json
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "src-tauri" / "resources" / "licenses" / "THIRD-PARTY-DEPENDENCIES.txt"
TARGET = "x86_64-pc-windows-msvc"
LICENSE_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)([-._].*)?$", re.IGNORECASE)


def license_texts(directory: pathlib.Path) -> list[tuple[str, str]]:
    if not directory.is_dir():
        return []
    texts = []
    for path in sorted(directory.iterdir()):
        if path.is_file() and LICENSE_FILE.match(path.name):
            texts.append((path.name, path.read_text(encoding="utf-8", errors="replace").strip()))
    return texts


def rust_crates() -> list[dict]:
    metadata = json.loads(
        subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
    )
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    members = set(metadata["workspace_members"])
    app = next(p["id"] for p in metadata["packages"] if p["name"] == "v2raysybau")
    seen, stack = set(), [app]
    while stack:
        current = stack.pop()
        for dep in nodes[current]["deps"]:
            # Only dependencies linked into the program (not build scripts or dev tools).
            if any(kind["kind"] is None for kind in dep["dep_kinds"]) and dep["pkg"] not in seen:
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])
    crates = []
    for pkg_id in sorted(seen - members, key=lambda i: (packages[i]["name"], packages[i]["version"])):
        package = packages[pkg_id]
        directory = pathlib.Path(package["manifest_path"]).parent
        texts = license_texts(directory)
        if not texts and package.get("license_file"):
            file = directory / package["license_file"]
            if file.is_file():
                texts = [(file.name, file.read_text(encoding="utf-8", errors="replace").strip())]
        crates.append(
            {
                "name": package["name"],
                "version": package["version"],
                "license": package.get("license") or "see license text",
                "url": package.get("repository") or package.get("homepage") or f"https://crates.io/crates/{package['name']}",
                "texts": texts,
            }
        )
    return crates


# Svelte is a dev dependency (the compiler), but its small runtime is bundled into the UI.
BUNDLED_FROM_DEV = {"svelte", "esm-env", "clsx"}


def pnpm_licenses(*args: str) -> dict:
    result = subprocess.run(
        ["pnpm", "licenses", "list", "--json", *args],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
        shell=sys.platform == "win32",
    )
    return json.loads(result.stdout or "{}")


def npm_packages() -> list[dict]:
    production = {entry["name"] for entries in pnpm_licenses("--prod").values() for entry in entries}
    shipped = production | BUNDLED_FROM_DEV
    packages = []
    for license_name, entries in pnpm_licenses().items():
        for entry in entries:
            if entry["name"] not in shipped:
                continue
            paths = entry.get("paths") or ([entry["path"]] if entry.get("path") else [])
            texts = license_texts(pathlib.Path(paths[0])) if paths else []
            for version in entry.get("versions") or [entry.get("version", "")]:
                packages.append(
                    {
                        "name": entry["name"],
                        "version": version,
                        "license": license_name,
                        "url": entry.get("homepage") or f"https://www.npmjs.com/package/{entry['name']}",
                        "texts": texts,
                    }
                )
    return sorted(packages, key=lambda p: (p["name"], p["version"]))


def main() -> None:
    sections = [("Rust crates", rust_crates()), ("JavaScript packages", npm_packages())]
    lines = [
        "Libraries compiled into V2Ray Sybau, with their licenses.",
        "Third-party programs shipped next to it (Xray-core, sing-box, Wintun, geo data) are listed in",
        "THIRD-PARTY-NOTICES.md.",
        "",
    ]
    for title, items in sections:
        lines += ["=" * 78, f"{title} ({len(items)})", "=" * 78, ""]
        for item in items:
            lines.append(f"{item['name']} {item['version']} — {item['license']}")
            lines.append(f"  {item['url']}")
            if not item["texts"]:
                lines.append("  (the published package has no license file: the license is the one named above)")
        lines.append("")
    lines += ["=" * 78, "License texts", "=" * 78, ""]
    # Identical license texts (most MIT/Apache files) are printed once with every package using them.
    by_text: dict[str, list[str]] = {}
    for _, items in sections:
        for item in items:
            for name, text in item["texts"]:
                by_text.setdefault(text, []).append(f"{item['name']} {item['version']} ({name})")
    for text, users in by_text.items():
        lines.append("-" * 78)
        lines.append("Used by: " + ", ".join(users))
        lines.append("-" * 78)
        lines.append(text)
        lines.append("")
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text("\n".join(lines) + "\n", encoding="utf-8")
    crates, packages = (len(items) for _, items in sections)
    print(f"{OUT.relative_to(ROOT)}: {crates} crates, {packages} npm packages, {len(by_text)} distinct license texts")


if __name__ == "__main__":
    main()
