use ratatui::style::{Color, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    DefaultDark,
    GithubLight,
}

impl Theme {
    pub fn cycle(self) -> Self {
        match self {
            Self::DefaultDark => Self::GithubLight,
            Self::GithubLight => Self::DefaultDark,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::DefaultDark => "default-dark",
            Self::GithubLight => "github-light",
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Self::DefaultDark)
    }

    pub fn bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(24, 24, 37),
            Self::GithubLight => Color::Rgb(255, 255, 255),
        }
    }

    pub fn fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(205, 214, 244),
            Self::GithubLight => Color::Rgb(36, 41, 47),
        }
    }

    pub fn border(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(88, 91, 112),
            Self::GithubLight => Color::Rgb(208, 215, 222),
        }
    }

    pub fn selection_bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(49, 50, 68),
            Self::GithubLight => Color::Rgb(234, 238, 242),
        }
    }

    pub fn added_bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(30, 70, 32),
            Self::GithubLight => Color::Rgb(204, 255, 204),
        }
    }

    pub fn removed_bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(80, 20, 20),
            Self::GithubLight => Color::Rgb(255, 220, 220),
        }
    }

    pub fn added_fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(166, 227, 161),
            Self::GithubLight => Color::Rgb(22, 128, 37),
        }
    }

    pub fn removed_fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(243, 139, 168),
            Self::GithubLight => Color::Rgb(176, 0, 32),
        }
    }

    pub fn stale_fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(127, 132, 156),
            Self::GithubLight => Color::Rgb(140, 140, 140),
        }
    }

    pub fn comment_fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(250, 179, 135),
            Self::GithubLight => Color::Rgb(154, 103, 0),
        }
    }

    pub fn comment_bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(10, 12, 20),
            Self::GithubLight => Color::Rgb(26, 29, 36),
        }
    }

    pub fn comment_text_fg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(230, 235, 255),
            Self::GithubLight => Color::Rgb(242, 246, 252),
        }
    }

    pub fn comment_border(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(245, 169, 127),
            Self::GithubLight => Color::Rgb(255, 196, 120),
        }
    }

    pub fn commented_line_bg(self) -> Color {
        match self {
            Self::DefaultDark => Color::Rgb(58, 42, 30),
            Self::GithubLight => Color::Rgb(255, 226, 186),
        }
    }

    pub fn base_style(self) -> Style {
        Style::default().fg(self.fg()).bg(self.bg())
    }
}
