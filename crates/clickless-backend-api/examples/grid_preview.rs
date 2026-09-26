//! Writes a preview from the same rasterizer used by native overlays. No input injection.
use clickless_backend_api::overlay::render_frame;
use clickless_core::{
    LogicalKey,
    grid::{GridConfig, GridNavigator},
};
use std::{fs, io};

fn main() -> io::Result<()> {
    let mut nav = GridNavigator::new(1920, 1080, GridConfig::dense());
    nav.activate();
    // --selected: narrow to the subgrid (level 2).
    // --bank: one outer key only, the view that carries help.
    if std::env::args().any(|arg| arg == "--selected") {
        nav.on_key_press(LogicalKey::F);
        nav.on_key_release(LogicalKey::F);
        nav.on_key_press(LogicalKey::G);
    }
    if std::env::args().any(|arg| arg == "--bank") {
        nav.on_key_press(LogicalKey::F);
    }
    let target = render_frame(&nav.overlay_frame().unwrap()).unwrap();
    let size = 54 + target.pixels.len() as u32;
    let mut bmp = Vec::with_capacity(size as usize);
    bmp.extend(b"BM");
    bmp.extend(size.to_le_bytes());
    bmp.extend([0; 4]);
    bmp.extend(54u32.to_le_bytes());
    bmp.extend(40u32.to_le_bytes());
    bmp.extend(target.width.to_le_bytes());
    bmp.extend(target.height.to_le_bytes());
    bmp.extend(1u16.to_le_bytes());
    bmp.extend(32u16.to_le_bytes());
    bmp.extend([0; 24]);
    for row in target.pixels.chunks_exact(target.width as usize * 4).rev() {
        for pixel in row.as_chunks::<4>().0 {
            // Composite on a light background to check label contrast.
            let background = 240u16 * (255 - pixel[3] as u16) / 255;
            for channel in &pixel[..3] {
                bmp.push((*channel as u16 + background).min(255) as u8);
            }
            bmp.push(255);
        }
    }
    let path = if std::env::args().any(|arg| arg == "--selected") {
        "grid-selected.bmp"
    } else if std::env::args().any(|arg| arg == "--bank") {
        "grid-bank.bmp"
    } else {
        "grid-preview.bmp"
    };
    fs::write(path, bmp)?;
    println!("{path}");
    Ok(())
}
