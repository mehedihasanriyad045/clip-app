use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Longest edge of a list thumbnail, in pixels.
const THUMB_MAX: u32 = 72;

/// Clipboard images above this many pixels are skipped. A 50 MP screenshot
/// would otherwise stall the poll loop and bloat the history folder.
const MAX_PIXELS: usize = 40_000_000;

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

pub fn images_dir(app_data: &Path) -> PathBuf {
    app_data.join("images")
}

/// Writes clipboard RGBA pixels to a PNG file named after their hash.
/// Returns the path, or None if the image is unusable or too large.
pub fn save_png(
    app_data: &Path,
    width: usize,
    height: usize,
    rgba: &[u8],
    hash: &str,
) -> Option<PathBuf> {
    if width == 0 || height == 0 || width.saturating_mul(height) > MAX_PIXELS {
        return None;
    }

    let dir = images_dir(app_data);
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join(format!("{hash}.png"));

    // Same image copied twice: the file is already there.
    if path.exists() {
        return Some(path);
    }

    let buf = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec())?;
    image::DynamicImage::ImageRgba8(buf).save(&path).ok()?;
    Some(path)
}

/// Loads a PNG and returns its RGBA pixels, for putting back on the clipboard.
pub fn load_rgba(path: &Path) -> Option<(usize, usize, Vec<u8>)> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    Some((w, h, img.into_raw()))
}

/// Builds a small PNG data URL for the list, so the UI can show a preview
/// without the asset protocol or reading the full-size file.
pub fn thumbnail_data_url(path: &Path) -> Option<String> {
    let img = image::open(path).ok()?.thumbnail(THUMB_MAX, THUMB_MAX);
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{b64}"))
}
