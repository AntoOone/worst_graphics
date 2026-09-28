use std::sync::Arc;

use crate::renderer::{DrawingTicket, Renderer};
use winit::error::EventLoopError;
use winit::keyboard::Key;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct RenderingData {
    window: Arc<Window>,
    renderer: Renderer<Window>,
}

type DrawingFunction = dyn Fn(&mut DrawingTicket<Window>);

#[derive(Default)]
struct App {
    rendering_data: Option<RenderingData>,
    draw_function: Option<Box<DrawingFunction>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(Window::default_attributes())
            .unwrap();
        let window = Arc::new(window);
        let renderer = Renderer::new(
            window.clone(),
            window.inner_size().into(),
            cfg!(debug_assertions),
            2,
        )
        .unwrap();
        self.rendering_data = Some(RenderingData { window, renderer });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match &event {
            WindowEvent::CloseRequested => {
                println!("The close button was pressed; stopping");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(r) = &mut self.rendering_data {
                    if let Some(f) = &self.draw_function {
                        let mut ticket = r.renderer.begin_drawing();
                        f(&mut ticket);
                        let _ = ticket.end_drawing();
                    }
                    r.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput {
                device_id: _,
                event,
                is_synthetic: _,
            } if let Key::Character(c) = &event.logical_key
                && event.state == ElementState::Pressed =>
            {
                println!("character pressed : {}", c);
            }
            WindowEvent::Resized(new_size) => {
                if let Some(r) = &mut self.rendering_data {
                    r.renderer.window_resized(new_size.into()).unwrap();
                }
            }
            _ => (),
        }
    }
}

pub struct Library {
    app: App,
    event_loop: EventLoop<()>,
}

impl Default for Library {
    fn default() -> Self {
        let event_loop = EventLoop::new().unwrap();

        event_loop.set_control_flow(ControlFlow::Wait);

        Library {
            app: App::default(),
            event_loop,
        }
    }
}

impl Library {
    pub fn draw_function<F>(&mut self, func: F)
    where
        F: 'static + Fn(&mut DrawingTicket<Window>),
    {
        self.app.draw_function = Some(Box::new(func));
    }

    pub fn run(mut self) -> Result<(), EventLoopError> {
        self.event_loop.run_app(&mut self.app)
    }
}
