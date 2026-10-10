"""exe に組み込まれる依存クレートのライセンス表示を 1 つのテキストにまとめる。

使い方:
    python tools/third_party_licenses.py THIRD_PARTY_LICENSES.txt

`cargo metadata` で Windows 向けビルドの依存関係を辿り、通常の依存（ビルド時のみ・テスト用を除く）について
各クレートに含まれる LICENSE / COPYING / NOTICE などのファイルを書き出す。
egui が埋め込む既定フォント（epaint_default_fonts）のライセンス文も含める。
同じ内容のライセンス文は 2 回目以降を省略する。
"""

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET = "x86_64-pc-windows-msvc"
LICENSE_FILE = re.compile(r"^(licen[cs]e|copying|notice|copyright|unlicense)([-_.].*)?$", re.IGNORECASE)
# クレートのルート以外にあるライセンス文（クレート名 → 相対ディレクトリ）
EXTRA_DIRS = {"epaint_default_fonts": "fonts"}


def metadata() -> dict:
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET, "--locked"],
        cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8",
    ).stdout
    return json.loads(out)


def runtime_packages(meta: dict) -> list[dict]:
    """ルートから通常の依存だけを辿ったパッケージ（ルート自身は除く）。"""
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = meta["resolve"]["root"]
    seen, stack = set(), [root]
    while stack:
        node = nodes[stack.pop()]
        for dep in node["deps"]:
            if dep["pkg"] in seen:
                continue
            if any(k["kind"] is None for k in dep["dep_kinds"]):
                seen.add(dep["pkg"])
                stack.append(dep["pkg"])
    return sorted((packages[i] for i in seen), key=lambda p: (p["name"], p["version"]))


def license_files(package: dict) -> list[Path]:
    root = Path(package["manifest_path"]).parent
    files = [f for f in sorted(root.iterdir()) if f.is_file() and LICENSE_FILE.match(f.name)]
    if extra := EXTRA_DIRS.get(package["name"]):
        files += sorted((root / extra).glob("*.txt"))
    return files


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    out_path = Path(sys.argv[1])

    lines = [
        "Third-party software included in performance-monitor.exe",
        "performance-monitor.exe に組み込まれている第三者ソフトウェアのライセンス表示",
        "",
        "Japanese font: BIZ UDPGothic subset — SIL Open Font License 1.1 (see OFL.txt)",
        "",
        "Packages that ship no license file are listed with their authors and SPDX license expression;",
        "the full text of each of those licenses appears elsewhere in this file.",
        "",
    ]
    printed: dict[str, str] = {}
    packages = runtime_packages(metadata())
    for package in packages:
        name = f"{package['name']} {package['version']}"
        lines += ["=" * 78, name, f"License: {package.get('license') or 'see below'}"]
        if repo := package.get("repository"):
            lines.append(f"Repository: {repo}")
        files = license_files(package)
        if not files:
            authors = ", ".join(package.get("authors") or []) or "unknown"
            lines += [f"Authors: {authors}", "(No license file in the package; licensed under the SPDX expression above.)"]
        for f in files:
            text = f.read_text(encoding="utf-8", errors="replace").strip()
            digest = hashlib.sha256(text.encode()).hexdigest()
            lines += ["", f"--- {f.name} ---"]
            if digest in printed:
                lines.append(f"(Same text as {f.name} of {printed[digest]})")
            else:
                printed[digest] = name
                lines.append(text)
        lines.append("")

    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(packages)} packages -> {out_path} ({out_path.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
