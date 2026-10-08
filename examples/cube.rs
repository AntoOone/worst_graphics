use ::core::f32;
use glam::{camera::rh::proj::vulkan::*, *};
use worse_graphics::{LibConfig, Library, Vertex};

fn main() {
    let colr = [0.9, 0.3, 0.3];
    let colg = [0.0, 0.7, 0.3];
    let colb = [0.1, 0.4, 0.8];
    let colw = [0.8, 0.8, 0.8];
    let vertices = [
        Vertex {
            pos: [1.0, 1.0, 1.0],
            col: colr,
        },
        Vertex {
            pos: [-1.0, 1.0, 1.0],
            col: colg,
        },
        Vertex {
            pos: [-1.0, 1.0, -1.0],
            col: colb,
        },
        Vertex {
            pos: [1.0, 1.0, -1.0],
            col: colw,
        },
        Vertex {
            pos: [1.0, -1.0, 1.0],
            col: colw,
        },
        Vertex {
            pos: [-1.0, -1.0, 1.0],
            col: colb,
        },
        Vertex {
            pos: [-1.0, -1.0, -1.0],
            col: colr,
        },
        Vertex {
            pos: [1.0, -1.0, -1.0],
            col: colg,
        },
    ];
    let triangles = [
        [0, 3, 2],
        [0, 2, 1],
        [0, 1, 5],
        [0, 5, 4],
        [0, 4, 7],
        [0, 7, 3],
        [6, 5, 1],
        [6, 1, 2],
        [6, 2, 3],
        [6, 3, 7],
        [6, 7, 4],
        [6, 4, 5],
    ];
    let instances = [[0.0, 0.0, 0.0]];
    let mut library = Library::new(LibConfig::default());
    let start_time = std::time::Instant::now();
    library.draw_function(move |_config, ticket, _dt| {
        ticket.draw_mesh(&vertices, &triangles, &instances);
        let now = std::time::Instant::now();
        let time = now - start_time;
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
