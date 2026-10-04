use ::core::f32;

use glam::{camera::rh::proj::vulkan::*, *};
use worse_graphics::{Library, Point};

fn main() {
    let mut library = Library::default();
    let start_time = std::time::Instant::now();
    library.draw_function(move |ticket| {
        ticket.draw_triangle(
            &[
                Point {
                    pos: [0.5, 0.5],
                    color: [0.0, 0.0, 1.0],
                },
                Point {
                    pos: [-0.5, -0.5],
                    color: [1.0, 0.0, 0.0],
                },
                Point {
                    pos: [-0.5, 0.5],
                    color: [1.0, 1.0, 1.0],
                },
            ],
            &[[-0.5, -0.5], [0.5, 0.5]],
        );
        ticket.draw_triangle(
            &[
                Point {
                    pos: [0.5, 0.5],
                    color: [0.0, 0.0, 1.0],
                },
                Point {
                    pos: [-0.5, 0.5],
                    color: [1.0, 1.0, 1.0],
                },
                Point {
                    pos: [-0.5, -0.5],
                    color: [1.0, 0.0, 0.0],
                },
            ],
            &[[-0.5, -0.5], [0.5, 0.5]],
        );

        let time = std::time::Instant::now() - start_time;
        let a = ticket.width() as f32 / ticket.height() as f32;
        let pos = Vec3::new(0.0, 0.0, 3.0);
        let camera = ticket.get_camera();
        camera.model = Mat4::from_rotation_translation(
            Quat::from_rotation_y(time.as_secs_f32())
                * Quat::from_rotation_z(0.5 * time.as_secs_f32()),
            -pos,
        );
        camera.proj = perspective(90., a, 1.0, 10.0);
    });
    let _ = library.run();
}
