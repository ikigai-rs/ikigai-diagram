use ikigai_core::{Door, MatchKind, SpaceKind, Topology};

#[test]
fn empty_chain_description_does_not_invent_a_root() {
    let tree = Topology::new(SpaceKind::Chain { severed: false });
    let svg = ikigai_diagram::render(&tree);
    let doc = roxmltree::Document::parse(&svg).expect("valid SVG XML");
    let desc = doc
        .descendants()
        .find(|n| n.has_tag_name("desc"))
        .unwrap()
        .text()
        .unwrap();
    assert!(desc.contains("The resolution chain is empty."), "{desc}");
}

#[test]
fn escaped_labels_are_xml_and_the_accessible_name_resolves() {
    let tree = Topology::new(SpaceKind::EndpointSpace {
        doors: vec![Door::new(
            "<&\"' café 日本",
            MatchKind::Exact,
            "<&\"' café 日本",
        )],
    });
    let svg = ikigai_diagram::render(&tree);
    let doc = roxmltree::Document::parse(&svg).expect("valid SVG XML");
    let root = doc.root_element();
    assert_eq!(root.attribute("role"), Some("img"));
    for id in root
        .attribute("aria-labelledby")
        .unwrap()
        .split_whitespace()
    {
        assert!(doc.descendants().any(|n| n.attribute("id") == Some(id)));
    }
    assert!(doc
        .descendants()
        .filter_map(|n| n.text())
        .any(|t| t.contains("<&\"' café 日本")));
    assert_eq!(svg, ikigai_diagram::render(&tree));
}

#[test]
#[ignore = "known limitation: the image description summarizes doors without their details"]
fn description_includes_the_door_details_visible_in_the_picture() {
    let tree = Topology::new(SpaceKind::EndpointSpace {
        doors: vec![Door::new(
            "urn:example:hello",
            MatchKind::Exact,
            "hello-handler",
        )],
    });
    let svg = ikigai_diagram::render(&tree);
    let doc = roxmltree::Document::parse(&svg).expect("valid SVG XML");
    let desc = doc
        .descendants()
        .find(|n| n.has_tag_name("desc"))
        .unwrap()
        .text()
        .unwrap();
    assert!(
        desc.contains("urn:example:hello")
            && desc.contains("hello-handler")
            && desc.contains("exact"),
        "description omits visible door details: {desc}"
    );
}

#[test]
fn fixture_pictures_parse_and_state_follows_the_arrangement() {
    for fixture in ["every-kind", "tic-tac-toe", "named-module", "dynamic"] {
        let turtle = std::fs::read_to_string(format!("tests/fixtures/{fixture}.ttl")).unwrap();
        let tree = Topology::from_turtle(&turtle).unwrap();
        let svg = ikigai_diagram::render(&tree);
        roxmltree::Document::parse(&svg).expect("valid fixture SVG");
        assert_eq!(svg, ikigai_diagram::render(&tree));
    }
    let turtle = std::fs::read_to_string("tests/fixtures/state/governed.ttl").unwrap();
    let tree = Topology::from_turtle(&turtle).unwrap();
    let board = ikigai_diagram::state::Board::from_turtle(
        &std::fs::read_to_string("tests/fixtures/state/every-state.ttl").unwrap(),
    )
    .unwrap();
    let svg = ikigai_diagram::render_with_state(&tree, &board);
    let doc = roxmltree::Document::parse(&svg).expect("valid state SVG");
    assert_eq!(doc.root_element().attribute("role"), Some("group"));
    let roles: Vec<_> = doc
        .root_element()
        .children()
        .filter_map(|n| n.attribute("role"))
        .collect();
    assert_eq!(roles, ["img", "status"]);
    let status = doc
        .descendants()
        .find(|n| n.attribute("role") == Some("status"))
        .unwrap();
    assert_eq!(status.attribute("aria-live"), Some("polite"));
    assert_eq!(status.attribute("aria-atomic"), Some("true"));
    assert_eq!(svg, ikigai_diagram::render_with_state(&tree, &board));
}
