use crate::event::ParallelyEvent;
use crate::message::MessageSender;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::signal;

#[derive(Clone)]
pub struct ShutdownHandler {
    message_sender: MessageSender,
}

impl ShutdownHandler {
    pub fn new(message_sender: MessageSender) -> Self {
        Self { message_sender }
    }

    pub fn listen_for_signal(&self) {
        let message_sender = self.message_sender.clone();
        tokio::spawn(async move {
            if let Err(e) = Self::listen_for_signal_inner(message_sender.clone()).await {
                message_sender.send_error(e);
            }
        });
    }

    async fn listen_for_signal_inner(message_sender: MessageSender) -> color_eyre::Result<()> {
        let ctrl_c_future = signal::ctrl_c();

        #[cfg(unix)]
        {
            let mut terminate = signal::unix::signal(signal::unix::SignalKind::terminate())?;
            let mut quit = signal::unix::signal(signal::unix::SignalKind::quit())?;
            tokio::select! {
                _ = ctrl_c_future => message_sender.send_shutdown(ShutdownReason::Sigint),
                _ = terminate.recv() => message_sender.send_shutdown(ShutdownReason::Sigterm),
                _ = quit.recv() => message_sender.send_shutdown(ShutdownReason::Sigquit),
            }
        }

        #[cfg(not(unix))]
        ctrl_c_future.await?;

        Ok(())
    }

    pub fn handle_event(&mut self, event: &mut ParallelyEvent) {
        if let Event::Key(KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            ..
        }) = event.as_ref()
        {
            match (code, modifiers) {
                (KeyCode::Char('q'), _) => {
                    self.message_sender.send_shutdown(ShutdownReason::Quit);
                    event.stop_propagation();
                }
                (KeyCode::Char('c'), &KeyModifiers::CONTROL) => {
                    self.message_sender.send_shutdown(ShutdownReason::CtrlC);
                    event.stop_propagation();
                }
                (KeyCode::Char('\\'), &KeyModifiers::CONTROL) => {
                    self.message_sender.send_shutdown(ShutdownReason::Sigquit);
                    event.stop_propagation();
                }
                _ => {}
            };
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ShutdownReason {
    Sigint,
    Sigterm,
    Sigquit,
    CtrlC,
    Quit,
    End,
}
