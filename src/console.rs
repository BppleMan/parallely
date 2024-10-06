use crate::context::Context;
use crate::message::MessageSender;
use crate::task_executor::{Executable, TaskExecutor, TaskOutputReceiver};
use ansi_to_tui::IntoText;
use crossterm::event::{MouseEvent, MouseEventKind};
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
use std::ops::{Deref, DerefMut};

pub struct Console {
    executor: TaskExecutor,
    output: Option<TaskOutputReceiver>,
    output_text: Text<'static>,
    output_vertical_scroll: usize,
    message_sender: MessageSender,
}

impl Console {
    pub fn new(command: String, message_sender: MessageSender) -> Self {
        let executor = TaskExecutor::new(command);
        Self {
            executor,
            output: None,
            output_text: Text::default(),
            output_vertical_scroll: 0,
            message_sender,
        }
    }

    pub fn execute(&mut self) -> color_eyre::Result<()> {
        let output_receiver = self.executor.execute()?;
        self.output = Some(output_receiver);
        Ok(())
    }

    fn handle_mouse_event(
        &mut self,
        mouse_event: &MouseEvent,
        rect: Rect,
        output_scroll_max: usize,
    ) {
        if rect.contains((mouse_event.column, mouse_event.row).into()) {
            self.output_vertical_scroll = match mouse_event.kind {
                MouseEventKind::ScrollUp => self.output_vertical_scroll.saturating_sub(1),
                MouseEventKind::ScrollDown => min(
                    self.output_vertical_scroll.saturating_add(1),
                    output_scroll_max,
                ),
                _ => self.output_vertical_scroll,
            }
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

impl StatefulWidget for &mut Console {
    type State = Context;

    fn render(self, area: Rect, buf: &mut Buffer, context: &mut Context)
    where
        Self: Sized,
    {
        let container = Block::default();
        let inner_area = container.inner(area);
        container.render(area, buf);

        let width_limit = inner_area.width as usize - 2;
        if let Err(e) = self.receive(width_limit) {
            self.message_sender.send_error(e);
        }

        let title_str = format!("[{}] - ({})", self.raw_command(), self.pid().unwrap_or(0));
        let title_text = Text::from(
            Console::wrap_text(&title_str, width_limit)
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        let [title_rect, output_rect] = Layout::vertical([
            Constraint::Max(title_text.lines.len() as u16 + 2),
            Constraint::Min(1),
        ])
        .areas(inner_area);

        let title_block = Block::bordered()
            .title(" Command - PID ".magenta().bold())
            .border_type(BorderType::Rounded);
        let title = Paragraph::new(title_text.blue()).block(title_block);
        title.render(title_rect, buf);

        let output_block = Block::bordered()
            .title(Title::from(" [output] ".green().bold()).alignment(Alignment::Left))
            .border_type(BorderType::Rounded);
        let output_scroll_max = self
            .output_text
            .lines
            .len()
            .saturating_sub(output_block.inner(output_rect).height as usize);
        context.try_as_mouse_events().for_each(|mouse_event| {
            self.handle_mouse_event(mouse_event, output_rect, output_scroll_max);
        });
        let output = Paragraph::new(self.output_text.clone())
            .scroll((self.output_vertical_scroll as u16, 0))
            .block(output_block);
        output.render(output_rect, buf);

        let output_scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));
        let mut scrollbar_state =
            ScrollbarState::new(output_scroll_max).position(self.output_vertical_scroll);
        output_scrollbar.render(
            output_rect.inner(Margin {
                horizontal: 0,
                vertical: 1,
            }),
            buf,
            &mut scrollbar_state,
        );
    }
}

impl Deref for Console {
    type Target = TaskExecutor;

    fn deref(&self) -> &Self::Target {
        &self.executor
    }
}

impl DerefMut for Console {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.executor
    }
}
