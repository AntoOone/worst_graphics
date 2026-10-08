use crate::renderer::{DrawingTicket, Renderer};

use std::{sync::Arc, time::Instant};
use winit::{
    application::ApplicationHandler,
    error::EventLoopError,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

struct RenderingData {
    window: Arc<Window>,
    renderer: Renderer<Window>,
}

pub trait DrawingFunction: FnMut(&mut LibConfig, &mut DrawingTicket<Window>, f32) {}

impl<T: FnMut(&mut LibConfig, &mut DrawingTicket<Window>, f32)> DrawingFunction for T {}

pub struct LibConfig {
    pub targeted_dt: f32,
}

impl Default for LibConfig {
    fn default() -> Self {
        let targeted_dt = 1.0 / 60.0;
        Self { targeted_dt }
    }
}

struct App {
    rendering_data: Option<RenderingData>,
    draw_function: Option<Box<dyn DrawingFunction>>,
    previous_draw: Instant,
    config: LibConfig,
}

impl App {
    fn new(config: LibConfig) -> Self {
        Self {
            rendering_data: Default::default(),
            draw_function: Default::default(),
            previous_draw: Instant::now(),
            config,
        }
    }
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
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                if let Some(r) = &mut self.rendering_data
                    && let Some(f) = &mut self.draw_function
                {
                    r.window.request_redraw();

                    let now = Instant::now();
                    let ellapsed = (now - self.previous_draw).as_secs_f32();
                    if ellapsed >= self.config.targeted_dt {
                        let mut ticket = r.renderer.begin_drawing();
                        f(&mut self.config, &mut ticket, ellapsed);
                        let _ = ticket.end_drawing();
                        self.previous_draw = now;
                    }
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(r) = &mut self.rendering_data {
                    r.renderer.window_resized(new_size.into());
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

impl Library {
    pub fn new(config: LibConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();

        event_loop.set_control_flow(ControlFlow::Poll);

        Library {
            app: App::new(config),
            event_loop,
        }
    }
}

impl Library {
    pub fn draw_function<F>(&mut self, func: F)
    where
        F: 'static + DrawingFunction,
    {
        self.app.draw_function = Some(Box::new(func));
    }

    pub fn run(mut self) -> Result<(), EventLoopError> {
        self.event_loop.run_app(&mut self.app)
    }
}
