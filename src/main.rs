use std::sync::Arc;

use winit::keyboard::Key;
use worse_graphics::Renderer;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

struct RenderingData {
    window: Arc<Window>,
    renderer: Renderer<Window>,
}

#[derive(Default)]
struct App {
    rendering_data: Option<RenderingData>,
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
                    r.renderer.draw_frame().unwrap();
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

fn main() {
    let event_loop = EventLoop::new().unwrap();

    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
