fn main() {
    ensure_windows_icon();
    tauri_build::build()
}

fn ensure_windows_icon() {
    use std::{fs, path::Path};

    let path = Path::new("icons/icon.ico");
    if path.exists() {
        return;
    }

    const WIDTH: usize = 64;
    const HEIGHT: usize = 64;
    const MASK_BYTES: usize = (WIDTH / 8) * HEIGHT;
    let image_bytes = 40 + WIDTH * HEIGHT * 4 + MASK_BYTES;
    let mut icon = Vec::with_capacity(22 + image_bytes);

    icon.extend_from_slice(&0u16.to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&[WIDTH as u8, HEIGHT as u8, 0, 0]);
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&(image_bytes as u32).to_le_bytes());
    icon.extend_from_slice(&22u32.to_le_bytes());

    icon.extend_from_slice(&40u32.to_le_bytes());
    icon.extend_from_slice(&(WIDTH as i32).to_le_bytes());
    icon.extend_from_slice(&((HEIGHT * 2) as i32).to_le_bytes());
    icon.extend_from_slice(&1u16.to_le_bytes());
    icon.extend_from_slice(&32u16.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&((WIDTH * HEIGHT * 4) as u32).to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0i32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());
    icon.extend_from_slice(&0u32.to_le_bytes());

    for y in (0..HEIGHT).rev() {
        for x in 0..WIDTH {
            let dx = x as f32 - 31.5;
            let dy = y as f32 - 31.5;
            let distance = (dx * dx + dy * dy).sqrt();
            let ring = (24.0..=28.0).contains(&distance);
            let bar = matches!(x, 24..=27 | 30..=33 | 36..=39)
                && (18 + x.abs_diff(31) / 2..=45 - x.abs_diff(31) / 2).contains(&y);
            let (red, green, blue) = if ring || bar {
                (120u8, 214u8, 204u8)
            } else {
                (16u8, 20u8, 17u8)
            };
            icon.extend_from_slice(&[blue, green, red, 255]);
        }
    }
    icon.resize(icon.len() + MASK_BYTES, 0);

    fs::create_dir_all("icons").expect("failed to create icon directory");
    fs::write(path, icon).expect("failed to generate Windows icon");
}
