/// Attempt to read an image from the system clipboard.
///
/// Uses the `arboard` crate which calls native platform APIs:
/// - macOS: NSPasteboard (no special permissions required)
/// - Linux: X11 / Wayland
/// - Windows: Win32 clipboard
///
/// Returns `Ok((bytes, media_type))` when an image is found, or
/// `Err(reason)` with a human-readable explanation when nothing is available.
///
/// Two sources are tried in order:
/// 1. Direct image data in the clipboard (e.g. screenshot, Cmd+C in Preview)
/// 2. Text in the clipboard that is a path to an image file (e.g. Cmd+C on a
///    file in Finder — the terminal pastes the file path as text, but Ctrl+V
///    reads it here and loads the image bytes directly)
pub(crate) fn read_clipboard_image() -> Result<(Vec<u8>, &'static str), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?;

    // --- Try 1: raw image data ---
    match clipboard.get_image() {
        Ok(img) => {
            return encode_rgba_to_png(img.width, img.height, &img.bytes)
                .map(|png| (png, "image/png"))
                .ok_or_else(|| "failed to encode clipboard image as PNG".to_string());
        }
        Err(arboard::Error::ContentNotAvailable) => {}
        Err(e) => return Err(format!("clipboard read error: {e}")),
    }

    // --- Try 2: clipboard text that is an image file path ---
    if let Ok(text) = clipboard.get_text() {
        let trimmed = text.trim();
        if !trimmed.is_empty() && !trimmed.contains('\n') {
            let path = std::path::Path::new(trimmed);
            if crate::image_util::is_image_path(path) && path.exists() {
                let bytes = std::fs::read(path)
                    .map_err(|e| format!("cannot read image file '{trimmed}': {e}"))?;
                let media_type = crate::image_util::detect_media_type(&bytes)
                    .or_else(|| crate::image_util::media_type_from_extension(path))
                    .unwrap_or("image/png");
                return Ok((bytes, media_type));
            }
        }
    }

    Err("no image in clipboard (copy an image with Cmd+C, then press Ctrl+V)".to_string())
}

fn encode_rgba_to_png(width: usize, height: usize, bytes: &[u8]) -> Option<Vec<u8>> {
    let mut buf = Vec::new();
    let mut encoder = png::Encoder::new(&mut buf, width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().ok()?;
    writer.write_image_data(bytes).ok()?;
    drop(writer);
    Some(buf)
}
