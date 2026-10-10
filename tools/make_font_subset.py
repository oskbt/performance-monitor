"""画面表示に使う文字だけを含む日本語フォントのサブセットを作る。

使い方:
    pip install fonttools
    python tools/make_font_subset.py path/to/BIZUDPGothic-Regular.ttf

元フォントは Google Fonts で配布されている BIZ UDPGothic（SIL Open Font License 1.1）:
    https://github.com/google/fonts/tree/main/ofl/bizudpgothic

src/ 以下の文字列リテラルに含まれる非 ASCII 文字と、下の EXTRA の文字を収録する。
画面に出す日本語を追加・変更したら、このスクリプトを再実行すること
（`cargo test` の font_subset_covers_ui_strings が収録漏れを検出する）。
"""

import re
import sys
from pathlib import Path

from fontTools import subset

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "assets" / "fonts"
OUT_FONT = OUT_DIR / "BIZUDPGothic-subset.ttf"
OUT_CHARS = OUT_DIR / "subset-chars.txt"

# OS から取得して表示する文字列（ネットワークアダプタ名など）向けに、あらかじめ入れておく文字。
# ここに無い文字が現れた場合、アプリはシステムフォントを追加で読み込んで表示する。
EXTRA = (
    "".join(chr(c) for c in range(0x3041, 0x3097))  # ひらがな
    + "".join(chr(c) for c in range(0x30A1, 0x30FB))  # カタカナ
    + "ー・、。「」『』（）［］｛｝＊：；！？＃＆％＋－＝／＼～　"
    + "０１２３４５６７８９"
    + "ローカル エリア接続 ワイヤレス ネットワーク接続 イーサネット 無線 有線 仮想 内部 外部"
)

LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')


def ui_chars() -> set[str]:
    chars: set[str] = set()
    for path in (ROOT / "src").rglob("*.rs"):
        for line in path.read_text(encoding="utf-8").splitlines():
            code = line.split("//")[0]  # コメントは除く
            for literal in LITERAL.findall(code):
                chars.update(c for c in literal if ord(c) > 0x7E)
    return chars


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    source = Path(sys.argv[1])

    chars = ui_chars() | {c for c in EXTRA if ord(c) > 0x7E and not c.isspace()}
    text = "".join(sorted(chars))

    options = subset.Options()
    options.hinting = False  # egui はヒンティングを使わない
    options.name_IDs = ["*"]
    options.name_languages = ["*"]
    font = subset.load_font(str(source), options)
    subsetter = subset.Subsetter(options)
    subsetter.populate(text=text)
    subsetter.subset(font)

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    subset.save_font(font, str(OUT_FONT), options)
    OUT_CHARS.write_text(text, encoding="utf-8", newline="")

    print(f"{len(chars)} chars -> {OUT_FONT.relative_to(ROOT)} ({OUT_FONT.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
