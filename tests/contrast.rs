//! Every color pair the picture draws meets the floor this ecosystem enforces, in
//! both schemes — measured with `ikigai-a11y`'s own contrast math and thresholds,
//! over the same token tables the picture's `<style>` is generated from.

use ikigai_a11y::config::DEFAULT_MIN;
use ikigai_a11y::{ratio, Rgba};
use ikigai_diagram::palette::{Scheme, DARK, LIGHT};

/// WCAG 2.2 SC 1.4.11 (non-text contrast): a box's outline is what tells one box
/// from the next, so it holds 3:1 against the page it is drawn on.
const NON_TEXT_MIN: f64 = 3.0;

fn color(hex: &str) -> Rgba {
    Rgba::parse(hex).unwrap_or_else(|e| panic!("{hex} is not a color: {e}"))
}

/// Every (what, foreground, background, floor) pair a scheme draws.
fn pairs(scheme: &Scheme) -> Vec<(String, &'static str, &'static str, f64)> {
    let mut out = vec![
        (
            "text on the page".to_string(),
            scheme.text,
            scheme.background,
            DEFAULT_MIN,
        ),
        (
            "muted text on the page".to_string(),
            scheme.muted,
            scheme.background,
            DEFAULT_MIN,
        ),
    ];
    for (class, outline, band) in scheme.kinds {
        out.push((
            format!("text on the {class} band"),
            scheme.text,
            band,
            DEFAULT_MIN,
        ));
        out.push((
            format!("muted text on the {class} band"),
            scheme.muted,
            band,
            DEFAULT_MIN,
        ));
        out.push((
            format!("the {class} outline on the page"),
            outline,
            scheme.background,
            NON_TEXT_MIN,
        ));
    }
    for (mark, fg) in scheme.marks {
        out.push((
            format!("the {mark} mark on the page"),
            fg,
            scheme.background,
            DEFAULT_MIN,
        ));
    }
    out
}

#[test]
fn every_pair_meets_the_floor_in_both_schemes() {
    let mut failures = Vec::new();
    for scheme in [&LIGHT, &DARK] {
        for (what, fg, bg, floor) in pairs(scheme) {
            let r = ratio(color(fg), color(bg));
            if r < floor {
                failures.push(format!(
                    "{}: {what} ({fg} on {bg}) is {r:.2}:1, below {floor}:1",
                    scheme.name
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_picture_carries_both_schemes_and_the_dark_one_is_behind_the_media_query() {
    let svg = ikigai_diagram::render(&ikigai_core::Topology::new(ikigai_core::SpaceKind::Opaque));
    let (light, dark) = svg
        .split_once("@media (prefers-color-scheme:dark)")
        .expect("a dark scheme");
    assert!(light.contains(&format!(".ikd-bg{{fill:{}}}", LIGHT.background)));
    assert!(dark.contains(&format!(".ikd-bg{{fill:{}}}", DARK.background)));
    // The page is painted, so a dark viewer never sees dark text on a transparent
    // SVG over a dark page.
    assert!(svg.contains("<rect class=\"ikd-bg\""));
}

/// Every color the picture states, as `#rrggbb`, in the order it appears.
fn hex_colors(svg: &str) -> Vec<String> {
    let bytes = svg.as_bytes();
    let mut out = Vec::new();
    for (i, _) in svg.match_indices('#') {
        let end = (i + 7).min(bytes.len());
        let candidate = &svg[i..end];
        if candidate.len() == 7 && candidate[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
            out.push(candidate.to_ascii_lowercase());
        }
    }
    out
}

#[test]
fn the_picture_draws_no_color_outside_the_token_tables() {
    let mut tokens: Vec<String> = Vec::new();
    for scheme in [&LIGHT, &DARK] {
        tokens.push(scheme.background.to_ascii_lowercase());
        tokens.push(scheme.text.to_ascii_lowercase());
        tokens.push(scheme.muted.to_ascii_lowercase());
        for (_, outline, band) in scheme.kinds {
            tokens.push(outline.to_ascii_lowercase());
            tokens.push(band.to_ascii_lowercase());
        }
        for (_, fg) in scheme.marks {
            tokens.push(fg.to_ascii_lowercase());
        }
    }
    for text in ["every-kind", "tic-tac-toe", "named-module"] {
        let ttl = std::fs::read_to_string(format!(
            "{}/tests/fixtures/{text}.ttl",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("the fixture");
        let tree = ikigai_core::Topology::from_turtle(&ttl).expect("it parses");
        let svg = ikigai_diagram::render(&tree);
        let drawn = hex_colors(&svg);
        assert!(!drawn.is_empty(), "{text}: the picture states no colors");
        for c in &drawn {
            assert!(tokens.contains(c), "{text}: {c} is not a palette token");
        }
        // Colors reach the page only through the stylesheet's classes: no inline
        // `style`, and the only paint attribute values are keywords.
        assert!(!svg.contains("style=\""), "{text}: an inline style");
        for attr in ["fill=\"", "stroke=\""] {
            for (i, _) in svg.match_indices(attr) {
                let value = svg[i + attr.len()..].split('"').next().unwrap_or("");
                assert!(
                    matches!(value, "none" | "currentColor" | "transparent"),
                    "{text}: {attr}{value}\" is not a keyword"
                );
            }
        }
    }
}
