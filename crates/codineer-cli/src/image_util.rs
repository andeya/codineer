use std::path::Path;

use base64::Engine;

const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;

pub(crate) fn is_image_path(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp")
    )
}

/// Detect MIME type from magic bytes.
pub(crate) fn detect_media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() < 4 {
        return None;
    }
    if bytes.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    }
}

/// Infer MIME type from file extension (fallback when magic bytes fail).
pub(crate) fn media_type_from_extension(path: &Path) -> Option<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => Some("image/png"),
        Some("jpg" | "jpeg") => Some("image/jpeg"),
        Some("gif") => Some("image/gif"),
        Some("webp") => Some("image/webp"),
        Some("bmp") => Some("image/bmp"),
        _ => None,
    }
}

/// Read an image file and return a `ContentBlock::Image`.
pub(crate) fn read_image_as_block(
    path: &Path,
) -> Result<runtime::ContentBlock, Box<dyn std::error::Error>> {
    let file_len = std::fs::metadata(path)?.len();
    if file_len > MAX_IMAGE_BYTES {
        return Err(size_error_msg(file_len).into());
    }
    let bytes = std::fs::read(path)?;
    let media_type = detect_media_type(&bytes)
        .or_else(|| media_type_from_extension(path))
        .ok_or("unrecognised image format")?;
    Ok(image_block(media_type, &bytes))
}

/// Encode raw bytes into a `ContentBlock::Image`.
pub(crate) fn bytes_to_image_block(
    bytes: &[u8],
    fallback_media_type: Option<&str>,
) -> Result<runtime::ContentBlock, Box<dyn std::error::Error>> {
    let len = bytes.len() as u64;
    if len > MAX_IMAGE_BYTES {
        return Err(size_error_msg(len).into());
    }
    let media_type = detect_media_type(bytes)
        .or(fallback_media_type)
        .ok_or("unrecognised image format")?;
    Ok(image_block(media_type, bytes))
}

fn image_block(media_type: &str, bytes: &[u8]) -> runtime::ContentBlock {
    runtime::ContentBlock::Image {
        media_type: media_type.to_string(),
        data: base64::engine::general_purpose::STANDARD.encode(bytes),
    }
}

fn size_error_msg(len: u64) -> String {
    format!(
        "image too large ({:.1} MB, max {:.0} MB)",
        len as f64 / 1_048_576.0,
        MAX_IMAGE_BYTES as f64 / 1_048_576.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_media_type_by_magic_bytes() {
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0; 4]);
        webp.extend_from_slice(b"WEBP");

        assert_eq!(detect_media_type(b"\x89PNG\r\n\x1a\n"), Some("image/png"));
        assert_eq!(
            detect_media_type(&[0xFF, 0xD8, 0xFF, 0xE0]),
            Some("image/jpeg")
        );
        assert_eq!(detect_media_type(b"GIF89a"), Some("image/gif"));
        assert_eq!(detect_media_type(&webp), Some("image/webp"));
        assert_eq!(detect_media_type(b"BM\x00\x00"), Some("image/bmp"));
        assert_eq!(detect_media_type(b"\x00\x00\x00\x00"), None);
        assert_eq!(detect_media_type(b"AB"), None); // too short
    }

    #[test]
    fn extension_fallback_covers_all_formats() {
        assert_eq!(
            media_type_from_extension(Path::new("img.PNG")),
            Some("image/png")
        );
        assert_eq!(
            media_type_from_extension(Path::new("img.JPG")),
            Some("image/jpeg")
        );
        assert_eq!(
            media_type_from_extension(Path::new("img.jpeg")),
            Some("image/jpeg")
        );
        assert_eq!(
            media_type_from_extension(Path::new("img.gif")),
            Some("image/gif")
        );
        assert_eq!(
            media_type_from_extension(Path::new("img.webp")),
            Some("image/webp")
        );
        assert_eq!(
            media_type_from_extension(Path::new("img.bmp")),
            Some("image/bmp")
        );
        assert_eq!(media_type_from_extension(Path::new("img.txt")), None);
    }

    #[test]
    fn is_image_path_recognises_extensions() {
        assert!(is_image_path(Path::new("a.png")));
        assert!(is_image_path(Path::new("b.JPEG")));
        assert!(!is_image_path(Path::new("c.txt")));
        assert!(!is_image_path(Path::new("d.svg")));
    }

    #[test]
    fn bytes_to_image_block_variants() {
        let png = b"\x89PNG\r\n\x1a\nfakedata";

        // Detects from magic bytes
        let block = bytes_to_image_block(png, None).unwrap();
        assert!(
            matches!(block, runtime::ContentBlock::Image { ref media_type, .. } if media_type == "image/png")
        );

        // Uses fallback when unrecognised
        let block = bytes_to_image_block(b"raw", Some("image/png")).unwrap();
        assert!(
            matches!(block, runtime::ContentBlock::Image { ref media_type, .. } if media_type == "image/png")
        );

        // Rejects unknown without fallback
        let err = bytes_to_image_block(b"raw", None).unwrap_err();
        assert!(err.to_string().contains("unrecognised"));
    }
}
