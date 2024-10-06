use crossterm::event::MouseEvent;

#[derive(Default)]
pub struct Context {
    pub event_chunk: Vec<crossterm::event::Event>,
}

impl Context {
    pub fn try_as_mouse_events(&self) -> impl Iterator<Item = &MouseEvent> {
        self.event_chunk.iter().flat_map(|event| match event {
            crossterm::event::Event::Mouse(event) => Some(event),
            _ => None,
        })
    }
}
