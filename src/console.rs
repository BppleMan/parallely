use crate::app::ErrorSender;
use crate::parallely::ParallelyResult;
use ansi_to_tui::IntoText;
use crossterm::event::MouseEvent;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Layout, Margin, Rect};
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};
use ratatui::widgets::block::Title;
use ratatui::widgets::{
    Block, BorderType, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Widget,
};
use std::borrow::Cow;
use std::cmp::min;
use std::future::Future;
use tokio::io::{AsyncBufReadExt, AsyncRead};
use tokio::process::Command;
use tokio::sync::mpsc;

pub type ConsoleOutputSender = mpsc::UnboundedSender<String>;
pub type ConsoleOutputReceiver = mpsc::UnboundedReceiver<String>;

#[derive(Debug)]
pub struct ConsoleState {
    command: String,
    pid: Option<u32>,
    output: Option<ConsoleOutputReceiver>,
    output_text: Text<'static>,
    output_vertical_scroll: usize,
    mouse_event: Option<MouseEvent>,
    error_sender: ErrorSender,
}

impl ConsoleState {
    pub fn new(command: String, error_sender: ErrorSender) -> Self {
        Self {
            command,
            pid: None,
            output: None,
            output_text: Text::default(),
            output_vertical_scroll: 0,
            mouse_event: None,
            error_sender,
        }
    }

    pub fn spawn(
        &mut self,
    ) -> color_eyre::Result<impl Future<Output = color_eyre::Result<ParallelyResult>> + Sized> {
        let (stdout_sender, stdout_receiver) = mpsc::unbounded_channel();
        self.output = Some(stdout_receiver);
        let mut args = self.command.split_whitespace().collect::<Vec<_>>();
        let mut child = Command::new(args.remove(0))
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        self.pid = child.id();
        let stdout_task = Self::forward_output(
            child.stdout.take(),
            stdout_sender.clone(),
            self.error_sender.clone(),
        );
        let stderr_task = Self::forward_output(
            child.stderr.take(),
            stdout_sender,
            self.error_sender.clone(),
        );
        let command = self.command.clone();
        Ok(async move {
            stdout_task.await?;
            stderr_task.await?;
            let exit_status = child.wait().await?;
            Ok(ParallelyResult {
                command,
                exit_status,
            })
        })
    }

    async fn forward_output(
        reader: Option<impl AsyncRead + Unpin + Send + 'static>,
        sender: ConsoleOutputSender,
        error_sender: ErrorSender,
    ) -> color_eyre::Result<()> {
        if let Some(reader) = reader {
            tokio::spawn(async move {
                let mut buf_reader = tokio::io::BufReader::new(reader).lines();
                while let Ok(Some(line)) = buf_reader.next_line().await {
                    if let Err(e) = sender.send(line) {
                        error_sender.send(e.into()).expect("Failed to send error");
                    }
                }
            })
            .await?;
        }
        Ok(())
    }

    pub fn mouse_event(&mut self, mouse_event: Option<MouseEvent>) {
        self.mouse_event = mouse_event;
    }

    fn handle_mouse_event(&mut self, vertical_scroll: usize, rect: Rect) -> usize {
        if let Some(mouse_event) = self.mouse_event.take() {
            if rect.contains((mouse_event.column, mouse_event.row).into()) {
                match mouse_event.kind {
                    crossterm::event::MouseEventKind::ScrollUp => vertical_scroll.saturating_sub(1),
                    crossterm::event::MouseEventKind::ScrollDown => {
                        vertical_scroll.saturating_add(1)
                    }
                    _ => vertical_scroll,
                }
            } else {
                vertical_scroll
            }
        } else {
            vertical_scroll
        }
    }

    pub fn receive(&mut self, width_limit: usize) -> color_eyre::Result<()> {
        if let Some(output) = self.output.as_mut() {
            while let Ok(line) = output.try_recv() {
                let wrapped_lines = Self::wrap_text(&line, width_limit);
                Self::append_text(&mut self.output_text, wrapped_lines);
            }
        }
        Ok(())
    }

    fn wrap_text(text: &str, width_limit: usize) -> Vec<String> {
        textwrap::wrap(text, width_limit)
            .into_iter()
            .map(|part| match part {
                Cow::Borrowed(sub_str) => {
                    let start = sub_str.as_ptr() as usize - text.as_ptr() as usize;
                    let end = start + sub_str.len();
                    text[start..end].to_owned()
                }
                Cow::Owned(str) => str,
            })
            .collect::<Vec<_>>()
    }

    fn append_text(text: &mut Text<'static>, lines: Vec<String>) {
        lines.into_iter().for_each(|line| match line.into_text() {
            Ok(t) => text.extend(t),
            Err(_) => text.push_line(line),
        });
    }
}

#[derive(Default)]
pub struct Console {
    error_sender: Option<ErrorSender>,
}

impl Console {
    pub fn new(error_sender: Option<ErrorSender>) -> Self {
        Self { error_sender }
    }

    pub fn error_sender(mut self, error_sender: ErrorSender) -> Self {
        self.error_sender = Some(error_sender);
        self
    }
}

impl StatefulWidget for Console {
    type State = ConsoleState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State)
    where
        Self: Sized,
    {
        let container = Block::default();
        let inner_area = container.inner(area);
        container.render(area, buf);

        let width_limit = inner_area.width as usize - 2;
        state.receive(width_limit).unwrap_or_else(|e| {
            if let Some(sender) = self.error_sender.as_ref() {
                sender
                    .send(e)
                    .expect("[Console] render: failed to send error");
            }
        });

        let title_str = format!("[{}] - ({})", state.command, state.pid.unwrap_or(0),);
        let title_text = Text::from(
            ConsoleState::wrap_text(&title_str, width_limit)
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        let [title_rect, stdout_rect] = Layout::vertical([
            Constraint::Max(title_text.lines.len() as u16 + 2),
            Constraint::Min(1),
        ])
        .areas(inner_area);

        let title_block = Block::bordered()
            .title(" Command - PID ".magenta().bold())
            .border_type(BorderType::Rounded);
        let title = Paragraph::new(title_text.blue()).block(title_block);
        title.render(title_rect, buf);

        let stdout_block = Block::bordered()
            .title(Title::from(" [output] ".green().bold()).alignment(Alignment::Left))
            .border_type(BorderType::Rounded);
        let stdout_scroll_max = state
            .output_text
            .lines
            .len()
            .saturating_sub(stdout_block.inner(stdout_rect).height as usize);
        let stdout = Paragraph::new(state.output_text.clone())
            .scroll((state.output_vertical_scroll as u16, 0))
            .block(stdout_block);
        stdout.render(stdout_rect, buf);

        state.output_vertical_scroll = min(
            state.handle_mouse_event(state.output_vertical_scroll, stdout_rect),
            stdout_scroll_max,
        );

        let stdout_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut stdout_scrollbar_state =
            ScrollbarState::new(stdout_scroll_max).position(state.output_vertical_scroll);
        stdout_scrollbar.render(
            stdout_rect.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            buf,
            &mut stdout_scrollbar_state,
        );
    }
}
