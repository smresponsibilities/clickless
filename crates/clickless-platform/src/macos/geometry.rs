/// Core/Quartz desktop coordinates use a top-left origin in logical points.
/// AppKit uses a bottom-left origin anchored to the primary screen.
pub(super) fn appkit_top_left(origin: (i64, i64), primary_top: f64) -> (f64, f64) {
    (origin.0 as f64, primary_top - origin.1 as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_and_secondary_desktop_origins_convert_in_logical_points() {
        assert_eq!(appkit_top_left((0, 0), 900.0), (0.0, 900.0));
        assert_eq!(appkit_top_left((-1200, -400), 900.0), (-1200.0, 1300.0));
        assert_eq!(appkit_top_left((1440, 200), 900.0), (1440.0, 700.0));
    }
}
