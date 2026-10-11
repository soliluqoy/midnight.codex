use crate::midnight::clean;
use ratatui::style::Stylize;
use ratatui::text::Line;
use std::path::PathBuf;

impl super::ChatWidget {
    pub(crate) fn midnight_cwd(&self) -> PathBuf {
        self.current_cwd
            .clone()
            .unwrap_or_else(|| self.config.cwd.to_path_buf())
    }
    pub(crate) fn midnight_lines(&self) -> Vec<Line<'static>> {
        let usage = self.token_usage();
        let mut lines = vec![
            Line::from("SESSION".cyan().bold()),
            Line::from(clean(
                &self.thread_name().unwrap_or_else(|| "New session".into()),
            )),
            Line::from(clean(self.model_display_name())),
            Line::from(clean(&self.midnight_cwd().display().to_string())),
            Line::from(format!(
                "Input {} · output {}",
                usage.input_tokens, usage.output_tokens
            )),
        ];
        if let Some(percent) = self.status_line_context_used_percent() {
            lines.push(Line::from(format!("Context {percent}% used")));
        }
        lines
    }
}
