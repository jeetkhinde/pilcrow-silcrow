use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use image::imageops::FilterType;
use sha2::{Digest, Sha256};

pub struct TransformParams<'a> {
    pub src: &'a str,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub quality: u8,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Webp,
    Jpeg,
    Png,
    Auto,
}

impl OutputFormat {
    pub fn from_str(s: &str) -> Self {
        match s {
            "webp" => Self::Webp,
            "jpeg" | "jpg" => Self::Jpeg,
            "png" => Self::Png,
            _ => Self::Auto,
        }
    }

    pub fn resolve(self, preferred: &[String]) -> Self {
        if self != Self::Auto {
            return self;
        }
        for fmt in preferred {
            match fmt.as_str() {
                "webp" => return Self::Webp,
                "jpeg" | "jpg" => return Self::Jpeg,
                "png" => return Self::Png,
                _ => {}
            }
        }
        Self::Jpeg
    }

    pub fn mime_type(self) -> &'static str {
        match self {
            Self::Webp => "image/webp",
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Auto => "image/jpeg",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Webp => "webp",
            Self::Jpeg => "jpeg",
            Self::Png => "png",
            Self::Auto => "jpeg",
        }
    }
}

/// Derive a cache-shard path for a set of transform parameters.
/// Uses first 2 hex chars as directory prefix (256 shards, avoids inode exhaustion).
pub fn cache_path(cache_dir: &str, params: &TransformParams) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(params.src.as_bytes());
    hasher.update(b":");
    hasher.update(params.width.unwrap_or(0).to_le_bytes());
    hasher.update(b":");
    hasher.update(params.height.unwrap_or(0).to_le_bytes());
    hasher.update(b":");
    hasher.update([params.quality]);
    hasher.update(b":");
    hasher.update(format!("{:?}", params.format).as_bytes());
    let hash = hex::encode(hasher.finalize());
    let ext = params.format.extension();
    Path::new(cache_dir)
        .join(&hash[..2])
        .join(format!("{}.{}", &hash[2..], ext))
}

/// Transform an image buffer and return (bytes, mime_type).
pub fn transform(
    input: &[u8],
    width: Option<u32>,
    height: Option<u32>,
    quality: u8,
    format: OutputFormat,
    max_width: u32,
    max_height: u32,
) -> anyhow::Result<(Vec<u8>, &'static str)> {
    let img = image::load_from_memory(input)?;
    let img = resize(img, width, height, max_width, max_height);

    let mut out = Cursor::new(Vec::new());
    match format {
        OutputFormat::Webp => {
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
            img.write_with_encoder(encoder)?;
        }
        OutputFormat::Jpeg | OutputFormat::Auto => {
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            img.write_with_encoder(encoder)?;
        }
        OutputFormat::Png => {
            let encoder = image::codecs::png::PngEncoder::new(&mut out);
            img.write_with_encoder(encoder)?;
        }
    }

    Ok((out.into_inner(), format.mime_type()))
}

fn resize(
    img: DynamicImage,
    target_w: Option<u32>,
    target_h: Option<u32>,
    max_w: u32,
    max_h: u32,
) -> DynamicImage {
    let orig_w = img.width();
    let orig_h = img.height();

    let w = target_w.unwrap_or(orig_w).min(max_w);
    let h = target_h.unwrap_or(orig_h).min(max_h);

    if w == orig_w && h == orig_h {
        return img;
    }

    // Maintain aspect ratio when only one dimension is specified.
    let (nw, nh) = if target_w.is_some() && target_h.is_none() {
        let ratio = w as f32 / orig_w as f32;
        (w, (orig_h as f32 * ratio) as u32)
    } else if target_h.is_some() && target_w.is_none() {
        let ratio = h as f32 / orig_h as f32;
        ((orig_w as f32 * ratio) as u32, h)
    } else {
        (w, h)
    };

    img.resize(nw, nh, FilterType::Lanczos3)
}
