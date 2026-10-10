//! A module space that names itself shows its name — the reason the space-name wave
//! (ledger #987) exists: the picture of an arrangement says WHICH spaces it holds.
//!
//! The arrangement is `common::named_module_kernel`: `ikigai_diagram::space()`, which
//! claims `urn:iki:space:diagram`, reached twice in one host. Checked on the committed
//! fixture (what core writes for that kernel, pinned by `golden.rs`) and on the live
//! kernel's topology, so neither the file nor the renderer can drift alone.

mod common;

use common::{committed, named_module_kernel};
use ikigai_core::Topology;
use ikigai_diagram::SPACE_ID;

/// The header band's IRI line: the name, as its own line of real text.
fn iri_line(iri: &str) -> String {
    format!("<tspan class=\"ikd-m\">{iri}</tspan></text>")
}

/// The text of the picture's `<desc>`.
fn desc(svg: &str) -> &str {
    let start = svg.find("<desc").expect("a <desc>");
    let open = start + svg[start..].find('>').expect("the <desc> tag closes") + 1;
    let end = svg.find("</desc>").expect("the </desc>");
    &svg[open..end]
}

fn pictures() -> Vec<(&'static str, String)> {
    let text = committed("fixtures/named-module.ttl").expect("the fixture");
    let fixture = Topology::from_turtle(&text).expect("it parses");
    vec![
        ("the committed fixture", ikigai_diagram::render(&fixture)),
        (
            "the live kernel",
            ikigai_diagram::render(&named_module_kernel().topology()),
        ),
    ]
}

#[test]
fn the_space_names_itself_in_the_header_band_and_the_desc() {
    assert_eq!(SPACE_ID, "urn:iki:space:diagram");
    for (what, svg) in pictures() {
        assert!(
            svg.contains(&iri_line(SPACE_ID)),
            "{what}: the header band shows {SPACE_ID}:\n{svg}"
        );
        assert!(
            desc(&svg).contains(SPACE_ID),
            "{what}: the desc names {SPACE_ID}: {}",
            desc(&svg)
        );
    }
}

#[test]
fn a_named_space_reached_twice_is_drawn_once_and_referenced_after() {
    for (what, svg) in pictures() {
        // Both doors drawn exactly once: each door row carries one tooltip.
        for door in ["diagram-arrangement", "diagram-kernel"] {
            let rows = svg.matches(&format!("is answered by {door}")).count();
            assert_eq!(rows, 1, "{what}: the door to {door} is drawn once:\n{svg}");
        }
        // The name heads two boxes: the drawing, then the reference.
        assert_eq!(
            svg.matches(&iri_line(SPACE_ID)).count(),
            2,
            "{what}: the space is named where it is drawn and where it is referred to:\n{svg}"
        );
        let reference = "drawn above: the same space, reached again";
        assert_eq!(svg.matches(reference).count(), 1, "{what}: one reference");
        let drawn = svg
            .find("is answered by diagram-arrangement")
            .expect("drawn");
        let referred = svg.find(reference).expect("referred to");
        assert!(
            drawn < referred,
            "{what}: the reference comes after the drawing"
        );
        // And the desc names it once: it states the spaces shown, not each box.
        assert_eq!(
            desc(&svg).matches(SPACE_ID).count(),
            1,
            "{what}: {}",
            desc(&svg)
        );
    }
}
