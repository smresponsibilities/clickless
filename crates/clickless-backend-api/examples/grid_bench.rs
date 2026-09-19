//! CPU raster timing only; excludes compositor/input latency. Never moves pointer.
use clickless_backend_api::overlay::render_frame;
use clickless_core::grid::{GridConfig, GridNavigator};
use std::{hint::black_box, time::Instant};

fn main() {
    let mut nav = GridNavigator::new(1920, 1080, GridConfig::dense());
    nav.activate();
    let frame = nav.overlay_frame().unwrap();
    let reference = render_frame(&frame).unwrap();
    let checksum = reference.pixels.iter().fold(0u64, |sum, &byte| {
        sum.wrapping_mul(31).wrapping_add(byte as u64)
    });
    let mut timings = Vec::new();
    for _ in 0..21 {
        let start = Instant::now();
        black_box(render_frame(black_box(&frame)).unwrap());
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    timings.sort_by(f64::total_cmp);
    println!(
        "1080p raster: median={:.3}ms p95={:.3}ms checksum={checksum}",
        timings[10], timings[19]
    );
    if std::env::args().any(|arg| arg == "--check-budget") {
        assert!(timings[10] < 16.667, "raster exceeds a 60Hz frame budget");
    }
}
