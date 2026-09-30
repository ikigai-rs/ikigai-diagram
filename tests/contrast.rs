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
