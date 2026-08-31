use std::path::Path;
use ratatui::style::{Color, Modifier, Style};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

use crate::ports::{Highlighter, StyledLine, StyledSpan};

pub struct SyntectHighlighter {
    ss: SyntaxSet,
    ts: ThemeSet,
    dark_theme: String,
    light_theme: String,
}

impl SyntectHighlighter {
    pub fn new() -> Self {
        Self {
            // two-face ships syntect defaults plus extras (Kotlin, TS, …)
            ss: two_face::syntax::extra_newlines(),
            ts: ThemeSet::load_defaults(),
            dark_theme: "base16-ocean.dark".into(),
            light_theme: "InspiredGitHub".into(),
        }
    }

    pub fn highlight_with_theme(&self, path: &Path, source: &str, dark: bool) -> Vec<StyledLine> {
        let theme_name = if dark { &self.dark_theme } else { &self.light_theme };
        let theme = match self.ts.themes.get(theme_name) {
            Some(t) => t,
            None => return plain_lines(source),
        };
        let syntax = resolve_syntax(&self.ss, path);
        let mut h = HighlightLines::new(syntax, theme);
        let mut result = Vec::new();
        for line in LinesWithEndings::from(source) {
            let ranges = match h.highlight_line(line, &self.ss) {
                Ok(r) => r,
                Err(_) => return plain_lines(source),
            };
            let spans: StyledLine = ranges
                .iter()
                .map(|(style, text)| {
                    let fg = syntect_color(style.foreground);
                    let mut rs = Style::default().fg(fg);
                    if style.font_style.contains(FontStyle::BOLD) {
                        rs = rs.add_modifier(Modifier::BOLD);
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        rs = rs.add_modifier(Modifier::ITALIC);
                    }
                    StyledSpan { text: text.trim_end_matches('\n').to_string(), style: rs }
                })
                .collect();
            result.push(spans);
        }
        result
    }
}

impl Highlighter for SyntectHighlighter {
    fn highlight(&self, path: &Path, source: &str, dark_theme: bool) -> Vec<StyledLine> {
        self.highlight_with_theme(path, source, dark_theme)
    }
}

fn resolve_syntax<'a>(ss: &'a SyntaxSet, path: &Path) -> &'a syntect::parsing::SyntaxReference {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !ext.is_empty() {
        if let Some(s) = ss.find_syntax_by_extension(&ext) {
            return s;
        }
    }
    // First-line / path heuristics (e.g. shebang, Makefile)
    if let Ok(Some(s)) = ss.find_syntax_for_file(path) {
        return s;
    }
    ss.find_syntax_plain_text()
}

fn syntect_color(c: syntect::highlighting::Color) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

fn plain_lines(source: &str) -> Vec<StyledLine> {
    source
        .lines()
        .map(|l| vec![StyledSpan { text: l.to_string(), style: Style::default() }])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn non_plain_spans(path: &str, source: &str) -> bool {
        let hl = SyntectHighlighter::new();
        let lines = hl.highlight_with_theme(Path::new(path), source, true);
        lines.iter().flatten().any(|s| {
            matches!(s.style.fg, Some(Color::Rgb(_, _, _)))
        })
    }

    #[test]
    fn highlights_requested_languages() {
        let cases: &[(&str, &str)] = &[
            ("src/main.rs", "fn main() {\n    let x = 1;\n}\n"),
            ("app.js", "const x = { a: 1 };\nfunction f() { return x; }\n"),
            ("data.json", "{\n  \"name\": \"rustiq\",\n  \"ok\": true\n}\n"),
            ("index.html", "<html><body><div class=\"x\">hi</div></body></html>\n"),
            ("Main.kt", "fun main() {\n    val x = 1\n    println(x)\n}\n"),
        ];
        for (path, source) in cases {
            assert!(
                non_plain_spans(path, source),
                "expected syntax colors for {path}"
            );
        }
    }

    #[test]
    fn resolves_kotlin_and_json_extensions() {
        let ss = two_face::syntax::extra_newlines();
        assert_eq!(
            resolve_syntax(&ss, &PathBuf::from("A.KT")).name,
            "Kotlin"
        );
        assert_eq!(
            resolve_syntax(&ss, Path::new("cfg.JSON")).name,
            "JSON"
        );
    }
}
