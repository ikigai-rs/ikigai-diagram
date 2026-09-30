//! The color tokens, one table per color scheme.
//!
//! Every color the picture uses is a named token here, and the picture's `<style>`
//! is generated from these tables and from nothing else — so the contrast test in
//! `tests/contrast.rs`, which walks the same tables, checks exactly what ships.
//! The floor is the one `ikigai-a11y` enforces: 4.5:1 for text on any surface it
//! sits on, and 3:1 for a box's outline against the page (WCAG 1.4.11, non-text
//! contrast).
//!
//! Tokens are CSS classes with literal colors, not custom properties: a class
//! renders in every SVG consumer that reads a `<style>` at all, and the dark
//! scheme is the same classes redefined under `prefers-color-scheme: dark`.

/// A kind of node, and the class suffix its tokens are written under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Chain,
    Fallback,
    Mount,
    Alias,
    Limit,
    Level,
    Endpoints,
    Opaque,
    Rewrite,
    Confine,
    /// A `SpaceKind` newer than this renderer (the enum is `#[non_exhaustive]`).
    Unknown,
    /// Not a space: the picture of a resource that does not declare an arrangement.
    Problem,
}

impl Kind {
    /// Every kind, in the order the tables list them.
    #[cfg(test)]
    pub(crate) const ALL: [Kind; 12] = [
        Kind::Chain,
        Kind::Fallback,
        Kind::Mount,
        Kind::Alias,
        Kind::Limit,
        Kind::Level,
        Kind::Endpoints,
        Kind::Opaque,
        Kind::Rewrite,
        Kind::Confine,
        Kind::Unknown,
        Kind::Problem,
    ];

    /// The class suffix: `ikd-k-{class}` outlines a box, `ikd-t-{class}` fills its
    /// header band.
    pub(crate) fn class(self) -> &'static str {
        match self {
            Kind::Chain => "chain",
            Kind::Fallback => "fallback",
            Kind::Mount => "mount",
            Kind::Alias => "alias",
            Kind::Limit => "limit",
            Kind::Level => "level",
            Kind::Endpoints => "endpoints",
            Kind::Opaque => "opaque",
            Kind::Rewrite => "rewrite",
            Kind::Confine => "confine",
            Kind::Unknown => "unknown",
            Kind::Problem => "problem",
        }
    }
}

/// One color scheme's tokens.
#[derive(Debug)]
pub struct Scheme {
    /// `light` or `dark`.
    pub name: &'static str,
    /// The page, and the body of every box.
    pub background: &'static str,
    /// Primary text.
    pub text: &'static str,
    /// Secondary text: tags, numbering, IRIs, notes.
    pub muted: &'static str,
    /// Per kind: `(class, outline, header band)`. Text of both weights sits on the
    /// header band; the outline sits on the background.
    pub kinds: [(&'static str, &'static str, &'static str); 12],
    /// The match-kind marks on door rows (`exact`, `template`, `prefix`,
    /// `custom`), as text on the background.
    pub marks: [(&'static str, &'static str); 4],
}

/// The light scheme — the default when the viewer states no preference.
pub const LIGHT: Scheme = Scheme {
    name: "light",
    background: "#ffffff",
    text: "#1f2328",
    muted: "#59636e",
    kinds: [
        ("chain", "#59636e", "#f6f8fa"),
        ("fallback", "#0969da", "#ddf4ff"),
        ("mount", "#1a7f37", "#dafbe1"),
        ("alias", "#8250df", "#fbefff"),
        ("limit", "#cf222e", "#ffebe9"),
        ("level", "#9a6700", "#fff8c5"),
        ("endpoints", "#1b7c83", "#dff7f5"),
        ("opaque", "#818b98", "#f6f8fa"),
        ("rewrite", "#bc4c00", "#fff1e5"),
        ("confine", "#953800", "#fff1e5"),
        ("unknown", "#59636e", "#f6f8fa"),
        ("problem", "#cf222e", "#ffebe9"),
    ],
    marks: [
        ("exact", "#59636e"),
        ("template", "#0969da"),
        ("prefix", "#1a7f37"),
        ("custom", "#cf222e"),
    ],
};

/// The dark scheme, selected by `prefers-color-scheme: dark`.
pub const DARK: Scheme = Scheme {
    name: "dark",
    background: "#0d1117",
    text: "#e6edf3",
    muted: "#9198a1",
    kinds: [
        ("chain", "#9198a1", "#161b22"),
        ("fallback", "#4493f8", "#0c1f3a"),
        ("mount", "#3fb950", "#0d2416"),
        ("alias", "#ab7df8", "#1d1533"),
        ("limit", "#f85149", "#2a1214"),
        ("level", "#d29922", "#261e0c"),
        ("endpoints", "#39c5cf", "#0b2326"),
        ("opaque", "#768390", "#161b22"),
        ("rewrite", "#db6d28", "#28190d"),
        ("confine", "#f0883e", "#28190d"),
        ("unknown", "#9198a1", "#161b22"),
        ("problem", "#f85149", "#2a1214"),
    ],
    marks: [
        ("exact", "#9198a1"),
        ("template", "#4493f8"),
        ("prefix", "#3fb950"),
        ("custom", "#f85149"),
    ],
};

/// The class rules for one scheme.
fn rules(scheme: &Scheme) -> String {
    let mut css = format!(
        ".ikd-bg{{fill:{bg}}}.ikd-box{{fill:{bg}}}.ikd-x{{fill:{text}}}.ikd-m{{fill:{muted}}}",
        bg = scheme.background,
        text = scheme.text,
        muted = scheme.muted,
    );
    for (class, outline, band) in scheme.kinds {
        css.push_str(&format!(
            ".ikd-k-{class}{{stroke:{outline}}}.ikd-t-{class}{{fill:{band}}}"
        ));
    }
    for (mark, color) in scheme.marks {
        css.push_str(&format!(".ikd-mk-{mark}{{fill:{color}}}"));
    }
    css
}

/// The picture's whole `<style>` body: layout rules, the light tokens, and the
/// dark tokens under `prefers-color-scheme: dark`.
pub(crate) fn stylesheet() -> String {
    format!(
        ".ikd{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,\"Liberation Mono\",monospace;\
         font-size:13px}}\
         .ikd-b{{font-weight:700}}\
         .ikd-hole{{stroke-dasharray:6 4}}.ikd-dim{{stroke-dasharray:2 3}}\
         {light}\
         @media (prefers-color-scheme:dark){{{dark}}}",
        light = rules(&LIGHT),
        dark = rules(&DARK),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_tokens_in_both_schemes_in_the_same_order() {
        for scheme in [&LIGHT, &DARK] {
            let classes: Vec<&str> = scheme.kinds.iter().map(|k| k.0).collect();
            let expected: Vec<&str> = Kind::ALL.iter().map(|k| k.class()).collect();
            assert_eq!(classes, expected, "{} scheme", scheme.name);
        }
    }
}
