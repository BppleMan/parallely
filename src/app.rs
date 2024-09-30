use crate::console::{Console, ConsoleState};
use crate::parallely::{Parallely, ParallelyResult};
use futures::FutureExt;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event;
use ratatui::crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, StatefulWidget, Widget};
use ratatui::{DefaultTerminal, Frame};
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

type ShutdownSender = mpsc::UnboundedSender<color_eyre::Result<ShutdownReason>>;
type ShutdownReceiver = mpsc::UnboundedReceiver<color_eyre::Result<ShutdownReason>>;

pub type ErrorSender = mpsc::UnboundedSender<color_eyre::Report>;
type ErrorReceiver = mpsc::UnboundedReceiver<color_eyre::Report>;

#[derive(Default, Debug)]
pub struct App {
    error_sender: Option<ErrorSender>,
    console_states: Vec<ConsoleState>,
    exit_on_complete: bool,
    should_be_quit: bool,
}

impl App {
    pub fn add_console_state(&mut self, console_state: ConsoleState) {
        self.console_states.push(console_state);
    }

    pub async fn run(
        &mut self,
        mut terminal: DefaultTerminal,
        parallely: Parallely,
    ) -> color_eyre::Result<ShutdownReason> {
        let (shutdown_sender, mut shutdown_receiver) = init_shutdown();
        let (error_sender, mut error_receiver): (ErrorSender, ErrorReceiver) =
            mpsc::unbounded_channel();
        self.error_sender = Some(error_sender.clone());
        self.exit_on_complete = parallely.exit_on_complete;
        self.console_states = parallely
            .commands
            .into_iter()
            .map(|command| ConsoleState::new(command, error_sender.clone()))
            .collect();
        let tasks = self
            .console_states
            .iter_mut()
            .map(|cs| cs.spawn())
            .collect::<color_eyre::Result<Vec<_>>>()?;
        let mut tasks_future = futures::future::join_all(tasks).fuse();
        tokio::pin! {
            let event_stream = event::EventStream::new().throttle(Duration::from_millis(16));
        }

        let result = loop {
            terminal.draw(|frame| self.draw(frame))?;
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(1000)) => {
                    tracing::trace!("[Main Loop] Tick");
                },
                tasks_end = &mut tasks_future => {
                    tracing::info!("[Main Loop] Tasks End: {:?}", tasks_end);
                    if !self.exit_on_complete {
                        break Ok(ShutdownReason::End(tasks_end));
                    }
                },
                Some(error) = error_receiver.recv() => {
                    tracing::error!("[Main Loop] Error: {:?}", error);
                },
                Some(result) = shutdown_receiver.recv() => {
                    break result;
                },
                Some(maybe_event) = event_stream.next() => {
                    tracing::trace!("[Main Loop] Event: {:?}", maybe_event);
                    match maybe_event {
                        Ok(event) => {
                            self.handle_events(event, shutdown_sender.clone())?;
                        }
                        Err(e) => {
                            tracing::error!("[Main Loop] Event error: {:?}", e);
                            break Ok(ShutdownReason::Error(e.into()));
                        }
                    }
                }
            }
        };
        self.exit();
        result
    }

    fn draw(&mut self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn exit(&mut self) {
        self.should_be_quit = true;
    }

    fn handle_events(&mut self, event: Event, shutdown: ShutdownSender) -> color_eyre::Result<()> {
        match event {
            Event::Key(key_event) => {
                if key_event.kind == KeyEventKind::Press {
                    if let KeyCode::Char('q') = key_event.code {
                        shutdown.send(Ok(ShutdownReason::Quit))?
                    } else if let (KeyCode::Char('c'), KeyModifiers::CONTROL) =
                        (key_event.code, key_event.modifiers)
                    {
                        shutdown.send(Ok(ShutdownReason::CtrlC))?
                    } else if let (KeyCode::Char('\''), KeyModifiers::CONTROL) =
                        (key_event.code, key_event.modifiers)
                    {
                        shutdown.send(Ok(ShutdownReason::Sigquit))?
                    }
                }
            }
            Event::Mouse(mouse_event) => match mouse_event.kind {
                event::MouseEventKind::ScrollDown => {
                    tracing::info!("[Main Loop] Scroll Down");
                    self.console_states
                        .iter_mut()
                        .for_each(|state| state.mouse_event(Some(mouse_event)));
                }
                event::MouseEventKind::ScrollUp => {
                    tracing::info!("[Main Loop] Scroll Up");
                    self.console_states
                        .iter_mut()
                        .for_each(|state| state.mouse_event(Some(mouse_event)));
                }
                _ => {}
            },
            Event::Resize(_w, _h) => {
                tracing::info!("[Main Loop] Resize");
                self.console_states
                    .iter_mut()
                    .for_each(|state| state.mouse_event(None));
            }
            Event::FocusGained | Event::FocusLost => {
                tracing::info!("[Main Loop] Focus Lost");
                self.console_states
                    .iter_mut()
                    .for_each(|state| state.mouse_event(None));
            }
            _ => {}
        }
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let title = Title::from(" Parallely ".bold());
        let instructions = Title::from(Line::from(vec![" Quit ".into(), "<Q> ".blue().bold()]));
        let container = Block::default()
            .title(title.alignment(Alignment::Center))
            .title(instructions.alignment(Alignment::Right));

        let areas = Layout::horizontal(
            self.console_states
                .iter()
                .map(|_| Constraint::Fill(0))
                .collect::<Vec<_>>(),
        )
        .flex(Flex::Center)
        .split(container.inner(area));

        for (index, rect) in areas.iter().enumerate() {
            Console::new(self.error_sender.clone()).render(
                *rect,
                buf,
                &mut self.console_states[index],
            );
        }

        container.render(area, buf);
    }
}

fn init_shutdown() -> (ShutdownSender, ShutdownReceiver) {
    let (shutdown_sender, shutdown_receiver) = mpsc::unbounded_channel();
    let shutdown_sender_cloned = shutdown_sender.clone();
    tokio::spawn(async move {
        if let Err(e) = listen_for_sigint(shutdown_sender_cloned.clone()).await {
            shutdown_sender_cloned
                .send(Err(e))
                .expect("Failed to send shutdown signal");
        }
    });
    (shutdown_sender, shutdown_receiver)
}

async fn listen_for_sigint(shutdown_send: ShutdownSender) -> color_eyre::Result<()> {
    let ctrl_c_future = signal::ctrl_c();

    #[cfg(unix)]
    {
        let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())?;
        let mut quit = signal::unix::signal(signal::unix::SignalKind::quit())?;
        tokio::select! {
            _ = ctrl_c_future => {
                shutdown_send.send(Ok(ShutdownReason::Sigint))?
            },
            _ = terminate.recv() => {
                shutdown_send.send(Ok(ShutdownReason::Sigterm))?
            },
            _ = quit.recv() => {
                shutdown_send.send(Ok(ShutdownReason::Sigquit))?
            },
        }
    }

    #[cfg(not(unix))]
    ctrl_c_future.await?;

    Ok(())
}

#[derive(Debug)]
pub enum ShutdownReason {
    Sigint,
    Sigterm,
    Sigquit,
    CtrlC,
    Quit,
    End(Vec<color_eyre::Result<ParallelyResult>>),
    Error(color_eyre::Report),
}
