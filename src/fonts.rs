//! 日本語フォント。
//!
//! 普段は画面表示に使う文字だけを収録したサブセット（BIZ UDPGothic、約 80KB）を exe に埋め込んで使う。
//! ネットワークアダプタ名など OS から来る文字列にサブセットに無い文字が現れたときだけ、
//! システムの日本語フォント（約 10MB）を追加で読み込む。
//! サブセットは `tools/make_font_subset.py` で生成する。

use eframe::egui::{self, FontData, FontDefinitions, FontFamily, FontId};

const SUBSET: &[u8] = include_bytes!("../assets/fonts/BIZUDPGothic-subset.ttf");
/// フォールバックに使うシステムフォント（見つかった最初のもの）。
const SYSTEM_FONTS: [&str; 3] = ["meiryo.ttc", "YuGothM.ttc", "msgothic.ttc"];

pub struct JapaneseFonts {
    /// システムフォントを読み込み済み（または読み込みを試した）か。
    fallback_loaded: bool,
}

impl JapaneseFonts {
    pub fn install(ctx: &egui::Context) -> Self {
        ctx.set_fonts(definitions(None));
        Self { fallback_loaded: false }
    }

    /// 表示する文字列にサブセットで描けない文字があれば、システムフォントを追加で読み込む。
    /// 反映は次のフレームから。
    pub fn ensure_glyphs<'a>(&mut self, ctx: &egui::Context, texts: impl IntoIterator<Item = &'a str>) {
        if self.fallback_loaded {
            return;
        }
        let font = FontId::proportional(13.0);
        let missing = ctx.fonts_mut(|fonts| texts.into_iter().any(|t| !fonts.has_glyphs(&font, t)));
        if !missing {
            return;
        }

        self.fallback_loaded = true;
        match load_system_font() {
            Some(data) => {
                log::info!("サブセットに無い文字があるため、システムの日本語フォントを読み込みます");
                ctx.set_fonts(definitions(Some(data)));
            }
            None => log::warn!("システムの日本語フォントが見つかりませんでした"),
        }
    }
}

/// egui 既定のフォント（英数字）→ サブセット → システムフォント の順に使う。
fn definitions(system: Option<Vec<u8>>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    let mut names = vec!["jp-subset"];
    fonts.font_data.insert("jp-subset".into(), FontData::from_static(SUBSET).into());
    if let Some(data) = system {
        fonts.font_data.insert("jp-system".into(), FontData::from_owned(data).into());
        names.push("jp-system");
    }
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        list.extend(names.iter().map(|n| n.to_string()));
    }
    fonts
}

fn load_system_font() -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    SYSTEM_FONTS.iter().find_map(|name| std::fs::read(format!(r"{windir}\Fonts\{name}")).ok())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    const SUBSET_CHARS: &str = include_str!("../assets/fonts/subset-chars.txt");

    /// src/ 以下の文字列リテラルに含まれる非 ASCII 文字が、すべてサブセットに収録されているか。
    /// 失敗したら `python tools/make_font_subset.py <元フォント>` でサブセットを作り直す。
    #[test]
    fn font_subset_covers_ui_strings() {
        let covered: HashSet<char> = SUBSET_CHARS.chars().collect();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut missing = std::collections::BTreeSet::new();
        for path in rust_files(&src) {
            let text = std::fs::read_to_string(&path).unwrap();
            for line in text.lines() {
                let code = line.split("//").next().unwrap_or("");
                for literal in string_literals(code) {
                    missing.extend(literal.chars().filter(|c| *c as u32 > 0x7e && !covered.contains(c)));
                }
            }
        }
        assert!(missing.is_empty(), "サブセットに無い文字: {}", missing.iter().collect::<String>());
    }

    fn rust_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(rust_files(&path));
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
        files
    }

    /// 1 行の中の `"..."` の中身を取り出す（エスケープされた引用符は飛ばす）。
    fn string_literals(code: &str) -> Vec<&str> {
        let mut literals = Vec::new();
        let mut start = None;
        let mut escaped = false;
        for (i, c) in code.char_indices() {
            match (start, c) {
                (Some(_), '\\') if !escaped => escaped = true,
                (Some(s), '"') if !escaped => {
                    literals.push(&code[s..i]);
                    start = None;
                }
                (None, '"') => start = Some(i + 1),
                _ => escaped = false,
            }
        }
        literals
    }

    #[test]
    fn literal_scanner() {
        assert_eq!(string_literals(r#"f("abc", "d\"e")"#), vec!["abc", r#"d\"e"#]);
    }
}
