//! Every color pair the picture draws meets the floor this ecosystem enforces, in
//! both schemes — measured with `ikigai-a11y`'s own contrast math and thresholds,
//! over the same token tables the picture's `<style>` is generated from.

use ikigai_a11y::config::DEFAULT_MIN;
use ikigai_a11y::{ratio, Rgba};
use ikigai_diagram::palette::{
    Scheme, StateScheme, DARK, DARK_CHANGED, DARK_STATE, LIGHT, LIGHT_CHANGED, LIGHT_STATE,
};

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

/// Every pair the STATE layer draws, over its scheme's page: text and muted text on a
/// governor's band and on the summary's header band (the same token), the summary's
/// outline on the page, and every state mark as text on the page.
fn state_pairs(
    scheme: &Scheme,
    state: &StateScheme,
) -> Vec<(String, &'static str, &'static str, f64)> {
    let mut out = vec![
        (
            "text on the state band".to_string(),
            scheme.text,
            state.band,
            DEFAULT_MIN,
        ),
        (
            "muted text on the state band".to_string(),
            scheme.muted,
            state.band,
            DEFAULT_MIN,
        ),
        (
            "the state outline on the page".to_string(),
            state.outline,
            scheme.background,
            NON_TEXT_MIN,
        ),
    ];
    for (mark, fg) in state.marks {
        out.push((
            format!("the {mark} state mark on the page"),
            fg,
            scheme.background,
            DEFAULT_MIN,
        ));
    }
    let changed = if state.name == "dark" {
        DARK_CHANGED
    } else {
        LIGHT_CHANGED
    };
    out.push((
        "the CHANGED marker on the page".to_string(),
        changed,
        scheme.background,
        DEFAULT_MIN,
    ));
    out
}

#[test]
fn every_pair_meets_the_floor_in_both_schemes() {
    let mut failures = Vec::new();
    for (scheme, state) in [(&LIGHT, &LIGHT_STATE), (&DARK, &DARK_STATE)] {
        assert_eq!(scheme.name, state.name);
        for (what, fg, bg, floor) in pairs(scheme).into_iter().chain(state_pairs(scheme, state)) {
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
    for state in [&LIGHT_STATE, &DARK_STATE] {
        tokens.push(state.outline.to_ascii_lowercase());
        tokens.push(state.band.to_ascii_lowercase());
        for (_, fg) in state.marks {
            tokens.push(fg.to_ascii_lowercase());
        }
    }
    let read = |relative: &str| {
        std::fs::read_to_string(format!("{}/tests/{relative}", env!("CARGO_MANIFEST_DIR")))
            .expect("the fixture")
    };
    let mut pictures: Vec<(&str, String)> = Vec::new();
    for text in ["every-kind", "tic-tac-toe", "named-module", "dynamic"] {
        let ttl = read(&format!("fixtures/{text}.ttl"));
        let tree = ikigai_core::Topology::from_turtle(&ttl).expect("it parses");
        pictures.push((text, ikigai_diagram::render(&tree)));
    }
    // The state layer, over every state value.
    let governed = ikigai_core::Topology::from_turtle(&read("fixtures/state/governed.ttl"))
        .expect("it parses");
    for board in ["board", "every-state"] {
        let board = ikigai_diagram::state::Board::from_turtle(&read(&format!(
            "fixtures/state/{board}.ttl"
        )))
        .expect("the board parses");
        pictures.push((
            "state",
            ikigai_diagram::render_with_state(&governed, &board),
        ));
    }
    for (text, svg) in pictures {
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

#[test]
fn the_state_layer_carries_both_schemes_and_its_dark_one_is_behind_the_media_query() {
    let read = |relative: &str| {
        std::fs::read_to_string(format!("{}/tests/{relative}", env!("CARGO_MANIFEST_DIR")))
            .expect("the fixture")
    };
    let tree = ikigai_core::Topology::from_turtle(&read("fixtures/state/governed.ttl"))
        .expect("it parses");
    let board = ikigai_diagram::state::Board::from_turtle(&read("fixtures/state/board.ttl"))
        .expect("the board parses");
    let svg = ikigai_diagram::render_with_state(&tree, &board);
    let light = format!(".ikd-t-state{{fill:{}}}", LIGHT_STATE.band);
    let dark = format!(".ikd-t-state{{fill:{}}}", DARK_STATE.band);
    let at_light = svg.find(&light).expect("the light state band");
    let at_dark = svg.find(&dark).expect("the dark state band");
    let media = svg[..at_dark]
        .rfind("@media (prefers-color-scheme:dark){")
        .expect("a media query before the dark state tokens");
    assert!(at_light < media, "the light tokens are outside the query");
}
