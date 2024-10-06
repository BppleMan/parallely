use crate::console::Console;
use crate::context::Context;
use crate::message;
use crate::message::{Message, MessageSender, MessageStream};
use crate::parallely::Parallely;
use crate::shutdown_handler::{ShutdownHandler, ShutdownReason};
use crate::task_executor::{Executable, TaskStatus};
use ratatui::buffer::Buffer;
use ratatui::crossterm::event;
use ratatui::crossterm::event::Event;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, StatefulWidget, Widget};
use ratatui::{DefaultTerminal, Frame};
use tokio_stream::StreamExt;

pub struct App {
    message_sender: MessageSender,
    message_stream: MessageStream,
    shutdown_handler: ShutdownHandler,
    consoles: Vec<Console>,
    #[allow(unused)]
    exit_on_complete: bool,
}

impl App {
    pub fn new(parallely: Parallely) -> Self {
        let (message_sender, message_stream) = message::message_queue();
        let shutdown_handler = ShutdownHandler::new(message_sender.clone());
        let consoles = parallely
            .commands
            .into_iter()
            .map(|command| Console::new(command, message_sender.clone()))
            .collect();
        let exit_on_complete = parallely.exit_on_complete;
        App {
            message_sender,
            message_stream,
            shutdown_handler,
            consoles,
            exit_on_complete,
        }
    }

    pub async fn run(
        &mut self,
        mut terminal: DefaultTerminal,
    ) -> color_eyre::Result<ShutdownReason> {
        self.listen_events();
        self.listen_shutdown();
        for console in self.consoles.iter_mut() {
            console.execute()?;
        }

        let mut context = Context::default();

        loop {
            tracing::debug!("[Main Loop] Drawing frame");
            terminal.draw(|frame| self.draw(frame, &mut context))?;
            tracing::debug!("[Main Loop] Try-Waiting for events");
            let tasks_status = self
                .consoles
                .iter_mut()
                .map(|c| c.try_wait())
                .collect::<Vec<_>>();
            if !tasks_status
                .iter()
                .any(|s| matches!(s, Ok(TaskStatus::Executing { .. })))
            {
                tracing::debug!("All tasks completed");
                break Ok(ShutdownReason::End(tasks_status));
            }
            tracing::debug!("[Main Loop] Waiting for message");
            if let Some(message) = self.message_stream.next().await {
                match message {
                    Message::Error(error) => {
                        tracing::error!("[Main Loop] Error: {:?}", error);
                    }
                    Message::Shutdown(reason) => {
                        tracing::info!("[Main Loop] Shutdown: {:?}", reason);
                        let handles = self
                            .consoles
                            .iter_mut()
                            .map(|c| c.interrupt_and_wait())
                            .collect::<Vec<_>>();
                        let tasks_status = futures::future::join_all(handles).await;
                        break Ok(ShutdownReason::End(tasks_status));
                    }
                    Message::EventChunk(events) => {
                        self.handle_events(events, &mut context)?;
                    }
                    Message::Update => {}
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame, context: &mut Context) {
        frame.render_stateful_widget(self, frame.area(), context);
    }

    fn handle_events(
        &mut self,
        events: Vec<Event>,
        context: &mut Context,
    ) -> color_eyre::Result<()> {
        self.shutdown_handler.handle_events(&events);
        context.event_chunk.clear();
        context.event_chunk.extend(events);
        Ok(())
    }

    fn listen_events(&self) {
        let message_sender = self.message_sender.clone();
        tokio::spawn(async move {
            let event_stream =
                event::EventStream::new().chunks_timeout(100, std::time::Duration::from_millis(2));
            tokio::pin!(event_stream);
            while let Some(maybe_event) = event_stream.next().await {
                tracing::debug!("Received event chunk: {}", maybe_event.len());
                let events = maybe_event.into_iter().flatten().collect::<Vec<_>>();
                message_sender.send_event_chunk(events);
            }
        });
    }

    fn listen_shutdown(&self) {
        self.shutdown_handler.listen_for_signal();
    }
}

impl StatefulWidget for &mut App {
    type State = Context;

    fn render(self, area: Rect, buf: &mut Buffer, context: &mut Context)
    where
        Self: Sized,
    {
        let pid = std::process::id();
        let title = Title::from(format!(" Parallely - ({pid})").bold());
        let instructions = Title::from(Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]));
        let container = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(instructions.alignment(Alignment::Right));

        let areas = Layout::horizontal(
            self.consoles
                .iter()
                .map(|_| Constraint::Fill(0))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(container.inner(area));

        for (index, rect) in areas.iter().enumerate() {
            tracing::debug!("[Main Loop] Rendering console {}", index);
            self.consoles[index].render(*rect, buf, context);
        }

        container.render(area, buf);
    }
}
