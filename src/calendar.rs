use ratatui::style::Color;

#[derive(Debug, Clone)]
pub enum Calendar {
    Primary,
    Private,
    University,
}

impl Calendar {
    pub fn id(&self) -> String {
        match self {
            Calendar::Primary => "primary".to_string(),
            Calendar::Private => "6cm3jsuvlmq0jkvml9bd5l272k@group.calendar.google.com".to_string(),
            Calendar::University => {
                "t5pc1renkfb0q54klr31bgp894@group.calendar.google.com".to_string()
            }
        }
    }

    pub fn color(&self) -> Color {
        match self {
            Calendar::Primary => Color::Red,
            Calendar::Private => Color::Blue,
            Calendar::University => Color::Green,
        }
    }
}
