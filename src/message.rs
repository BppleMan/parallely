use crate::event::ParallelyEvent;
use crate::shutdown_handler::ShutdownReason;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::sync::mpsc::error::SendError;
use tokio_stream::wrappers::UnboundedReceiverStream;

pub fn message_queue() -> (MessageSender, MessageStream) {
    let (message_sender, message_receiver) = tokio::sync::mpsc::unbounded_channel();
    let message_sender = MessageSender::new(message_sender);
    let message_stream = MessageStream::new(message_receiver);
    (message_sender, message_stream)
}

#[derive(Debug)]
pub enum Message {
    Error(color_eyre::Report),
    Shutdown(ShutdownReason),
    EventChunk(Vec<ParallelyEvent>),
    Update,
}

impl From<ShutdownReason> for Message {
    fn from(value: ShutdownReason) -> Self {
        Self::Shutdown(value)
    }
}

impl From<color_eyre::Report> for Message {
    fn from(value: color_eyre::Report) -> Self {
        Self::Error(value)
    }
}

impl From<Vec<ParallelyEvent>> for Message {
    fn from(value: Vec<ParallelyEvent>) -> Self {
        Self::EventChunk(value)
    }
}

#[derive(Clone)]
pub struct MessageSender {
    inner: tokio::sync::mpsc::UnboundedSender<Message>,
}

impl MessageSender {
    fn new(inner: tokio::sync::mpsc::UnboundedSender<Message>) -> Self {
        Self { inner }
    }

    pub fn send<T>(&self, message: T) -> color_eyre::Result<(), SendError<Message>>
    where
        T: Into<Message>,
    {
        self.inner.send(message.into())
    }

    pub fn send_error<E>(&self, error: E)
    where
        E: Into<color_eyre::Report>,
    {
        if let Err(e) = self.send(error.into()) {
            panic!("Failed to send error: {}", e);
        }
    }

    pub fn send_shutdown(&self, reason: ShutdownReason) {
        if let Err(e) = self.send(reason) {
            self.send_error(e);
        }
    }

    pub fn send_event_chunk(&self, events: Vec<ParallelyEvent>) {
        if let Err(e) = self.send(events) {
            self.send_error(e);
        }
    }

    pub fn need_update(&self) {
        if let Err(e) = self.send(Message::Update) {
            self.send_error(e);
        }
    }
}

pub struct MessageStream {
    message_stream: UnboundedReceiverStream<Message>,
}

impl MessageStream {
    pub fn new(message_stream: tokio::sync::mpsc::UnboundedReceiver<Message>) -> Self {
        let message_stream = UnboundedReceiverStream::new(message_stream);
        Self { message_stream }
    }
}

impl Stream for MessageStream {
    type Item = Message;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().message_stream).poll_next(cx)
    }
}
