mod buffer;
mod renderer;
mod window;

pub use renderer::{DrawingTicket, Point};
pub use window::{LibConfig, Library};

#[derive(Clone, Copy)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub col: [f32; 3],
}

impl From<Vertex> for renderer::Vertex {
    fn from(value: Vertex) -> Self {
        renderer::Vertex {
            pos: value.pos.into(),
            color: value.col.into(),
        }
    }
}
