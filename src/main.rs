use worse_graphics::{Library, Point};

fn main() {
    let mut library = Library::default();
    library.draw_function(|ticket| {
        ticket.draw_triangle([
            Point {
                pos: [-0.5, -0.5],
                color: [1.0, 0.0, 0.0],
            },
            Point {
                pos: [0.5, -0.5],
                color: [0.0, 1.0, 0.0],
            },
            Point {
                pos: [0.5, 0.5],
                color: [0.0, 0.0, 1.0],
            },
        ]);
        ticket.draw_triangle([
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
        ]);
    });
    let _ = library.run();
}
