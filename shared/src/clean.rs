//! Preprocessing of copied HTML and its images before anything is stored.
//!
//! The result is a display-only copy of the HTML for the History viewer:
//! - `url` admits only `https:` and `data:` image sources.
//! - `image` decodes each image with memory-safe Rust decoders, under size
//!   limits that stop decompression bombs, and re-encodes it. Only pixels
//!   survive: metadata (EXIF, GPS) and anything appended to the file are gone.
//! - `resvg` rasterises SVG to PNG. Scripts never run and `<image>` links to
//!   files or the network are refused.
//! - `ammonia` rebuilds the HTML from an allowlist (no scripts, event
//!   handlers, frames or forms; only harmless inline CSS) and swaps each image
//!   for its cleaned `data:` copy.
//!
//! Pasting never uses this copy: the original formats go back untouched.

use std::{
    borrow::Cow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::Cursor,
    sync::{Arc, Mutex},
};

use ammonia::UrlRelative;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, codecs::jpeg::JpegEncoder};
use resvg::{tiny_skia, usvg};
use url::Url;

/// The format the display copy is stored under. Klippy's own; never written
/// to the clipboard.
pub const DISPLAY_HTML_KIND: &str = "com.kksaisub.klippy.display-html";

/// At most this many images per copy are downloaded and inlined.
pub const MAX_IMAGES: usize = 20;
/// Largest image file accepted, downloaded or inline.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
/// Images are shrunk to fit this on their longer side.
const MAX_PIXEL_SIZE: u32 = 1600;
/// Keeps the display copy under the store's 10 MB per-format limit.
const MAX_INLINED_BYTES: usize = 8 * 1024 * 1024;
/// Decoding limits: refuse absurd dimensions or allocations up front.
const MAX_DECODE_DIMENSION: u32 = 16_384;
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

/// Inline CSS that can style text and layout but can't load anything (no
/// `background-image`, `content`, `cursor`, … which take URLs).
const STYLE_PROPERTIES: &[&str] = &[
    "color",
    "background-color",
    "font",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "font-variant",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "text-align",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-style",
    "text-indent",
    "text-transform",
    "white-space",
    "vertical-align",
    "margin",
    "margin-top",
    "margin-right",
    "margin-bottom",
    "margin-left",
    "padding",
    "padding-top",
    "padding-right",
    "padding-bottom",
    "padding-left",
    "border",
    "border-top",
    "border-right",
    "border-bottom",
    "border-left",
    "border-color",
    "border-style",
    "border-width",
    "border-collapse",
    "border-spacing",
    "border-radius",
    "width",
    "height",
    "max-width",
    "display",
    "list-style-type",
];

/// The allowlist both passes share.
fn sanitizer() -> ammonia::Builder<'static> {
    let mut builder = ammonia::Builder::default();
    builder
        .add_tags(&["font"])
        .add_tag_attributes("font", &["color", "face", "size"])
        .add_generic_attributes(&[
            "style", "align", "valign", "width", "height", "bgcolor", "dir",
        ])
        .filter_style_properties(STYLE_PROPERTIES.iter().copied().collect())
        // `data:` is for the inlined images; links using it are dropped below.
        .url_schemes(HashSet::from(["https", "http", "mailto", "data"]))
        .url_relative(UrlRelative::Deny);
    builder
}

/// The `src` of each `<img>` that survives sanitising, deduplicated, in
/// document order, at most `MAX_IMAGES`.
pub fn image_sources(html: &str) -> Vec<String> {
    let found = Arc::new(Mutex::new(Vec::<String>::new()));
    let collect = Arc::clone(&found);
    sanitizer()
        .attribute_filter(move |element, attribute, value| {
            if (element, attribute) == ("img", "src") {
                let mut found = collect
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if found.len() < MAX_IMAGES && !found.iter().any(|seen| seen == value) {
                    found.push(value.to_owned());
                }
            }
            Some(Cow::Borrowed(value))
        })
        .clean(html);
    let found = found
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    found.clone()
}

/// Signs of an attack in the original HTML, as reasons to show the user.
/// Sanitising would remove all of these, but their presence means the copy
/// was crafted to run code, so it isn't saved at all.
pub fn threats(html: &str) -> Vec<String> {
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    let tokenizer = Tokenizer::new(ThreatScan::default(), TokenizerOpts::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.found.take()
}

/// Looks at every start tag the HTML tokenizer sees, with attribute values
/// already decoded as a browser would (`&#x09;` etc.).
#[derive(Default)]
struct ThreatScan {
    found: RefCell<Vec<String>>,
}

impl TokenSink for ThreatScan {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        let Token::TagToken(tag) = token else {
            return TokenSinkResult::Continue;
        };
        if tag.kind != TagKind::StartTag {
            return TokenSinkResult::Continue;
        }
        let found = &mut self.found.borrow_mut();
        let name = tag.name.as_ref();
        let reason = match name {
            "script" => Some("Contains a script".to_owned()),
            "iframe" | "frame" | "frameset" => Some("Embeds another web page".to_owned()),
            "object" | "embed" | "applet" => Some("Embeds a plug-in or file".to_owned()),
            "base" => Some("Rewrites where links point".to_owned()),
            _ => None,
        };
        report(found, reason);
        for attr in &tag.attrs {
            let attribute = (&*attr.name.local).to_ascii_lowercase();
            let value: &str = &attr.value;
            if attribute.starts_with("on") {
                report(found, Some(format!("Runs code on an event ({attribute})")));
            } else if is_script_url(value) {
                report(
                    found,
                    Some("Contains a link that runs code (javascript:)".to_owned()),
                );
            } else if name == "meta"
                && attribute == "http-equiv"
                && value.eq_ignore_ascii_case("refresh")
            {
                report(found, Some("Redirects to another page".to_owned()));
            }
        }
        TokenSinkResult::Continue
    }
}

fn report(found: &mut Vec<String>, reason: Option<String>) {
    if let Some(reason) = reason
        && !found.contains(&reason)
    {
        found.push(reason);
    }
}

/// `javascript:`/`vbscript:` URLs, and `data:` URLs carrying a page, even when
/// disguised with case, spaces or control characters as browsers allow.
fn is_script_url(value: &str) -> bool {
    let compact = value
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    compact.starts_with("javascript:")
        || compact.starts_with("vbscript:")
        || compact.starts_with("data:text/html")
        || compact.starts_with("data:application/xhtml")
}

/// The sources the shell has to download: the `https:` ones.
pub fn remote_sources(sources: &[String]) -> Vec<String> {
    sources
        .iter()
        .filter(|source| Url::parse(source).is_ok_and(|url| url.scheme() == "https"))
        .cloned()
        .collect()
}

pub struct Display {
    pub html: String,
    /// Images that were attacks, not merely unusable (see `ImageIssue`).
    pub threats: Vec<String>,
}

/// Builds the display copy. `downloaded` maps remote sources to their bytes;
/// `data:` sources are decoded here.
pub fn display_html(html: &str, downloaded: &HashMap<String, Vec<u8>>) -> Display {
    let mut inlined = HashMap::new();
    let mut threats = Vec::new();
    let mut total = 0;
    for source in image_sources(html) {
        let raw = match Url::parse(&source) {
            Ok(url) if url.scheme() == "data" => decode_data_url(&source),
            Ok(url) if url.scheme() == "https" => downloaded.get(&source).cloned(),
            _ => None,
        };
        let Some(raw) = raw.filter(|raw| raw.len() <= MAX_IMAGE_BYTES) else {
            continue;
        };
        let image = match clean_image(&raw) {
            Ok(image) => image,
            Err(ImageIssue::Malicious(reason)) => {
                report(&mut threats, Some(reason.to_owned()));
                continue;
            }
            Err(ImageIssue::Unusable) => continue,
        };
        let uri = format!("data:{};base64,{}", image.mime, BASE64.encode(&image.data));
        if total + uri.len() > MAX_INLINED_BYTES {
            continue;
        }
        total += uri.len();
        inlined.insert(source, uri);
    }

    let html = sanitizer()
        .attribute_filter(
            move |element, attribute, value| match (element, attribute) {
                // Only cleaned copies; anything else (failed, too big, http, …) goes.
                ("img", "src") => inlined.get(value).map(|uri| Cow::Owned(uri.clone())),
                (_, "href") if value.trim_start().to_ascii_lowercase().starts_with("data:") => None,
                _ => Some(Cow::Borrowed(value)),
            },
        )
        .clean(html)
        .to_string();
    Display { html, threats }
}

/// The bytes of a `data:` URL, base64 or percent-encoded.
fn decode_data_url(source: &str) -> Option<Vec<u8>> {
    let (header, payload) = source.strip_prefix("data:")?.split_once(',')?;
    if header.to_ascii_lowercase().ends_with(";base64") {
        let compact: String = payload
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect();
        BASE64.decode(compact).ok()
    } else {
        Some(percent_encoding::percent_decode_str(payload).collect())
    }
}

#[derive(Debug)]
pub struct CleanImage {
    pub data: Vec<u8>,
    pub mime: &'static str,
}

#[derive(Debug, PartialEq)]
pub enum ImageIssue {
    /// Built to attack: a decompression bomb, or an SVG carrying script.
    Malicious(&'static str),
    /// Broken, unsupported (e.g. HEIC) or not an image; just left out.
    Unusable,
}

/// Decodes and re-encodes one image: PNG if it has transparency, else JPEG.
pub fn clean_image(bytes: &[u8]) -> Result<CleanImage, ImageIssue> {
    let image = if looks_like_svg(bytes) {
        let text = String::from_utf8_lossy(bytes).to_ascii_lowercase();
        if text.contains("<script") || text.contains("javascript:") {
            return Err(ImageIssue::Malicious(
                "Has an SVG image containing a script",
            ));
        }
        rasterize_svg(bytes).ok_or(ImageIssue::Unusable)?
    } else {
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| ImageIssue::Unusable)?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_DECODE_DIMENSION);
        limits.max_image_height = Some(MAX_DECODE_DIMENSION);
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);
        reader.decode().map_err(|error| match error {
            image::ImageError::Limits(_) => {
                ImageIssue::Malicious("Has an image built to exhaust memory (decompression bomb)")
            }
            _ => ImageIssue::Unusable,
        })?
    };
    let image = if image.width().max(image.height()) > MAX_PIXEL_SIZE {
        image.resize(
            MAX_PIXEL_SIZE,
            MAX_PIXEL_SIZE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        image
    };

    let mut out = Cursor::new(Vec::new());
    let mime = if image.color().has_alpha() {
        image
            .write_to(&mut out, ImageFormat::Png)
            .map_err(|_| ImageIssue::Unusable)?;
        "image/png"
    } else {
        JpegEncoder::new_with_quality(&mut out, 85)
            .encode_image(&image.to_rgb8())
            .map_err(|_| ImageIssue::Unusable)?;
        "image/jpeg"
    };
    Ok(CleanImage {
        data: out.into_inner(),
        mime,
    })
}

fn looks_like_svg(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    String::from_utf8_lossy(head)
        .to_ascii_lowercase()
        .contains("<svg")
}

/// Renders SVG to pixels. `<image>` elements may only use `data:` URLs, so an
/// SVG can't pull in local files; scripts and animation are never run.
fn rasterize_svg(bytes: &[u8]) -> Option<DynamicImage> {
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: usvg::ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(|_, _| None),
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).ok()?;
    let size = tree.size();
    let scale = (MAX_PIXEL_SIZE as f32 / size.width().max(size.height())).min(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (width, height) = (
        ((size.width() * scale).ceil() as u32).max(1),
        ((size.height() * scale).ceil() as u32).max(1),
    );
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied alpha; image wants straight alpha.
    let pixels = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    image::RgbaImage::from_raw(width, height, pixels).map(DynamicImage::ImageRgba8)
}

#[cfg(test)]
mod test {
    use super::*;

    fn png_bytes(width: u32, height: u32, alpha: bool) -> Vec<u8> {
        let image = if alpha {
            DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                width,
                height,
                image::Rgba([255, 0, 0, 128]),
            ))
        } else {
            DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
                width,
                height,
                image::Rgb([0, 128, 255]),
            ))
        };
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn data_url(bytes: &[u8]) -> String {
        format!("data:image/png;base64,{}", BASE64.encode(bytes))
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = !0u32;
        for &byte in bytes {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn strips_scripts_handlers_frames_and_javascript_links() {
        let html = r#"<p onclick="steal()">Hi<script>alert(1)</script></p>
            <iframe src="https://evil.example"></iframe>
            <a href="javascript:alert(1)">x</a><a href="data:text/html,boom">y</a>
            <form action="https://evil.example"><input name="pw"></form>"#;
        let clean = display_html(html, &HashMap::new()).html;
        for banned in [
            "script",
            "onclick",
            "iframe",
            "javascript:",
            "data:text",
            "form",
            "input",
        ] {
            assert!(!clean.contains(banned), "{banned} survived: {clean}");
        }
        assert!(clean.contains("Hi"));
    }

    #[test]
    fn keeps_harmless_styling_but_not_css_that_loads_urls() {
        let html = r#"<span style="color: red; font-weight: bold; background-image: url(https://track.example/p.gif)">styled</span>"#;
        let clean = display_html(html, &HashMap::new()).html;
        assert!(
            clean.contains("color:red") || clean.contains("color: red"),
            "{clean}"
        );
        assert!(clean.contains("font-weight"), "{clean}");
        assert!(!clean.contains("track.example"), "{clean}");
    }

    #[test]
    fn finds_only_https_and_data_images_once_each() {
        let html = r#"<img src="https://a.example/1.png"><img src="https://a.example/1.png">
            <img src="http://a.example/2.png"><img src="relative.png"><img src="data:image/png;base64,AA==">"#;
        let sources = image_sources(html);
        assert_eq!(remote_sources(&sources), ["https://a.example/1.png"]);
        assert!(sources.iter().any(|source| source.starts_with("data:")));
        assert!(!sources.iter().any(|source| source.contains("relative")));
    }

    #[test]
    fn inlines_cleaned_downloads_and_drops_everything_else() {
        let html = r#"<img src="https://a.example/ok.png" srcset="https://a.example/big.png 2x">
            <img src="https://a.example/missing.png"><img src="https://a.example/not-an-image.png">
            <img src="http://a.example/plain.png">"#;
        let downloaded = HashMap::from([
            ("https://a.example/ok.png".to_owned(), png_bytes(4, 4, true)),
            (
                "https://a.example/not-an-image.png".to_owned(),
                b"<script>alert(1)</script>".to_vec(),
            ),
        ]);
        let clean = display_html(html, &downloaded).html;
        assert_eq!(
            clean.matches("data:image/png;base64,").count(),
            1,
            "{clean}"
        );
        assert!(!clean.contains("a.example"), "{clean}");
        assert!(!clean.contains("srcset"), "{clean}");
    }

    #[test]
    fn reencoding_drops_bytes_hidden_after_the_image() {
        let mut polyglot = png_bytes(4, 4, false);
        polyglot.extend_from_slice(b"<script>hidden payload</script>");
        let cleaned = clean_image(&polyglot).unwrap();
        assert_eq!(cleaned.mime, "image/jpeg");
        let hidden = b"hidden payload";
        assert!(!cleaned.data.windows(hidden.len()).any(|w| w == hidden));
    }

    #[test]
    fn refuses_decompression_bombs() {
        // A real 1×1 PNG whose header claims 100 000 × 100 000 pixels.
        let mut bomb = png_bytes(1, 1, false);
        bomb[16..20].copy_from_slice(&100_000u32.to_be_bytes());
        bomb[20..24].copy_from_slice(&100_000u32.to_be_bytes());
        let crc = crc32(&bomb[12..29]);
        bomb[29..33].copy_from_slice(&crc.to_be_bytes());
        assert_eq!(
            clean_image(&bomb).unwrap_err(),
            ImageIssue::Malicious("Has an image built to exhaust memory (decompression bomb)")
        );
    }

    #[test]
    fn shrinks_large_images() {
        let cleaned = clean_image(&png_bytes(3200, 800, true)).unwrap();
        let image = image::load_from_memory(&cleaned.data).unwrap();
        assert_eq!((image.width(), image.height()), (1600, 400));
    }

    #[test]
    fn svg_with_a_script_is_malicious() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#;
        assert_eq!(
            clean_image(svg).unwrap_err(),
            ImageIssue::Malicious("Has an SVG image containing a script")
        );
    }

    #[test]
    fn broken_images_are_unusable_not_malicious() {
        assert_eq!(
            clean_image(b"not an image").unwrap_err(),
            ImageIssue::Unusable
        );
        let html = r#"<img src="https://a.example/x.png">"#;
        let downloaded = HashMap::from([("https://a.example/x.png".to_owned(), b"404".to_vec())]);
        assert!(display_html(html, &downloaded).threats.is_empty());
    }

    #[test]
    fn finds_threats_in_the_original_html() {
        let html = r#"<p onmouseover="x()">a</p><script>y()</script>
            <a href=" JaVa&#x09;Script:alert(1)">b</a><iframe src="https://e.example"></iframe>
            <meta http-equiv="Refresh" content="0;url=https://e.example">"#;
        assert_eq!(
            threats(html),
            [
                "Runs code on an event (onmouseover)",
                "Contains a script",
                "Contains a link that runs code (javascript:)",
                "Embeds another web page",
                "Redirects to another page",
            ]
        );
    }

    #[test]
    fn ordinary_copied_html_has_no_threats() {
        let html = r#"<meta charset="utf-8"><h1 style="color:#333">Title</h1>
            <p>Some <b>bold</b> and a <a href="https://example.com">link</a>.</p>
            <img src="https://example.com/a.png" alt="pic"><table><tr><td>1</td></tr></table>"#;
        assert!(threats(html).is_empty());
    }

    #[test]
    fn rasterises_svg_without_reading_files() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="10">
            <image xlink:href="/etc/passwd" width="20" height="10"/>
            <rect width="20" height="10" fill="red"/></svg>"#;
        let cleaned = clean_image(svg).unwrap();
        assert_eq!(cleaned.mime, "image/png");
        let image = image::load_from_memory(&cleaned.data).unwrap().to_rgba8();
        assert_eq!(image.dimensions(), (20, 10));
        assert_eq!(image.get_pixel(5, 5).0, [255, 0, 0, 255]);
    }

    #[test]
    fn decodes_inline_data_images() {
        let html = format!(
            r#"<p>x</p><img src="{}">"#,
            data_url(&png_bytes(2, 2, true))
        );
        let clean = display_html(&html, &HashMap::new()).html;
        assert_eq!(clean.matches("data:image/png;base64,").count(), 1);
    }
}
