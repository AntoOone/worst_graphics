use worse_graphics::{LibConfig, Library, Point};

fn main() {
    let mut library = Library::new(LibConfig::default());
    library.draw_function(move |_config, ticket, _dt| {
        ticket.draw_triangle(
            &[
                Point {
                    pos: [0.0, -0.5],
                    color: [1.0, 0.0, 0.0],
                },
                Point {
                    pos: [-0.5, 0.5],
                    color: [0.0, 0.0, 1.0],
                },
                Point {
                    pos: [0.5, 0.5],
                    color: [0.0, 1.0, 0.0],
                },
            ],
            &[[0.0, 0.0]],
        );
    });
    let _ = library.run();
}
