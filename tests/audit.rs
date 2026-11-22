//! The text alternative, read the way assistive technology reads it: the SVG parsed as
//! XML, the accessible name and description resolved through their ids.

use ikigai_core::{Door, MatchKind, SpaceKind, Topology};

fn desc(svg: &str) -> String {
    let doc = roxmltree::Document::parse(svg).expect("valid SVG XML");
    doc.descendants()
        .find(|n| n.has_tag_name("desc"))
        .unwrap()
        .text()
        .unwrap()
        .to_string()
}

#[test]
fn description_includes_the_door_details_visible_in_the_picture() {
    let tree = Topology::new(SpaceKind::EndpointSpace {
        doors: vec![Door::new(
            "urn:example:hello",
            MatchKind::Exact,
            "hello-handler",
        )],
    });
    let svg = ikigai_diagram::render(&tree);
    let desc = desc(&svg);
    assert!(
        desc.contains("urn:example:hello")
            && desc.contains("hello-handler")
            && desc.contains("exact"),
        "description omits visible door details: {desc}"
    );
}

/// The root's accessible name and description, resolved through their ids.
fn name_and_description(svg: &str) -> (String, String) {
    let doc = roxmltree::Document::parse(svg).expect("valid SVG XML");
    let img = doc
        .descendants()
        .find(|n| n.attribute("role") == Some("img"))
        .expect("a role=img");
    let text_of = |ids: &str| -> String {
        ids.split_whitespace()
            .map(|id| {
                doc.descendants()
                    .find(|n| n.attribute("id") == Some(id))
                    .unwrap_or_else(|| panic!("no element with id {id}"))
                    .text()
                    .unwrap_or_default()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    (
        text_of(img.attribute("aria-labelledby").expect("aria-labelledby")),
        text_of(img.attribute("aria-describedby").expect("aria-describedby")),
    )
}

/// 20 endpoint spaces of 10 doors under one fallback: 200 doors, every pattern too long
/// for its column, so the picture cuts each one with an ellipsis.
fn large() -> Topology {
    let mut root = Topology::new(SpaceKind::Fallback);
    for s in 0..20 {
        let doors = (0..10)
            .map(|d| {
                let kind = [MatchKind::Exact, MatchKind::Prefix, MatchKind::Template][d % 3];
                Door::new(
                    format!("urn:large:space-{s}:door-{d}:{}", "segment:".repeat(12)),
                    kind,
                    format!("handler-{s}-{d}"),
                )
            })
            .collect();
        root = root.child(Topology::new(SpaceKind::EndpointSpace { doors }));
    }
    root
}

#[test]
fn a_large_arrangement_opens_with_its_summary_and_lists_every_door_in_order() {
    let tree = large();
    let svg = ikigai_diagram::render(&tree);
    let (name, description) = name_and_description(&svg);

    // The name stays short however large the arrangement: the title alone.
    assert_eq!(name, "Space arrangement");

    // The description opens with the size and shape, before any single door, so a
    // listener knows what is coming and can stop.
    let summary = "An arrangement whose outermost space is a fallback. In all: 1 fallback; and \
                   200 doors across 20 endpoint spaces.";
    assert!(description.starts_with(summary), "{}", &description[..200]);

    // Then every space and every door, in the order drawn, each untruncated though the
    // picture cuts every pattern.
    assert!(svg.contains('…'), "the picture truncates the long patterns");
    let mut at = description
        .find(" Everything drawn, in reading order: ")
        .expect("a listing");
    let mut next = |needle: &str| {
        let found = description[at..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing or out of order: {needle}"));
        at += found + needle.len();
    };
    next("Space 1: Fallback, 20 layers consulted in order; the first hit wins.");
    for s in 0..20 {
        next(&format!(
            "Space 1.{}, layer {}: Endpoints, 10 doors tried in order; the first match wins.",
            s + 1,
            s + 1
        ));
        for d in 0..10 {
            let kind = ["exact", "prefix", "template"][d % 3];
            next(&format!(
                "Door {}: urn:large:space-{s}:door-{d}:{} ({kind}) is answered by handler-{s}-{d}.",
                d + 1,
                "segment:".repeat(12)
            ));
        }
    }
    assert_eq!(&description[at..], "", "nothing after the last door");
}

#[test]
fn every_door_and_rule_row_drawn_is_in_the_description() {
    let turtle = std::fs::read_to_string("tests/fixtures/every-kind.ttl").unwrap();
    let tree = Topology::from_turtle(&turtle).unwrap();
    let svg = ikigai_diagram::render(&tree);
    let (_, description) = name_and_description(&svg);
    let doc = roxmltree::Document::parse(&svg).unwrap();
    // Each door and rule row carries its full sentence as its tooltip; the description
    // states each one, capitalized, in the order the rows are drawn.
    let rows: Vec<String> = doc
        .descendants()
        .filter(|n| n.has_tag_name("title"))
        .filter_map(|n| n.text())
        .filter(|t| t.starts_with("door ") || t.starts_with("rule "))
        .map(|t| {
            let mut c = t.chars();
            c.next().unwrap().to_uppercase().chain(c).collect()
        })
        .collect();
    assert!(rows.len() > 5, "the fixture draws doors and rules");
    let mut at = 0;
    for row in &rows {
        let found = description[at..]
            .find(row.as_str())
            .unwrap_or_else(|| panic!("missing or out of order: {row}\n{description}"));
        at += found + row.len();
    }
}

#[test]
fn a_state_picture_describes_its_arrangement_the_same_way() {
    let turtle = std::fs::read_to_string("tests/fixtures/state/governed.ttl").unwrap();
    let tree = Topology::from_turtle(&turtle).unwrap();
    let board = ikigai_diagram::state::Board::from_turtle(
        &std::fs::read_to_string("tests/fixtures/state/every-state.ttl").unwrap(),
    )
    .unwrap();
    let svg = ikigai_diagram::render_with_state(&tree, &board);
    let (name, description) = name_and_description(&svg);
    assert!(name.ends_with(", with governor state"), "{name}");
    assert!(description.contains(" Everything drawn, in reading order: Space 1: "));
    let plain = ikigai_diagram::render(&tree);
    let (_, plain_description) = name_and_description(&plain);
    let listing = |d: &str| d.split_once(" Everything drawn").unwrap().1.to_string();
    assert_eq!(listing(&description), listing(&plain_description));
}
