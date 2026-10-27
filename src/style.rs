//! The look: one inline stylesheet with light and dark themes, a toggle that remembers the reader's choice,
//! and link and button colors worked out from the site's accent so they always have enough contrast.

const LIGHT_BG: [f64; 3] = [251.0, 250.0, 247.0];
const LIGHT_SOFT: [f64; 3] = [242.0, 240.0, 235.0];
const DARK_BG: [f64; 3] = [15.0, 16.0, 18.0];
const DARK_SOFT: [f64; 3] = [26.0, 28.0, 32.0];

pub fn rgb(hex: &str) -> Option<[f64; 3]> {
    let h = hex.strip_prefix('#')?;
    let h: String = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect() } else { h.to_string() };
    let v = u32::from_str_radix(&h, 16).ok().filter(|_| h.len() == 6)?;
    Some([((v >> 16) & 255) as f64, ((v >> 8) & 255) as f64, (v & 255) as f64])
}

fn luminance(c: [f64; 3]) -> f64 {
    let f = |v: f64| {
        let v = v / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])
}

pub fn contrast(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

fn hex(c: [f64; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0].round() as u8, c[1].round() as u8, c[2].round() as u8)
}

/// The accent mixed toward `toward` just far enough to read at 4.6:1 on both backgrounds.
fn readable(accent: [f64; 3], backgrounds: [[f64; 3]; 2], toward: [f64; 3]) -> String {
    for step in 0..=40 {
        let t = f64::from(step) / 40.0;
        let c = [0, 1, 2].map(|k| accent[k] + (toward[k] - accent[k]) * t);
        if backgrounds.iter().all(|&bg| contrast(c, bg) >= 4.6) {
            return hex(c);
        }
    }
    hex(toward)
}

/// Link colors for the light and dark themes. Buttons use the light one with white text.
pub fn links(accent: &str) -> (String, String) {
    let a = rgb(accent).unwrap_or([37.0, 99.0, 235.0]);
    (readable(a, [LIGHT_BG, LIGHT_SOFT], [0.0; 3]), readable(a, [DARK_BG, DARK_SOFT], [255.0; 3]))
}

const DARK: &str = "color-scheme:dark;--link:LINK_DARK;--bg:#0f1012;--fg:#e8e7e4;--muted:#a3a6ac;--line:#26282d;--soft:#1a1c20;--card:#15171a;--code:#16181c";

const CSS: &str = r#":root{color-scheme:light;--accent:ACCENT;--link:LINK_LIGHT;--button:LINK_LIGHT;--bg:#fbfaf7;--fg:#1d1d1f;--muted:#5c6068;--line:#e7e4dd;--soft:#f2f0eb;--card:#fff;--code:#f4f2ed;--sans:system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;--serif:Charter,"Bitstream Charter","Sitka Text",Cambria,"Iowan Old Style",Georgia,serif;--mono:ui-monospace,"SF Mono",SFMono-Regular,Menlo,Consolas,"Liberation Mono",monospace}
:root[data-theme=dark]{DARK}
@media (prefers-color-scheme:dark){:root:not([data-theme=light]){DARK}}
*,::before,::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%;scroll-padding-top:1.5rem}
@media (prefers-reduced-motion:no-preference){html{scroll-behavior:smooth}}
body{margin:0;background:var(--bg);color:var(--fg);font:400 1rem/1.6 var(--sans);-webkit-font-smoothing:antialiased;text-rendering:optimizeLegibility}
::selection{background:color-mix(in srgb,var(--accent) 30%,transparent)}
a{color:var(--link);text-decoration-thickness:1px;text-underline-offset:.18em}
a:hover{text-decoration-thickness:2px}
:focus-visible{outline:2px solid var(--link);outline-offset:3px;border-radius:4px}
.skip{position:absolute;left:-999px;top:.6rem;z-index:20;background:var(--card);color:var(--fg);padding:.5rem .8rem;border-radius:8px}
.skip:focus{left:.75rem}
.wrap{max-width:44rem;margin:0 auto;padding:0 1.25rem}
.top{border-bottom:1px solid var(--line)}
.top .wrap{display:flex;align-items:center;flex-wrap:wrap;gap:.5rem 1.25rem;min-height:4rem;padding-top:.6rem;padding-bottom:.6rem}
.brand{display:flex;align-items:center;gap:.6rem;margin-right:auto;color:var(--fg);text-decoration:none;font-weight:700;letter-spacing:-.01em}
.logo{width:1.75rem;height:1.75rem;object-fit:contain;border-radius:.4rem}
.mark{display:grid;place-items:center;width:1.65rem;height:1.65rem;border-radius:.5rem;background:var(--button);color:#fff;font-size:.9rem;font-weight:800}
.nav{display:flex;align-items:center;flex-wrap:wrap;gap:.25rem 1.1rem;font-size:.95rem}
.nav a{color:var(--muted);text-decoration:none;padding:.4rem 0}
.nav a:hover{color:var(--fg)}
.nav .project{color:var(--link);font-weight:600}
.theme{display:grid;place-items:center;width:2.4rem;height:2.4rem;padding:0;border:1px solid var(--line);border-radius:50%;background:var(--card);color:var(--fg);cursor:pointer}
.theme svg{width:1.05rem;height:1.05rem}
.theme .sun,:root[data-theme=dark] .theme .moon{display:none}
:root[data-theme=dark] .theme .sun{display:block}
@media (prefers-color-scheme:dark){:root:not([data-theme=light]) .theme .sun{display:block}:root:not([data-theme=light]) .theme .moon{display:none}}
main.wrap{padding-top:2.5rem;padding-bottom:1rem}
h1,h2,h3,h4{font-family:var(--sans);color:var(--fg);text-wrap:balance}
h1{font-size:clamp(2rem,1.35rem + 2.6vw,2.85rem);line-height:1.1;letter-spacing:-.025em;font-weight:780;margin:.4rem 0 1rem}
.lead,.dek{font-family:var(--serif);font-size:1.3rem;line-height:1.55;color:var(--muted);margin:0 0 1.5rem;text-wrap:pretty}
.prose{font-family:var(--serif);font-size:1.2rem;line-height:1.72}
.prose p,.prose li{text-wrap:pretty}
.prose p{margin:0 0 1.3rem}
.prose h2{font-size:1.55rem;line-height:1.25;letter-spacing:-.015em;margin:2.7rem 0 .8rem}
.prose h3{font-size:1.22rem;line-height:1.3;margin:2.1rem 0 .6rem}
.prose ul,.prose ol{margin:0 0 1.3rem;padding-left:1.4rem}
.prose li{margin:.35rem 0}
.prose li::marker{color:var(--muted)}
.prose blockquote{margin:1.8rem 0;padding:.1rem 0 .1rem 1.25rem;border-left:3px solid var(--accent);color:var(--muted);font-style:italic}
.prose img,.prose video,.prose iframe{display:block;max-width:100%;height:auto;margin:1.8rem auto;border-radius:12px}
.prose hr{width:5rem;margin:2.8rem auto;border:0;border-top:1px solid var(--line)}
.prose .table{margin:1.8rem 0}
.prose table{width:100%;border-collapse:collapse;font:.95rem/1.5 var(--sans)}
.prose th,.prose td{padding:.6rem .8rem;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}
.prose thead th{font-weight:650;border-bottom-width:2px}
.prose tbody th{font-weight:650}
.prose td code,.prose dd code{overflow-wrap:break-word}.prose tbody th code{white-space:nowrap}.prose dt code{overflow-wrap:anywhere}
.prose dl{margin:1.8rem 0;font:.98rem/1.55 var(--sans);border-top:1px solid var(--line)}
.prose dl>div{display:grid;grid-template-columns:minmax(8rem,34%) minmax(0,1fr);gap:.3rem 1.2rem;padding:.7rem 0;border-bottom:1px solid var(--line)}
.prose dt{font-weight:650}
.prose dd{margin:0;color:var(--muted)}
@media (max-width:45rem){.prose dl>div{display:block}.prose dd{margin-top:.25rem}
.prose table,.prose tbody,.prose tr,.prose tbody th,.prose td{display:block}
.prose thead{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap}
.prose tbody tr{margin:0 0 .8rem;padding:.35rem 0 .5rem;border:1px solid var(--line);border-radius:12px;background:var(--card)}
.prose tbody th{padding:.55rem .9rem .3rem;border:0}.prose tbody th code{white-space:normal;overflow-wrap:anywhere}
.prose td{display:grid;grid-template-columns:7rem minmax(0,1fr);gap:.75rem;padding:.3rem .9rem;border:0}
.prose td::before{content:attr(data-label);color:var(--muted);font-size:.88rem}}
.prose .button{font-size:1rem}
code{font-family:var(--mono);font-size:.86em}
:not(pre)>code{padding:.12em .38em;border-radius:6px;background:var(--soft)}
pre{margin:1.8rem 0;padding:1rem 1.15rem;overflow-x:auto;border:1px solid var(--line);border-radius:12px;background:var(--code);font-size:.95rem;line-height:1.6}
.prose pre{font-size:.8rem}
pre code{font-size:inherit}
.anchor{margin-left:.45rem;color:var(--muted);font-weight:400;text-decoration:none;opacity:0}
h2:hover>.anchor,h3:hover>.anchor{opacity:1}
.footnote-definition,.footnotes{font-size:.92rem;color:var(--muted)}
.crumbs{font-size:.9rem}
.crumbs a{color:var(--muted);text-decoration:none}
.crumbs a:hover{color:var(--fg)}
.meta{display:flex;flex-wrap:wrap;gap:.2rem .5rem;margin:0 0 2.2rem;font-size:.9rem;color:var(--muted)}
.meta a{color:inherit}
.cover{display:block;width:100%;height:auto;margin:0 0 2.2rem;border-radius:16px}
.toc{margin:0 0 2.4rem;padding:.9rem 1.2rem;border:1px solid var(--line);border-radius:14px;background:var(--card);font-size:.95rem}
.toc summary{cursor:pointer;font-weight:650}
.toc ol{margin:.6rem 0 .2rem;padding-left:1.2rem}
.toc li{margin:.3rem 0}
.toc .l3{margin-left:1rem;list-style:circle}
.tags{display:flex;flex-wrap:wrap;gap:.5rem;margin:2.6rem 0 0;padding:0;list-style:none}
.tags a{display:inline-block;padding:.3rem .75rem;border:1px solid var(--line);border-radius:999px;font-size:.85rem;text-decoration:none}
.cta{position:relative;overflow:hidden;margin:3rem 0 1rem;padding:1.5rem 1.6rem 1.5rem 1.9rem;border:1px solid var(--line);border-radius:16px;background:var(--card)}
.cta::before{content:"";position:absolute;inset:0 auto 0 0;width:4px;background:var(--accent)}
.cta p{margin:0 0 1rem;font-size:1.05rem;line-height:1.55}
.button{display:inline-flex;align-items:center;gap:.4rem;padding:.7rem 1.15rem;border-radius:10px;background:var(--button);color:#fff;font-family:var(--sans);font-weight:650;text-decoration:none}
.button:hover{filter:brightness(1.1)}
.section{margin:3.2rem 0 .2rem;font-size:.8rem;font-weight:650;letter-spacing:.08em;text-transform:uppercase;color:var(--muted)}
.posts{margin:.8rem 0 0;padding:0;list-style:none}
.posts li{padding:1.4rem 0;border-top:1px solid var(--line)}
.posts li:first-child{border-top:0}
.posts time{font-size:.8rem;letter-spacing:.05em;text-transform:uppercase;color:var(--muted)}
.posts h2,.posts h3{margin:.3rem 0 .45rem;font-size:1.4rem;line-height:1.3;letter-spacing:-.01em}
.posts h2 a,.posts h3 a{color:var(--fg);text-decoration:none}
.posts h2 a:hover,.posts h3 a:hover{color:var(--link)}
.posts p{margin:0;font-family:var(--serif);font-size:1.06rem;line-height:1.6;color:var(--muted)}
.progress{position:fixed;inset:0 0 auto;z-index:10;height:3px;background:var(--accent);transform:scaleX(0);transform-origin:0 50%}
@supports (animation-timeline:scroll()){.progress{animation:read linear both;animation-timeline:scroll(root)}}
@keyframes read{to{transform:scaleX(1)}}
.foot{margin-top:4rem;border-top:1px solid var(--line);font-size:.88rem;color:var(--muted)}
.foot .wrap{display:grid;gap:.6rem;padding-top:2rem;padding-bottom:3rem}
.foot p{margin:0}
.foot strong{color:var(--fg)}
.foot nav{display:flex;flex-wrap:wrap;gap:.4rem 1.1rem}
.foot a{color:var(--muted)}
@media (max-width:640px){.nav{order:3;width:100%;gap:.25rem 1rem}.prose table{font-size:.86rem}.prose th,.prose td{padding:.5rem .55rem}main.wrap{padding-top:1.75rem}.prose{font-size:1.1rem;line-height:1.68}.lead,.dek{font-size:1.15rem}.posts h2,.posts h3{font-size:1.25rem}}"#;

pub fn css(accent: &str, sans: bool) -> String {
    let (light, dark) = links(accent);
    let css = CSS.replace("DARK", &DARK.replace("LINK_DARK", &dark)).replace("LINK_LIGHT", &light).replace("ACCENT", accent);
    if sans { css + "\n:root{--serif:var(--sans)}" } else { css }
}

/// Runs before the first paint, so a reader who picked a theme never sees the other one flash.
pub const HEAD_JS: &str = r#"try{var t=localStorage.getItem("theme");if(t)document.documentElement.dataset.theme=t}catch(e){}"#;

pub const TOGGLE_JS: &str = r#"document.getElementById("theme").addEventListener("click",function(){var d=document.documentElement,n=(d.dataset.theme||(matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light"))=="dark"?"light":"dark";d.dataset.theme=n;try{localStorage.setItem("theme",n)}catch(e){}})"#;

pub const SUN: &str = r#"<svg class="sun" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="4.5"/><path d="M12 2v2m0 16v2M4.9 4.9l1.4 1.4m11.4 11.4 1.4 1.4M2 12h2m16 0h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>"#;

pub const MOON: &str = r#"<svg class="moon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"/></svg>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_accent_yields_readable_links() {
        for accent in ["#f59e0b", "#ffff00", "#2563eb", "#000000", "#ffffff", "#e4572e", "nonsense"] {
            let (light, dark) = links(accent);
            let (l, d) = (rgb(&light).unwrap_or_default(), rgb(&dark).unwrap_or_default());
            assert!(contrast(l, LIGHT_BG) >= 4.5 && contrast(l, LIGHT_SOFT) >= 4.5, "{accent} light {light}");
            assert!(contrast(d, DARK_BG) >= 4.5 && contrast(d, DARK_SOFT) >= 4.5, "{accent} dark {dark}");
            assert!(contrast(l, [255.0; 3]) >= 4.5, "{accent} button {light}");
        }
    }
}
