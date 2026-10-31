//! Builds exported documents from a saved copy. Everything comes from the
//! already-sanitised display HTML (or the plain text), so exports carry no
//! scripts and need no network: images are embedded as `data:` URIs.

use crate::ExportFormat;

/// Longest part of a file name taken from the copy itself.
const NAME_CHARS: usize = 40;

/// "Klippy – First words of the copy.pdf", safe for any file system.
pub fn file_name(preview: &str, format: ExportFormat) -> String {
    let words: String = preview
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -_.,'()".contains(c) {
                c
            } else {
                ' '
            }
        })
        .collect();
    let mut stem: String = words.split_whitespace().collect::<Vec<_>>().join(" ");
    if stem.chars().count() > NAME_CHARS {
        stem = stem
            .chars()
            .take(NAME_CHARS)
            .collect::<String>()
            .trim_end()
            .to_owned();
    }
    let stem = stem.trim_matches('.');
    let stem = if stem.is_empty() { "Copy" } else { stem };
    format!("Klippy – {stem}.{}", format.extension())
}

/// A standalone page. The CSP keeps it inert even if opened in a browser.
pub fn html_document(title: &str, display_html: Option<&str>, text: &str) -> String {
    let body = display_html.map_or_else(|| format!("<pre>{}</pre>", escape(text)), str::to_owned);
    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">
<meta name="generator" content="Klippy">
<title>{title}</title>
<style>
body {{ font: 15px/1.55 -apple-system, BlinkMacSystemFont, "Helvetica Neue", sans-serif; color: #1d1d1f; max-width: 760px; margin: 40px auto; padding: 0 24px; }}
img {{ max-width: 100%; height: auto; }}
pre {{ white-space: pre-wrap; word-wrap: break-word; font: 13px/1.5 ui-monospace, Menlo, monospace; }}
table {{ border-collapse: collapse; }}
td, th {{ padding: 4px 8px; }}
</style>
</head>
<body>
{body}
</body>
</html>
"#,
        title = escape(title)
    )
}

/// Markdown from the sanitised HTML; plain text is already Markdown-safe
/// enough to keep as it is.
pub fn markdown(display_html: Option<&str>, text: &str) -> String {
    display_html
        .and_then(|html| htmd::convert(html).ok())
        .filter(|markdown| !markdown.trim().is_empty())
        .unwrap_or_else(|| text.to_owned())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn file_names_are_short_and_safe() {
        assert_eq!(
            file_name("Hello / world: <b> \"quotes\"", ExportFormat::Pdf),
            "Klippy – Hello world b quotes.pdf"
        );
        assert_eq!(file_name("...", ExportFormat::Html), "Klippy – Copy.html");
        let long = file_name(&"word ".repeat(30), ExportFormat::Markdown);
        assert!(long.ends_with(".md"));
        assert!(long.chars().count() <= "Klippy – ".len() + NAME_CHARS + 3);
    }

    #[test]
    fn html_wraps_the_sanitised_copy_or_escapes_plain_text() {
        let rich = html_document("T", Some("<p><b>bold</b></p>"), "bold");
        assert!(rich.contains("<p><b>bold</b></p>"));
        assert!(rich.contains("Content-Security-Policy"));

        let plain = html_document("<T>", None, "a < b & c");
        assert!(plain.contains("<pre>a &lt; b &amp; c</pre>"));
        assert!(plain.contains("<title>&lt;T&gt;</title>"));
    }

    #[test]
    fn markdown_converts_html_and_keeps_plain_text() {
        let md = markdown(
            Some("<h1>Title</h1><p>Some <strong>bold</strong> text</p>"),
            "x",
        );
        assert!(md.contains("# Title"), "{md}");
        assert!(md.contains("**bold**"), "{md}");
        assert_eq!(markdown(None, "just *text*"), "just *text*");
    }
}
