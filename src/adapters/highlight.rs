use std::path::PathBuf;
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
            ss: SyntaxSet::load_defaults_newlines(),
            ts: ThemeSet::load_defaults(),
            dark_theme: "base16-ocean.dark".into(),
            light_theme: "InspiredGitHub".into(),
        }
    }

    pub fn highlight_with_theme(&self, path: &PathBuf, source: &str, dark: bool) -> Vec<StyledLine> {
        let theme_name = if dark { &self.dark_theme } else { &self.light_theme };
        let theme = match self.ts.themes.get(theme_name) {
            Some(t) => t,
            None => return plain_lines(source),
        };
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let syntax = self
            .ss
            .find_syntax_by_extension(ext)
            .unwrap_or_else(|| self.ss.find_syntax_plain_text());

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
    fn highlight(&self, path: &PathBuf, source: &str) -> Vec<StyledLine> {
        self.highlight_with_theme(path, source, true)
    }
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
