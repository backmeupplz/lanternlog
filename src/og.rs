//! Share images: a 1200×630 card for every post and site, drawn from SVG with the server's fonts.

use crate::render::esc;
use resvg::{tiny_skia, usvg};
use std::sync::{Arc, OnceLock};

fn fonts() -> Arc<usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = usvg::fontdb::Database::new();
            db.load_system_fonts();
            if let Ok(dir) = std::env::var("LANTERNLOG_FONTS") {
                db.load_fonts_dir(dir);
            }
            Arc::new(db)
        })
        .clone()
}

/// Whether a character takes two columns: Chinese, Japanese and Korean script and full-width forms.
pub fn wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD)
}

/// Whether a character belongs to the letter before it (a vowel sign, a tone or other mark, a joiner), so a line
/// must not start with it.
fn mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05C7 | 0x0610..=0x061A | 0x064B..=0x065F | 0x0670
        | 0x0900..=0x0903 | 0x093A..=0x094F | 0x0951..=0x0957 | 0x0962..=0x0963 | 0x0981..=0x0983 | 0x09BC..=0x09D7
        | 0x09E2..=0x09E3 | 0x0E31 | 0x0E34..=0x0E3A | 0x0E47..=0x0E4E | 0x200C..=0x200D | 0xFE00..=0xFE0F)
}

/// A virama joins the consonant after it into a conjunct, and a Thai leading vowel is written before the consonant
/// it follows in speech, so a line must not end with either.
fn virama(c: char) -> bool {
    matches!(c as u32, 0x094D | 0x09CD | 0x0E40..=0x0E44)
}

/// How many columns text takes, counting wide characters twice.
fn cols(text: &str) -> usize {
    text.chars().map(|c| if wide(c) { 2 } else { 1 }).sum()
}

/// Breaks text into lines of at most `width` columns, splitting words only when one is longer than a line
/// (as runs of Chinese or Japanese, written without spaces, always are).
fn wrap(text: &str, width: usize) -> Vec<String> {
    let (mut lines, mut line) = (Vec::new(), String::new());
    for word in text.split_whitespace() {
        let mut word: Vec<char> = word.chars().collect();
        while cols(&word.iter().collect::<String>()) > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let (mut n, mut used) = (0, 0);
            while let Some(&c) = word.get(n) {
                let w = if wide(c) { 2 } else { 1 };
                if used + w > width {
                    break;
                }
                used += w;
                n += 1;
            }
            while n > 1 && (word.get(n).is_some_and(|&c| mark(c)) || word.get(n - 1).is_some_and(|&c| virama(c))) {
                n -= 1;
            }
            lines.push(word.drain(..n.max(1)).collect());
        }
        let word: String = word.into_iter().collect();
        if line.is_empty() {
            line = word;
        } else if cols(&line) + 1 + cols(&word) <= width {
            line = format!("{line} {word}");
        } else {
            lines.push(std::mem::replace(&mut line, word));
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// The biggest type size at which the title fits, with its lines. Widths assume wide bold letters, so text
/// never reaches the edges.
fn fit(title: &str) -> (u32, Vec<String>) {
    for (size, width, max) in [(80, 20, 3), (68, 24, 3), (58, 28, 4), (50, 33, 5)] {
        let lines = wrap(title, width);
        if lines.len() <= max {
            return (size, lines);
        }
    }
    let mut lines = wrap(title, 33);
    lines.truncate(5);
    if let Some(last) = lines.last_mut() {
        last.push('…');
    }
    (50, lines)
}

/// What a share card shows. The site's name sits at the top unless the card is the site's own.
pub struct Card<'a> {
    pub site: &'a str,
    pub title: &'a str,
    pub subtitle: &'a str,
    pub host: &'a str,
    pub meta: &'a str,
    pub accent: &'a str,
    /// The site's logo as a data URI, drawn in place of its initial.
    pub logo: Option<&'a str>,
    /// The language of the text, which picks the right Chinese, Japanese or Korean letterforms.
    pub lang: &'a str,
}

pub fn svg(c: &Card) -> String {
    // Scripts Inter lacks get a font of their own first (Noto in the Docker image; the second name is macOS's), so a
    // whole title shapes in one font, and Chinese, Japanese and Korean get their own letterforms. Tight letter
    // spacing only suits Latin-like scripts; Arabic letters must stay joined.
    let lower = c.lang.to_ascii_lowercase();
    let script = match lower.split('-').next().unwrap_or("") {
        "ja" => "Noto Sans CJK JP, Hiragino Sans, ",
        "ko" => "Noto Sans CJK KR, Apple SD Gothic Neo, ",
        "zh" if ["zh-hant", "zh-tw", "zh-hk", "zh-mo"].contains(&lower.as_str()) => "Noto Sans CJK TC, PingFang TC, ",
        "zh" => "Noto Sans CJK SC, PingFang SC, ",
        "ar" | "fa" | "ur" | "ps" | "ckb" => "Noto Sans Arabic, Noto Naskh Arabic, Geeza Pro, ",
        "he" | "yi" => "Noto Sans Hebrew, Arial Hebrew, ",
        "hi" | "mr" | "ne" => "Noto Sans Devanagari, Kohinoor Devanagari, ",
        "bn" => "Noto Sans Bengali, Kohinoor Bangla, ",
        "th" => "Noto Sans Thai, Thonburi, ",
        _ => "",
    };
    const FONT: &str = "Inter, DejaVu Sans, Helvetica, Arial, sans-serif";
    let font = format!("{script}{FONT}");
    let font = font.as_str();
    let rtl = crate::lang::rtl(c.lang);
    // Right-to-left text starts at the right edge; each line is a text of its own so bidi runs never cross lines.
    let (x, anchor) = if rtl { (1120, " text-anchor=\"end\" direction=\"rtl\"") } else { (80, "") };
    let (size, lines) = fit(c.title);
    let mut sub = wrap(c.subtitle, 56);
    if sub.len() > 2 {
        sub.truncate(2);
        if let Some(last) = sub.last_mut() {
            last.push('…');
        }
    }
    let line = size * 114 / 100;
    let mut height = size + line * (lines.len() as u32 - 1);
    if !sub.is_empty() {
        height += 30 + 32 + 44 * (sub.len() as u32 - 1);
    }
    let first = 170 + 320u32.saturating_sub(height) / 2 + size * 78 / 100;
    let spacing = if script.is_empty() { -(size as f32) * 0.02 } else { 0.0 };
    let mut text = String::new();
    for (i, l) in lines.iter().enumerate() {
        text += &format!(
            "<text x=\"{x}\" y=\"{}\"{anchor} font-family=\"{font}\" font-size=\"{size}\" font-weight=\"800\" letter-spacing=\"{spacing}\" fill=\"#ffffff\">{}</text>",
            first + line * i as u32,
            esc(l)
        );
    }
    let sub_y = first + line * (lines.len() as u32 - 1) + 30 + 32;
    for (i, l) in sub.iter().enumerate() {
        text += &format!(
            "<text x=\"{x}\" y=\"{}\"{anchor} font-family=\"{font}\" font-size=\"32\" fill=\"#b8bbc1\">{}</text>",
            sub_y + 44 * i as u32,
            esc(l)
        );
    }
    let meta_dir = if rtl { " direction=\"rtl\"" } else { "" };
    let mark_text = crate::style::rgb(c.accent)
        .map_or("#ffffff", |a| if crate::style::contrast(a, [255.0; 3]) >= 2.5 { "#ffffff" } else { "#111214" });
    let initial: String = c.site.chars().find(|ch| ch.is_alphanumeric()).map(|ch| ch.to_uppercase().collect()).unwrap_or_default();
    let name = if c.site == c.title { String::new() } else { esc(c.site) };
    let mark = match c.logo {
        Some(data) => format!("<image x=\"80\" y=\"78\" width=\"52\" height=\"52\" preserveAspectRatio=\"xMidYMid meet\" href=\"{data}\"/>"),
        None => format!(
            "<rect x=\"80\" y=\"78\" width=\"52\" height=\"52\" rx=\"14\" fill=\"{}\"/><text x=\"106\" y=\"115\" text-anchor=\"middle\" \
             font-family=\"{FONT}\" font-size=\"30\" font-weight=\"800\" fill=\"{mark_text}\">{}</text>",
            esc(c.accent),
            esc(&initial)
        ),
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1200\" height=\"630\" viewBox=\"0 0 1200 630\"><defs>\
         <linearGradient id=\"bg\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\"><stop offset=\"0\" stop-color=\"#16181c\"/><stop offset=\"1\" stop-color=\"#0b0c0e\"/></linearGradient>\
         <radialGradient id=\"glow\" cx=\"1050\" cy=\"70\" r=\"600\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"{accent}\" stop-opacity=\".5\"/>\
         <stop offset=\".5\" stop-color=\"{accent}\" stop-opacity=\".12\"/><stop offset=\"1\" stop-color=\"{accent}\" stop-opacity=\"0\"/></radialGradient>\
         <radialGradient id=\"low\" cx=\"60\" cy=\"660\" r=\"440\" gradientUnits=\"userSpaceOnUse\"><stop offset=\"0\" stop-color=\"{accent}\" stop-opacity=\".13\"/>\
         <stop offset=\"1\" stop-color=\"{accent}\" stop-opacity=\"0\"/></radialGradient></defs>\
         <rect width=\"1200\" height=\"630\" fill=\"url(#bg)\"/><rect width=\"1200\" height=\"630\" fill=\"url(#glow)\"/><rect width=\"1200\" height=\"630\" fill=\"url(#low)\"/>\
         {mark}\
         <text x=\"150\" y=\"114\" font-family=\"{font}\" font-size=\"30\" font-weight=\"600\" fill=\"#e8e7e4\">{name}</text>\
         {text}\
         <rect x=\"80\" y=\"514\" width=\"1040\" height=\"1\" fill=\"#ffffff\" fill-opacity=\".14\"/>\
         <text x=\"80\" y=\"562\" font-family=\"{FONT}\" font-size=\"26\" font-weight=\"500\" fill=\"#a3a6ac\">{host}</text>\
         <text x=\"1120\" y=\"562\" text-anchor=\"end\"{meta_dir} font-family=\"{font}\" font-size=\"26\" fill=\"#a3a6ac\">{meta}</text></svg>",
        accent = esc(c.accent),
        host = esc(c.host),
        meta = esc(c.meta),
    )
}

pub fn png(svg: &str) -> anyhow::Result<Vec<u8>> {
    let opt = usvg::Options { fontdb: fonts(), ..Default::default() };
    let tree = usvg::Tree::from_str(svg, &opt)?;
    let size = tree.size().to_int_size();
    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height()).ok_or_else(|| anyhow::anyhow!("empty image"))?;
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    Ok(pixmap.encode_png()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_fit_inside_the_card() {
        assert_eq!(fit("Short").1, vec!["Short"]);
        let (size, lines) = fit("How we cut our deploy times from four minutes to forty seconds on a single Hetzner box");
        assert!(size < 80 && lines.len() <= 5 && lines.iter().all(|l| l.chars().count() <= 33));
        assert!(wrap("a-very-long-unbroken-identifier-that-keeps-going-forever", 22).iter().all(|l| l.chars().count() <= 22));
        // Chinese has no spaces and every character is twice as wide, so it breaks by columns.
        let zh = wrap("我们如何把部署时间从四分钟缩短到四十秒", 20);
        assert!(zh.len() == 2 && zh.iter().all(|l| cols(l) <= 20), "{zh:?}");
        // Thai runs without spaces never break before a vowel or tone mark.
        for l in wrap("ย้ายทุกบริการไปไว้บนเซิร์ฟเวอร์เดียวเพื่อประหยัดค่าใช้จ่าย", 20) {
            assert!(!l.starts_with(mark) && !l.ends_with(virama), "{l:?}");
        }
    }

    #[test]
    fn cards_follow_the_script_of_their_language() {
        let card = |title: &'static str, lang: &'static str| Card { site: "Notes", title, subtitle: "", host: "h", meta: "", accent: "#2563eb", logo: None, lang };
        let ar = svg(&card("نقل كل الخدمات إلى خادم واحد", "ar"));
        assert!(ar.contains(r#"x="1120" y="#) && ar.contains(r#"text-anchor="end" direction="rtl""#) && ar.contains("Noto Sans Arabic"));
        assert!(svg(&card("हर सेवा को एक सर्वर पर ले जाना", "hi")).contains(r#"font-family="Noto Sans Devanagari"#));
        assert!(svg(&card("繁體標題", "zh-hant")).contains("Noto Sans CJK TC"));
        let en = svg(&card("One server", "en"));
        assert!(!en.contains("direction") && en.contains(r#"<text x="80" y="#) && en.contains("letter-spacing=\"-1.5"), "{en}");
    }
}
