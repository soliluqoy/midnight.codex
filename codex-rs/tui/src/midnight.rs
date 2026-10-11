//! Midnight's native session sidebar. Git runs through the workspace host, never while painting.
use crate::tui::FrameRequester;
use crate::workspace_command::WorkspaceCommand;
use crate::workspace_command::WorkspaceCommandRunner;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::Wrap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::task::JoinHandle;

const WIDTH: u16 = 36;
const MIN_WIDTH: u16 = 110;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Snapshot {
    branch: String,
    staged: usize,
    unstaged: usize,
    untracked: usize,
    conflicted: usize,
    files: Vec<String>,
}

pub(crate) fn clean(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
                ' '
            } else {
                ch
            }
        })
        .collect()
}

// Refuse capped or malformed output instead of presenting incomplete counts as complete.
fn parse_status(output: &str) -> Option<Snapshot> {
    if output.len() >= 8000 || !output.ends_with('\0') {
        return None;
    }
    let mut fields = output.split_terminator('\0');
    let mut snapshot = Snapshot {
        branch: clean(fields.next()?.strip_prefix("## ")?),
        ..Default::default()
    };
    while let Some(row) = fields.next() {
        let bytes = row.as_bytes();
        if bytes.len() < 4 || bytes[2] != b' ' {
            return None;
        }
        let code = std::str::from_utf8(&bytes[..2]).ok()?;
        if code == "!!" {
            continue;
        }
        if code == "??" {
            snapshot.untracked += 1;
        } else if matches!(code, "DD" | "AU" | "UD" | "UA" | "DU" | "AA" | "UU") {
            snapshot.conflicted += 1;
        } else {
            snapshot.staged += usize::from(bytes[0] != b' ');
            snapshot.unstaged += usize::from(bytes[1] != b' ');
        }
        snapshot.files.push(clean(row));
        if bytes[..2].iter().any(|c| *c == b'R' || *c == b'C') {
            fields.next()?;
        }
    }
    Some(snapshot)
}

pub(crate) struct Sidebar {
    enabled: bool,
    cwd: Option<PathBuf>,
    dirty: bool,
    task: Option<JoinHandle<()>>,
    snapshot: Arc<Mutex<Option<Result<Snapshot, ()>>>>,
    pub(crate) scroll: u16,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            enabled: true,
            cwd: None,
            dirty: true,
            task: None,
            snapshot: Arc::default(),
            scroll: 0,
        }
    }
}

impl Sidebar {
    pub(crate) fn width(&self, total: u16) -> u16 {
        if self.enabled && total >= MIN_WIDTH {
            WIDTH
        } else {
            0
        }
    }

    pub(crate) fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.dirty = true;
        if !self.enabled
            && let Some(task) = self.task.take()
        {
            task.abort();
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub(crate) fn refresh(
        &mut self,
        cwd: PathBuf,
        runner: Option<WorkspaceCommandRunner>,
        frame: FrameRequester,
    ) {
        if self.cwd.as_ref() != Some(&cwd) {
            if let Some(task) = self.task.take() {
                task.abort();
            }
            self.cwd = Some(cwd.clone());
            self.snapshot = Arc::default();
            self.scroll = 0;
            self.dirty = true;
        }
        if !self.enabled
            || !self.dirty
            || self.task.as_ref().is_some_and(|task| !task.is_finished())
        {
            return;
        }
        let Some(runner) = runner else {
            return;
        };
        self.dirty = false;
        let snapshot = Arc::clone(&self.snapshot);
        self.task = Some(tokio::spawn(async move {
            let command = WorkspaceCommand::new([
                "git",
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "-z",
                "--branch",
                "--untracked-files=all",
            ])
            .cwd(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .timeout(Duration::from_secs(3));
            let result = runner
                .run(command)
                .await
                .ok()
                .filter(|output| output.success())
                .and_then(|output| parse_status(&output.stdout));
            if let Ok(mut value) = snapshot.lock() {
                *value = Some(result.ok_or(()));
            }
            frame.schedule_frame();
        }));
    }

    pub(crate) fn lines(&self, mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
        lines.push(Line::from(""));
        lines.push(Line::from("GIT".cyan().bold()));
        let snapshot = self.snapshot.lock().ok().and_then(|value| value.clone());
        if let Some(Ok(snapshot)) = snapshot {
            lines.push(Line::from(snapshot.branch));
            lines.push(Line::from(format!(
                "{} staged · {} unstaged",
                snapshot.staged, snapshot.unstaged
            )));
            lines.push(Line::from(format!(
                "{} untracked · {} conflicts",
                snapshot.untracked, snapshot.conflicted
            )));
            lines.push(Line::from(""));
            lines.push(Line::from("CHANGED FILES".cyan().bold()));
            lines.push(Line::from("Repository-wide changes".dim()));
            if snapshot.files.is_empty() {
                lines.push(Line::from("Working tree clean"));
            }
            lines.extend(snapshot.files.into_iter().map(Line::from));
        } else {
            lines.push(Line::from(if snapshot.is_none() {
                "Checking…"
            } else {
                "Git unavailable or status too large"
            }));
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Alt+S hide · Alt+R refresh".dim()));
        lines.push(Line::from("Wheel over sidebar to scroll".dim()));
        lines
    }

    pub(crate) fn render(area: Rect, lines: Vec<Line<'static>>, scroll: u16, buffer: &mut Buffer) {
        if area.width == 0 {
            return;
        }
        Paragraph::new(lines)
            .block(
                Block::bordered()
                    .title(" Midnight ")
                    .border_style(Color::DarkGray),
            )
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .render(area, buffer);
    }

    pub(crate) fn clamp_scroll(&mut self, area: Rect, lines: &[Line<'static>]) {
        let rows = Paragraph::new(lines.to_vec())
            .wrap(Wrap { trim: false })
            .line_count(area.width.saturating_sub(2).max(1));
        let max_scroll = rows.saturating_sub(usize::from(area.height.saturating_sub(2)));
        self.scroll = self
            .scroll
            .min(max_scroll.min(usize::from(u16::MAX)) as u16);
    }
}

impl Drop for Sidebar {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_command::WorkspaceCommandError;
    use crate::workspace_command::WorkspaceCommandExecutor;
    use crate::workspace_command::WorkspaceCommandOutput;
    use std::future::Future;
    use std::pin::Pin;
    use tokio::sync::mpsc;
    use tokio::sync::oneshot;

    struct ControlledRunner(
        mpsc::UnboundedSender<(WorkspaceCommand, oneshot::Sender<WorkspaceCommandOutput>)>,
    );

    impl WorkspaceCommandExecutor for ControlledRunner {
        fn run(
            &self,
            command: WorkspaceCommand,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<WorkspaceCommandOutput, WorkspaceCommandError>>
                    + Send
                    + '_,
            >,
        > {
            Box::pin(async move {
                let (tx, rx) = oneshot::channel();
                self.0.send((command, tx)).expect("test receiver");
                Ok(rx.await.expect("test response"))
            })
        }
    }

    #[tokio::test]
    async fn midnight_coalesces_reads_and_discards_replaced_workspaces() {
        let (tx, mut calls) = mpsc::unbounded_channel();
        let runner: WorkspaceCommandRunner = Arc::new(ControlledRunner(tx));
        let mut sidebar = Sidebar::default();
        let refresh = |sidebar: &mut Sidebar, cwd: &str| {
            sidebar.refresh(
                PathBuf::from(cwd),
                Some(Arc::clone(&runner)),
                FrameRequester::test_dummy(),
            );
        };
        refresh(&mut sidebar, "first");
        let (command, mut first) = calls.recv().await.expect("first read");
        assert_eq!(command.cwd, Some(PathBuf::from("first")));
        assert_eq!(command.timeout, Duration::from_secs(3));
        assert!(!command.disable_output_cap);
        sidebar.invalidate();
        refresh(&mut sidebar, "first");
        assert!(calls.try_recv().is_err());

        refresh(&mut sidebar, "second");
        let (_, second) = calls.recv().await.expect("replacement read");
        tokio::time::timeout(Duration::from_secs(1), first.closed())
            .await
            .expect("old read cancelled");
        second
            .send(WorkspaceCommandOutput {
                exit_code: 0,
                stdout: "## second\0".into(),
                stderr: String::new(),
            })
            .expect("active read");
        sidebar
            .task
            .take()
            .expect("running task")
            .await
            .expect("completed read");
        assert_eq!(
            sidebar
                .snapshot
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap()
                .branch,
            "second"
        );
        refresh(&mut sidebar, "second");
        assert!(calls.try_recv().is_err());

        sidebar.invalidate();
        refresh(&mut sidebar, "second");
        let (_, mut hidden) = calls.recv().await.expect("manual refresh");
        sidebar.toggle();
        tokio::time::timeout(Duration::from_secs(1), hidden.closed())
            .await
            .expect("hidden read cancelled");
        refresh(&mut sidebar, "second");
        assert!(calls.try_recv().is_err());
    }

    #[test]
    fn midnight_counts_layers_renames_and_conflicts() {
        let state = parse_status(
            "## main...origin/main\0MM file.txt\0R  new.txt\0old.txt\0?? new\nfile\0UU conflict\0",
        )
        .expect("valid status");
        assert_eq!(
            (
                state.staged,
                state.unstaged,
                state.untracked,
                state.conflicted
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(state.files.len(), 4);
        assert_eq!(state.files[2], "?? new file");
    }
    #[test]
    fn midnight_refuses_partial_status_and_hides_at_narrow_width() {
        assert!(parse_status("## main\0R  new\0").is_none());
        assert!(parse_status("## main\0 M partial").is_none());
        assert!(parse_status(&format!("## main\0?? {}\0", "x".repeat(8000))).is_none());
        let mut sidebar = Sidebar::default();
        assert_eq!(sidebar.width(109), 0);
        assert_eq!(sidebar.width(110), 36);
        sidebar.toggle();
        assert_eq!(sidebar.width(160), 0);
    }
    #[test]
    fn midnight_renders_inside_its_rectangle() {
        let sidebar = Sidebar::default();
        let mut buffer = Buffer::empty(Rect::new(0, 0, 120, 30));
        Sidebar::render(
            Rect::new(84, 0, 36, 30),
            sidebar.lines(vec![Line::from("SESSION")]),
            0,
            &mut buffer,
        );
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(85, 1)].symbol(), "S");
    }
}
