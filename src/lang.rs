//! Languages: the codes posts are translated into (lowercase, as in `pt-br` or `zh-hans`), what each one is
//! called in itself, its Open Graph locale, how it writes dates, and the words of the blog's chrome.

use chrono::DateTime;
use std::collections::BTreeMap;

/// Code, name in the language itself, Open Graph locale, the glibc locale that names its months, and a long date.
const LANGS: &[(&str, &str, &str, &str, &str)] = &[
    ("en", "English", "en_US", "en_US", "%B %-d, %Y"),
    ("en-gb", "English (UK)", "en_GB", "en_GB", "%-d %B %Y"),
    ("ar", "العربية", "ar_AR", "ar_EG", "%-d %B %Y"),
    ("az", "Azərbaycanca", "az_AZ", "az_AZ", "%-d %B %Y"),
    ("bg", "Български", "bg_BG", "bg_BG", "%-d %B %Y"),
    ("bn", "বাংলা", "bn_IN", "bn_IN", "%-d %B %Y"),
    ("ca", "Català", "ca_ES", "ca_ES", "%-d %B %Y"),
    ("cs", "Čeština", "cs_CZ", "cs_CZ", "%-d. %B %Y"),
    ("da", "Dansk", "da_DK", "da_DK", "%-d. %B %Y"),
    ("de", "Deutsch", "de_DE", "de_DE", "%-d. %B %Y"),
    ("el", "Ελληνικά", "el_GR", "el_GR", "%-d %B %Y"),
    ("es", "Español", "es_ES", "es_ES", "%-d de %B de %Y"),
    ("es-419", "Español (Latinoamérica)", "es_LA", "es_MX", "%-d de %B de %Y"),
    ("et", "Eesti", "et_EE", "et_EE", "%-d. %B %Y"),
    ("fa", "فارسی", "fa_IR", "fa_IR", "%-d %B %Y"),
    ("fi", "Suomi", "fi_FI", "fi_FI", "%-d. %Bta %Y"),
    ("fil", "Filipino", "tl_PH", "fil_PH", "%B %-d, %Y"),
    ("fr", "Français", "fr_FR", "fr_FR", "%-d %B %Y"),
    ("he", "עברית", "he_IL", "he_IL", "%-d ב%B %Y"),
    ("hi", "हिन्दी", "hi_IN", "hi_IN", "%-d %B %Y"),
    ("hr", "Hrvatski", "hr_HR", "hr_HR", "%-d. %B %Y."),
    ("hu", "Magyar", "hu_HU", "hu_HU", "%Y. %B %-d."),
    ("id", "Bahasa Indonesia", "id_ID", "id_ID", "%-d %B %Y"),
    ("it", "Italiano", "it_IT", "it_IT", "%-d %B %Y"),
    ("ja", "日本語", "ja_JP", "ja_JP", "%Y年%-m月%-d日"),
    ("kk", "Қазақ тілі", "kk_KZ", "kk_KZ", "%Y ж. %-d %B"),
    ("ko", "한국어", "ko_KR", "ko_KR", "%Y년 %-m월 %-d일"),
    ("lt", "Lietuvių", "lt_LT", "lt_LT", "%Y-%m-%d"),
    ("lv", "Latviešu", "lv_LV", "lv_LV", "%Y. gada %-d. %B"),
    ("ms", "Bahasa Melayu", "ms_MY", "ms_MY", "%-d %B %Y"),
    ("nb", "Norsk bokmål", "nb_NO", "nb_NO", "%-d. %B %Y"),
    ("nl", "Nederlands", "nl_NL", "nl_NL", "%-d %B %Y"),
    ("no", "Norsk", "nb_NO", "nb_NO", "%-d. %B %Y"),
    ("pl", "Polski", "pl_PL", "pl_PL", "%-d %B %Y"),
    ("pt", "Português", "pt_PT", "pt_PT", "%-d de %B de %Y"),
    ("pt-br", "Português (Brasil)", "pt_BR", "pt_BR", "%-d de %B de %Y"),
    ("pt-pt", "Português (Portugal)", "pt_PT", "pt_PT", "%-d de %B de %Y"),
    ("ro", "Română", "ro_RO", "ro_RO", "%-d %B %Y"),
    ("ru", "Русский", "ru_RU", "ru_RU", "%-d %B %Y г."),
    ("sk", "Slovenčina", "sk_SK", "sk_SK", "%-d. %B %Y"),
    ("sl", "Slovenščina", "sl_SI", "sl_SI", "%-d. %B %Y"),
    ("sr", "Српски", "sr_RS", "sr_RS", "%-d. %B %Y."),
    ("sv", "Svenska", "sv_SE", "sv_SE", "%-d %B %Y"),
    ("sw", "Kiswahili", "sw_KE", "sw_KE", "%-d %B %Y"),
    ("th", "ไทย", "th_TH", "th_TH", "%-d %B %Y"),
    ("tr", "Türkçe", "tr_TR", "tr_TR", "%-d %B %Y"),
    ("uk", "Українська", "uk_UA", "uk_UA", "%-d %B %Y р."),
    ("ur", "اردو", "ur_PK", "ur_PK", "%-d %B %Y"),
    ("uz", "Oʻzbekcha", "uz_UZ", "uz_UZ", "%-d %B %Y"),
    ("vi", "Tiếng Việt", "vi_VN", "vi_VN", "%-d %B, %Y"),
    ("zh", "中文", "zh_CN", "zh_CN", "%Y年%-m月%-d日"),
    ("zh-hans", "简体中文", "zh_CN", "zh_CN", "%Y年%-m月%-d日"),
    ("zh-hant", "繁體中文", "zh_TW", "zh_TW", "%Y年%-m月%-d日"),
    ("zh-cn", "简体中文", "zh_CN", "zh_CN", "%Y年%-m月%-d日"),
    ("zh-tw", "繁體中文", "zh_TW", "zh_TW", "%Y年%-m月%-d日"),
];

fn row(code: &str) -> Option<&'static (&'static str, &'static str, &'static str, &'static str, &'static str)> {
    let code = code.to_ascii_lowercase();
    LANGS.iter().find(|r| r.0 == code).or_else(|| LANGS.iter().find(|r| Some(r.0) == code.split('-').next()))
}

/// A language code as Lanternlog keeps it: lowercase, such as `de`, `pt-br` or `zh-hans`. None when it isn't one.
pub fn code(raw: &str) -> Option<String> {
    let c = raw.trim().to_ascii_lowercase().replace('_', "-");
    let parts: Vec<&str> = c.split('-').collect();
    let letters = |p: &str, n: usize| p.len() == n && p.bytes().all(|b| b.is_ascii_lowercase());
    let region = |p: &str| letters(p, 2) || (p.len() == 3 && p.bytes().all(|b| b.is_ascii_digit()));
    // A language, then an optional script (hans), then an optional region (br) or UN M.49 area (419), as hreflang takes them.
    let rest_ok = match parts.get(1..).unwrap_or(&[]) {
        [] => true,
        [one] => letters(one, 4) || region(one),
        [script, area] => letters(script, 4) && region(area),
        _ => false,
    };
    let first = parts.first().copied().unwrap_or("");
    let ok = (letters(first, 2) || letters(first, 3))
        && rest_ok
        // These are addresses of their own on every blog.
        && !["api", "og"].contains(&c.as_str());
    ok.then_some(c)
}

/// Whether two codes name the same language (`pt-BR` and `pt-br`).
pub fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// What the language calls itself, for the language switcher.
pub fn name(code: &str) -> String {
    match LANGS.iter().find(|r| r.0.eq_ignore_ascii_case(code)) {
        Some(r) => r.1.to_string(),
        None => row(code).map_or_else(|| code.to_string(), |r| format!("{} ({code})", r.1)),
    }
}

/// The Open Graph locale, such as `de_DE` or `pt_BR`.
pub fn og(code: &str) -> String {
    let lower = code.to_ascii_lowercase();
    if let Some(r) = LANGS.iter().find(|r| r.0 == lower) {
        return r.2.to_string();
    }
    match lower.split_once('-') {
        Some((l, region)) if region.len() == 2 => format!("{l}_{}", region.to_ascii_uppercase()),
        _ => row(code).map_or_else(|| lower.clone(), |r| r.2.to_string()),
    }
}

/// Languages written right to left.
pub fn rtl(code: &str) -> bool {
    ["ar", "fa", "he", "ur", "ps", "yi", "dv", "ckb"].contains(&code.split('-').next().unwrap_or("").to_ascii_lowercase().as_str())
}

/// The words of the blog's chrome, in English. A blog can say each of them in other languages through
/// `locales.<lang>.strings`; whatever it leaves out stays English.
pub const STRINGS: &[(&str, &str)] = &[
    ("skip", "Skip to content"),
    ("theme", "Switch between light and dark themes"),
    ("site_nav", "Site"),
    ("breadcrumb", "Breadcrumb"),
    ("on_this_page", "On this page"),
    ("read_next", "Read next"),
    ("from_the_blog", "From the blog"),
    ("min_read", "{n} min read"),
    ("updated", "updated"),
    ("nothing_yet", "Nothing has been published here yet."),
    ("not_found", "Nothing here"),
    ("not_found_lead", "This page doesn't exist, but these do."),
    ("tag_description", "Posts about {tag} on {site}."),
    ("languages", "Languages"),
    ("feeds_nav", "Feeds"),
    ("feed", "Feed"),
    ("sitemap", "Sitemap"),
    ("published_with", "Published with"),
    ("date", ""),
];

/// The chrome's words for one language: the blog's own where it set them, English elsewhere.
pub struct Words<'a> {
    lang: String,
    own: Option<&'a BTreeMap<String, String>>,
}

impl<'a> Words<'a> {
    pub fn new(lang: &str, own: Option<&'a BTreeMap<String, String>>) -> Self {
        Words { lang: lang.to_string(), own }
    }

    pub fn get(&self, key: &str) -> &str {
        match self.own.and_then(|o| o.get(key)).filter(|v| !v.trim().is_empty()) {
            Some(v) => v,
            None => STRINGS.iter().find(|(k, _)| *k == key).map_or("", |(_, v)| v),
        }
    }

    /// "4 min read", in the page's language.
    pub fn min_read(&self, n: usize) -> String {
        self.get("min_read").replace("{n}", &n.to_string())
    }

    /// A date written the way the language writes it, with its own month names.
    pub fn day(&self, ts: i64) -> String {
        let Some(d) = DateTime::from_timestamp(ts, 0) else { return String::new() };
        let r = row(&self.lang);
        let own = self.get("date");
        let fmt = if own.is_empty() { r.map_or("%Y-%m-%d", |r| r.4) } else { own };
        let locale = r.and_then(|r| chrono::Locale::try_from(r.3).ok()).unwrap_or(chrono::Locale::en_US);
        // A format a blog wrote itself might be malformed; that must not take the page down.
        let mut out = String::new();
        if std::fmt::Write::write_fmt(&mut out, format_args!("{}", d.format_localized(fmt, locale))).is_err() {
            return d.format("%Y-%m-%d").to_string();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_names_and_locales() {
        assert_eq!(code("pt-BR").as_deref(), Some("pt-br"));
        assert_eq!(code("zh_Hans").as_deref(), Some("zh-hans"));
        assert_eq!(code("es-419").as_deref(), Some("es-419"));
        assert_eq!(code("zh-Hant-TW").as_deref(), Some("zh-hant-tw"));
        for bad in ["", "e", "english", "api", "og", "de-", "../de", "de/x", "1a", "de-1996", "zh-hans-cn-x", "zh-cn-hans", "en-us-gb"] {
            assert_eq!(code(bad), None, "{bad}");
        }
        assert_eq!(name("zh-hant"), "繁體中文");
        assert_eq!(name("pt-br"), "Português (Brasil)");
        assert_eq!(name("de-at"), "Deutsch (de-at)");
        assert_eq!(name("xx"), "xx");
        assert_eq!((og("de"), og("pt-br"), og("zh-hans"), og("de-at"), og("ar")), ("de_DE".into(), "pt_BR".into(), "zh_CN".into(), "de_AT".into(), "ar_AR".into()));
        assert!(rtl("ar") && rtl("fa") && !rtl("de"));
    }

    #[test]
    fn dates_and_words_in_each_language() {
        let ts = 1_793_059_200; // 2026-10-27
        assert_eq!(Words::new("en", None).day(ts), "October 27, 2026");
        assert_eq!(Words::new("de", None).day(ts), "27. Oktober 2026");
        assert_eq!(Words::new("ja", None).day(ts), "2026年10月27日");
        assert_eq!(Words::new("pt-br", None).day(ts), "27 de outubro de 2026");
        assert_eq!(Words::new("xx", None).day(ts), "2026-10-27");
        let own: BTreeMap<String, String> =
            [("min_read".to_string(), "{n} Min. Lesezeit".to_string()), ("date".to_string(), "%d.%m.%Y".to_string())].into();
        let de = Words::new("de", Some(&own));
        assert_eq!((de.min_read(4), de.day(ts), de.get("read_next")), ("4 Min. Lesezeit".into(), "27.10.2026".into(), "Read next"));
        let broken: BTreeMap<String, String> = [("date".to_string(), "%Q %".to_string())].into();
        assert!(!Words::new("de", Some(&broken)).day(ts).is_empty());
    }
}
