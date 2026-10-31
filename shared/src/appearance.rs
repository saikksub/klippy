//! Whether a copy was made for a light or a dark page, decided once when it's
//! saved, so the viewer can show it on the kind of background it came from.
//!
//! Each run of text counts towards the background it sits on (the nearest
//! one set), or, with none set, towards the opposite of its own colour: light
//! text was meant for a dark page. Text with neither follows the system.
//! Counting characters means the page's own background outweighs a small
//! highlighted word or badge.

use std::cell::RefCell;

use html5ever::{
    tendril::StrTendril,
    tokenizer::{
        BufferQueue, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer, TokenizerOpts,
    },
};

use crate::{Appearance, ClipboardContent, clean::DISPLAY_HTML_KIND};

/// Decides from the HTML the viewer shows (the display copy, else the
/// original), otherwise from the RTF. Plain text has no colours of its own.
pub fn detect(content: &ClipboardContent) -> Appearance {
    let format = |kind: &str| {
        content
            .formats
            .iter()
            .find(|format| format.kind == kind)
            .map(|format| String::from_utf8_lossy(&format.data))
    };
    let votes = if let Some(html) = format(DISPLAY_HTML_KIND).or_else(|| format("public.html")) {
        html_votes(&html)
    } else if let Some(rtf) = format("com.apple.flat-rtfd").or_else(|| format("public.rtf")) {
        rtf_votes(&rtf)
    } else {
        Votes::default()
    };
    votes.appearance()
}

/// Characters of text that need a light or a dark page.
#[derive(Default, Debug)]
struct Votes {
    light: usize,
    dark: usize,
}

impl Votes {
    fn add(&mut self, chars: usize, background: Option<Rgb>, foreground: Option<Rgb>) {
        match (background, foreground) {
            (Some(background), _) if background.is_dark() => self.dark += chars,
            (Some(_), _) => self.light += chars,
            (None, Some(foreground)) if foreground.is_dark() => self.light += chars,
            (None, Some(_)) => self.dark += chars,
            (None, None) => {}
        }
    }

    fn appearance(&self) -> Appearance {
        match (self.light, self.dark) {
            (0, 0) => Appearance::System,
            (light, dark) if dark > light => Appearance::Dark,
            _ => Appearance::Light,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Rgb(u8, u8, u8);

impl Rgb {
    /// Darker than the luminance where black and white text contrast equally
    /// with it (WCAG relative luminance, about 0.18).
    fn is_dark(self) -> bool {
        let linear = |channel: u8| {
            let c = f64::from(channel) / 255.0;
            if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        let luminance = 0.2126 * linear(self.0) + 0.7152 * linear(self.1) + 0.0722 * linear(self.2);
        // Where (L + 0.05) / 0.05 == 1.05 / (L + 0.05).
        luminance < (1.05f64 * 0.05).sqrt() - 0.05
    }
}

/// A CSS or HTML attribute colour. Mostly transparent colours, keywords like
/// `inherit`, and forms copies don't use (`hsl()`, …) give `None`, so they
/// don't count.
fn parse_color(value: &str) -> Option<Rgb> {
    let value = value.to_ascii_lowercase();
    let value = value.trim().trim_end_matches("!important").trim();
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex(hex);
    }
    if let Some(args) = value
        .strip_prefix("rgba(")
        .or_else(|| value.strip_prefix("rgb("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return parse_rgb_args(args);
    }
    let named = match value {
        "black" => Rgb(0, 0, 0),
        "white" => Rgb(255, 255, 255),
        "gray" | "grey" => Rgb(128, 128, 128),
        "silver" => Rgb(192, 192, 192),
        "lightgray" | "lightgrey" => Rgb(211, 211, 211),
        "darkgray" | "darkgrey" => Rgb(169, 169, 169),
        "dimgray" | "dimgrey" => Rgb(105, 105, 105),
        "gainsboro" => Rgb(220, 220, 220),
        "whitesmoke" => Rgb(245, 245, 245),
        "red" => Rgb(255, 0, 0),
        "maroon" => Rgb(128, 0, 0),
        "orange" => Rgb(255, 165, 0),
        "yellow" => Rgb(255, 255, 0),
        "olive" => Rgb(128, 128, 0),
        "lime" => Rgb(0, 255, 0),
        "green" => Rgb(0, 128, 0),
        "aqua" | "cyan" => Rgb(0, 255, 255),
        "teal" => Rgb(0, 128, 128),
        "blue" => Rgb(0, 0, 255),
        "navy" => Rgb(0, 0, 128),
        "fuchsia" | "magenta" => Rgb(255, 0, 255),
        "purple" => Rgb(128, 0, 128),
        _ => return None,
    };
    Some(named)
}

fn parse_hex(hex: &str) -> Option<Rgb> {
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 | 4 => {
            if hex.len() == 4 && digit(3)? < 8 {
                return None;
            }
            Some(Rgb(digit(0)? * 17, digit(1)? * 17, digit(2)? * 17))
        }
        6 | 8 => {
            if hex.len() == 8 && pair(6)? < 128 {
                return None;
            }
            Some(Rgb(pair(0)?, pair(2)?, pair(4)?))
        }
        _ => None,
    }
}

/// `r, g, b[, a]` or `r g b[ / a]`, each a number or a percentage.
fn parse_rgb_args(args: &str) -> Option<Rgb> {
    let (channels, alpha) = match args.split_once('/') {
        Some((channels, alpha)) => (channels, Some(alpha)),
        None => (args, None),
    };
    let mut parts: Vec<&str> = channels
        .split([',', ' '])
        .filter(|part| !part.is_empty())
        .collect();
    let alpha = alpha.or_else(|| (parts.len() == 4).then(|| parts.pop()).flatten());
    if parts.len() != 3 {
        return None;
    }
    if let Some(alpha) = alpha {
        let alpha = alpha.trim();
        let alpha = match alpha.strip_suffix('%') {
            Some(percent) => percent.parse::<f64>().ok()? / 100.0,
            None => alpha.parse::<f64>().ok()?,
        };
        if alpha < 0.5 {
            return None;
        }
    }
    let channel = |part: &str| -> Option<u8> {
        let value = match part.strip_suffix('%') {
            Some(percent) => percent.parse::<f64>().ok()? * 2.55,
            None => part.parse::<f64>().ok()?,
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = value.round().clamp(0.0, 255.0) as u8;
        Some(value)
    };
    Some(Rgb(
        channel(parts[0])?,
        channel(parts[1])?,
        channel(parts[2])?,
    ))
}

/// The colour in a `background` shorthand, if it has one.
fn shorthand_color(value: &str) -> Option<Rgb> {
    let lower = value.to_ascii_lowercase();
    if let Some(start) = lower.find("rgb") {
        let end = lower[start..].find(')')? + start;
        return parse_color(&lower[start..=end]);
    }
    lower.split_whitespace().find_map(parse_color)
}

// MARK: HTML

fn html_votes(html: &str) -> Votes {
    let input = BufferQueue::default();
    input.push_back(StrTendril::from_slice(html));
    let tokenizer = Tokenizer::new(HtmlScan::default(), TokenizerOpts::default());
    let _ = tokenizer.feed(&input);
    tokenizer.end();
    tokenizer.sink.state.take().votes
}

/// Open elements with the colours in effect inside each.
#[derive(Default)]
struct HtmlState {
    stack: Vec<OpenElement>,
    votes: Votes,
    /// Inside `<style>`, `<title>`, … whose text isn't shown.
    hidden: usize,
}

struct OpenElement {
    name: String,
    background: Option<Rgb>,
    foreground: Option<Rgb>,
}

#[derive(Default)]
struct HtmlScan {
    state: RefCell<HtmlState>,
}

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

const HIDDEN_ELEMENTS: &[&str] = &["head", "style", "script", "title", "template", "noscript"];

impl TokenSink for HtmlScan {
    type Handle = ();

    fn process_token(&self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
        let mut state = self.state.borrow_mut();
        match token {
            Token::TagToken(tag) => {
                let name = tag.name.to_string();
                if tag.kind == TagKind::EndTag {
                    // Closes the nearest match and anything left open inside it.
                    if let Some(index) = state.stack.iter().rposition(|open| open.name == name) {
                        state.stack.truncate(index);
                    }
                    if HIDDEN_ELEMENTS.contains(&name.as_str()) {
                        state.hidden = state.hidden.saturating_sub(1);
                    }
                    return TokenSinkResult::Continue;
                }
                if VOID_ELEMENTS.contains(&name.as_str()) || tag.self_closing {
                    return TokenSinkResult::Continue;
                }
                if HIDDEN_ELEMENTS.contains(&name.as_str()) {
                    state.hidden += 1;
                }
                let parent = state.stack.last();
                let mut background = parent.and_then(|open| open.background);
                let mut foreground = parent.and_then(|open| open.foreground);
                for attr in &tag.attrs {
                    let value: &str = &attr.value;
                    match &*attr.name.local {
                        "bgcolor" => background = parse_color(value).or(background),
                        "color" if name == "font" => foreground = parse_color(value).or(foreground),
                        "text" if name == "body" => foreground = parse_color(value).or(foreground),
                        "style" => {
                            for declaration in value.split(';') {
                                let Some((property, value)) = declaration.split_once(':') else {
                                    continue;
                                };
                                match property.trim().to_ascii_lowercase().as_str() {
                                    "color" => foreground = parse_color(value).or(foreground),
                                    "background-color" => {
                                        background = parse_color(value).or(background);
                                    }
                                    "background" => {
                                        background = shorthand_color(value).or(background);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                state.stack.push(OpenElement {
                    name,
                    background,
                    foreground,
                });
            }
            Token::CharacterTokens(text) if state.hidden == 0 => {
                let chars = text.chars().filter(|c| !c.is_whitespace()).count();
                let open = state.stack.last();
                let (background, foreground) = (
                    open.and_then(|open| open.background),
                    open.and_then(|open| open.foreground),
                );
                state.votes.add(chars, background, foreground);
            }
            _ => {}
        }
        TokenSinkResult::Continue
    }
}

// MARK: RTF

/// Groups whose content isn't document text.
const RTF_DESTINATIONS: &[&str] = &[
    "fonttbl",
    "colortbl",
    "expandedcolortbl",
    "stylesheet",
    "info",
    "pict",
    "filetbl",
    "listtable",
    "listoverridetable",
    "generator",
    "header",
    "footer",
];

#[derive(Clone, Copy, Default)]
struct RtfGroup {
    skip: bool,
    background: Option<Rgb>,
    foreground: Option<Rgb>,
}

/// Flat RTFD is a small archive with the RTF inside; plain RTF is just that.
fn rtf_votes(data: &str) -> Votes {
    let mut votes = Votes::default();
    let Some(start) = data.find("{\\rtf") else {
        return votes;
    };
    let rtf = &data[start..];
    let colors = color_table(rtf);
    let color = |index: Option<i32>| {
        let index = usize::try_from(index.unwrap_or(0)).ok()?;
        colors.get(index).copied().flatten()
    };

    let mut stack: Vec<RtfGroup> = Vec::new();
    let mut group = RtfGroup::default();
    // Right after `{`, where a destination keyword can start the group.
    let mut group_start = false;
    let mut chars = rtf.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        match c {
            '{' => {
                stack.push(group);
                group_start = true;
                continue;
            }
            '}' => {
                group = stack.pop().unwrap_or_default();
                if stack.is_empty() {
                    break;
                }
            }
            '\\' => {
                let Some(&(_, next)) = chars.peek() else {
                    break;
                };
                if next.is_ascii_alphabetic() {
                    let mut word = String::new();
                    while let Some(&(_, letter)) =
                        chars.peek().filter(|(_, c)| c.is_ascii_alphabetic())
                    {
                        word.push(letter);
                        chars.next();
                    }
                    let mut number = String::new();
                    if let Some(&(_, '-')) = chars.peek() {
                        number.push('-');
                        chars.next();
                    }
                    while let Some(&(_, digit)) = chars.peek().filter(|(_, c)| c.is_ascii_digit()) {
                        number.push(digit);
                        chars.next();
                    }
                    if let Some(&(_, ' ')) = chars.peek() {
                        chars.next();
                    }
                    let parameter = number.parse::<i32>().ok();
                    if group_start && RTF_DESTINATIONS.contains(&word.as_str()) {
                        group.skip = true;
                    }
                    match word.as_str() {
                        "cf" => group.foreground = color(parameter),
                        "cb" | "highlight" | "chcbpat" => group.background = color(parameter),
                        "plain" => {
                            group.foreground = None;
                            group.background = None;
                        }
                        "u" if !group.skip => votes.add(1, group.background, group.foreground),
                        _ => {}
                    }
                } else {
                    chars.next();
                    match next {
                        '*' if group_start => group.skip = true,
                        '\'' => {
                            chars.next();
                            chars.next();
                            if !group.skip {
                                votes.add(1, group.background, group.foreground);
                            }
                        }
                        '\\' | '{' | '}' | '~' if !group.skip => {
                            votes.add(1, group.background, group.foreground);
                        }
                        _ => {}
                    }
                }
                // `\*` is followed by the destination's own keyword.
                if next == '*' {
                    continue;
                }
            }
            c if !group.skip && !c.is_whitespace() => {
                votes.add(1, group.background, group.foreground);
            }
            _ => {}
        }
        group_start = false;
    }
    votes
}

/// Entries of `\colortbl`, in order; `None` for "auto" (an empty entry).
fn color_table(rtf: &str) -> Vec<Option<Rgb>> {
    let Some(start) = rtf.find("{\\colortbl") else {
        return Vec::new();
    };
    let table = &rtf[start + "{\\colortbl".len()..];
    let table = &table[..table.find('}').unwrap_or(table.len())];
    let mut entries: Vec<&str> = table.split(';').collect();
    // Text after the last `;` isn't an entry.
    entries.pop();
    entries
        .into_iter()
        .map(|entry| {
            let channel = |name: &str| -> Option<u8> {
                let rest = &entry[entry.find(name)? + name.len()..];
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                digits.parse().ok()
            };
            Some(Rgb(
                channel("\\red")?,
                channel("\\green")?,
                channel("\\blue")?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ClipboardFormat;

    fn with(kind: &str, data: &str) -> ClipboardContent {
        ClipboardContent {
            text: "text".into(),
            formats: vec![ClipboardFormat {
                kind: kind.into(),
                data: data.as_bytes().to_vec(),
            }],
        }
    }

    fn html(data: &str) -> Appearance {
        detect(&with("public.html", data))
    }

    fn rtf(data: &str) -> Appearance {
        detect(&with("public.rtf", data))
    }

    #[test]
    fn plain_text_and_uncoloured_html_follow_the_system() {
        assert_eq!(
            detect(&with("public.utf8-plain-text", "hi")),
            Appearance::System
        );
        assert_eq!(html("<p>Just <b>words</b></p>"), Appearance::System);
    }

    #[test]
    fn a_dark_code_editor_copy_is_dark_despite_coloured_tokens() {
        let copy = r#"<meta charset="utf-8"><div style="color: #cccccc;background-color: #1f1f1f;font-family: Menlo"><div><span style="color: #569cd6;">fn</span> <span style="color: #dcdcaa;">main</span>() {}</div></div>"#;
        assert_eq!(html(copy), Appearance::Dark);
    }

    #[test]
    fn dark_text_without_a_background_needs_a_light_page() {
        let copy = r#"<span style="color: rgb(31, 35, 40); font-family: -apple-system">Hello from a web page</span>"#;
        assert_eq!(html(copy), Appearance::Light);
    }

    #[test]
    fn light_text_without_a_background_needs_a_dark_page() {
        assert_eq!(
            html(r#"<p style="color: rgb(240 246 252 / 100%)">Copied from a dark site</p>"#),
            Appearance::Dark
        );
    }

    #[test]
    fn a_small_dark_badge_doesnt_outweigh_the_page() {
        let copy = r#"<div style="color:#111">A long paragraph of ordinary dark text on a light page, <span style="background-color:#000;color:#fff">new</span></div>"#;
        assert_eq!(html(copy), Appearance::Light);
    }

    #[test]
    fn transparent_backgrounds_and_hidden_text_dont_count() {
        let copy = r#"<style>p { color: white }</style><p style="background-color: rgba(0,0,0,0.05); color: #222">text</p>"#;
        assert_eq!(html(copy), Appearance::Light);
        assert_eq!(
            html(r##"<table bgcolor="#000000"><tr><td>dark cell</td></tr></table>"##),
            Appearance::Dark
        );
    }

    #[test]
    fn the_display_copy_wins_over_the_original() {
        let mut content = with("public.html", r#"<p style="color:#fff">a</p>"#);
        content.formats.push(ClipboardFormat {
            kind: DISPLAY_HTML_KIND.into(),
            data: br#"<p style="color:#000">a</p>"#.to_vec(),
        });
        assert_eq!(detect(&content), Appearance::Light);
    }

    #[test]
    fn rtf_with_auto_colours_follows_the_system() {
        let copy = r"{\rtf1\ansi{\fonttbl\f0\fswiss Helvetica;}{\colortbl;\red255\green255\blue255;}\f0\fs24 \cf0 Hello world}";
        assert_eq!(rtf(copy), Appearance::System);
    }

    #[test]
    fn rtf_terminal_copy_with_a_dark_background_is_dark() {
        let copy = r"{\rtf1\ansi{\fonttbl\f0\fmodern Menlo;}{\colortbl;\red255\green255\blue255;\red0\green0\blue0;\red230\green230\blue230;}\f0\cb2\cf3 ls -la\par drwxr-xr-x  5 me  staff}";
        assert_eq!(rtf(copy), Appearance::Dark);
    }

    #[test]
    fn rtf_with_black_text_is_light() {
        let copy = r"{\rtf1\ansi{\colortbl;\red255\green255\blue255;\red0\green0\blue0;}{\*\expandedcolortbl;;\cssrgb\c0\c0\c0;}\cf2 Black text from TextEdit}";
        assert_eq!(rtf(copy), Appearance::Light);
    }

    #[test]
    fn parses_css_colour_forms() {
        assert_eq!(parse_color("#FFF"), Some(Rgb(255, 255, 255)));
        assert_eq!(parse_color("#1f1f1f !important"), Some(Rgb(31, 31, 31)));
        assert_eq!(parse_color("#00000010"), None);
        assert_eq!(parse_color("rgba(10, 20, 30, 0.9)"), Some(Rgb(10, 20, 30)));
        assert_eq!(parse_color("rgb(100% 0% 0%)"), Some(Rgb(255, 0, 0)));
        assert_eq!(parse_color("transparent"), None);
        assert_eq!(
            shorthand_color("rgb(1, 2, 3) none repeat"),
            Some(Rgb(1, 2, 3))
        );
        assert!(Rgb(31, 31, 31).is_dark());
        assert!(!Rgb(255, 255, 255).is_dark());
        assert!(!Rgb(128, 128, 128).is_dark());
    }
}
