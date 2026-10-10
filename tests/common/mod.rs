//! The arrangements the tests draw, built from core's own constructors — so each
//! fixture file under `tests/fixtures/` is checked to be exactly what core writes
//! for a real tree, not a hand-written graph that merely parses.

#![allow(dead_code)] // each test binary uses a different subset

use std::sync::Arc;

use ikigai_core::{
    Alias, AliasTable, Door, EndpointSpace, Error, Fallback, FnEndpoint, Iri, Kernel, MatchKind,
    Mount, RuleKind, Space, SpaceKind, Topology, TopologyRule, UriTemplate,
};

/// A committed file under `tests/`, read at run time so a missing one is a test
/// failure that says what to do rather than a build that does not compile.
pub fn committed(relative: &str) -> Option<String> {
    std::fs::read_to_string(format!("{}/tests/{relative}", env!("CARGO_MANIFEST_DIR"))).ok()
}

fn iri(s: &str) -> Option<Iri> {
    Some(Iri::parse(s).expect("a fixture IRI"))
}

fn leaf(doors: Vec<Door>) -> Topology {
    Topology::new(SpaceKind::EndpointSpace { doors })
}

/// Every kind a topology has, at least once: a chain with a corridor and the root;
/// under the root a fallback holding a named mount (with exact, template and custom
/// doors, a confined door, and a pattern long enough to be cut), an alias with both
/// rule kinds, a limit, a level with seals and a namespace, a rewrite over a named
/// opaque peer, a confine, the SAME named mount again, an anonymous opaque space,
/// and an endpoint space with no doors.
pub fn every_kind() -> Topology {
    let sandbox = leaf(vec![Door::new(
        "urn:example:sandbox:read",
        MatchKind::Exact,
        "sandbox-read",
    )])
    .with_id(iri("urn:example:corridor:sandbox"));
    let module = Topology::new(SpaceKind::Mount {
        prefix: "urn:example:mod:".into(),
    })
    .with_id(iri("urn:example:space:mod"))
    .child(leaf(vec![
        Door::new("urn:example:mod:hello", MatchKind::Exact, "hello"),
        Door::new("urn:example:mod:item:{id}", MatchKind::Template, "item"),
        Door::new("urn:example:mod:v<n>", MatchKind::Custom, "versioned"),
        Door::new("urn:example:mod:run", MatchKind::Exact, "run").confined_to(sandbox),
        Door::new(
            "urn:example:mod:a-pattern-long-enough-to-be-cut:{first}:{second}:{third}:{fourth}",
            MatchKind::Template,
            "an-endpoint-whose-name-is-long-enough-to-be-cut-as-well",
        ),
    ]));
    let alias = Topology::new(SpaceKind::Alias {
        rules: vec![
            TopologyRule::new(RuleKind::Exact, "urn:example:home", "urn:example:mod:hello"),
            TopologyRule::new(RuleKind::Prefix, "urn:ex:", "urn:example:"),
        ],
        max_hops: 8,
    })
    .child(leaf(vec![Door::new(
        "urn:example:greeting",
        MatchKind::Exact,
        "greeting",
    )]));
    let limit = Topology::new(SpaceKind::Limit {
        family: "urn:personal:".into(),
        kind: MatchKind::Prefix,
    });
    let level = Topology::new(SpaceKind::Level {
        seals: vec!["urn:tenant:".into(), "urn:audit:".into()],
        namespace: Some("urn:iki:tenant:".into()),
    })
    .with_id(iri("urn:example:level:tenant"))
    .child(leaf(vec![Door::new(
        "urn:tenant:{key}",
        MatchKind::Template,
        "tenant-store",
    )]));
    let rewrite =
        Topology::new(SpaceKind::Rewrite).child(Topology::opaque(iri("urn:example:peer:remote")));
    let confine = Topology::new(SpaceKind::Confine).child(leaf(vec![Door::new(
        "urn:example:jail:{path}",
        MatchKind::Template,
        "jailed-file",
    )]));
    let root = Topology::new(SpaceKind::Fallback)
        .with_id(iri("urn:example:space:host"))
        .child(module.clone())
        .child(alias)
        .child(limit)
        .child(level)
        .child(rewrite)
        .child(confine)
        .child(module)
        .child(Topology::opaque(None))
        .child(leaf(Vec::new()));
    let corridor = leaf(vec![Door::new(
        "urn:example:game:state",
        MatchKind::Exact,
        "game-state",
    )])
    .with_id(iri("urn:example:corridor:game:42"));
    Topology::new(SpaceKind::Chain { severed: false })
        .with_id(iri("urn:ikigai:chain:root"))
        .child(corridor)
        .child(root)
}

/// The eight lines of a 3×3 board, as names for lines of cells.
pub const LINES: &str = "\
exact urn:ttt:row:0      urn:ttt:cells:0.0,1.0,2.0
exact urn:ttt:row:1      urn:ttt:cells:0.1,1.1,2.1
exact urn:ttt:row:2      urn:ttt:cells:0.2,1.2,2.2
exact urn:ttt:column:0   urn:ttt:cells:0.0,0.1,0.2
exact urn:ttt:column:1   urn:ttt:cells:1.0,1.1,1.2
exact urn:ttt:column:2   urn:ttt:cells:2.0,2.1,2.2
exact urn:ttt:diagonal:0 urn:ttt:cells:0.0,1.1,2.2
exact urn:ttt:diagonal:1 urn:ttt:cells:2.0,1.1,0.2
";

/// An endpoint that only has a name: the picture draws doors, it never calls them.
fn named(name: &'static str) -> FnEndpoint {
    FnEndpoint::new(name, |_| Err(Error::Endpoint("a fixture door".into())))
}

fn template(pattern: &str) -> UriTemplate {
    UriTemplate::parse(pattern).expect("a fixture template")
}

/// A tic-tac-toe-shaped kernel, built from real spaces: fourteen doors — thirteen
/// composites and the stored cell, under a fallback — wrapped in the alias table
/// that names the eight lines onto the `cells:{list}` template.
pub fn tic_tac_toe_kernel() -> Kernel {
    let composites = EndpointSpace::new()
        .bind(template("urn:ttt:cell:{x}:{y}"), named("ttt-cell"))
        .bind(template("urn:ttt:cells:{list}"), named("ttt-cells"))
        .bind(template("urn:ttt:board"), named("ttt-board"))
        .bind(template("urn:ttt:checkset:{list}"), named("ttt-checkset"))
        .bind(template("urn:ttt:winner"), named("ttt-winner"))
        .bind(template("urn:ttt:turn"), named("ttt-turn"))
        .bind(template("urn:ttt:move:{x}:{y}"), named("ttt-move"))
        .bind(template("urn:ttt:reset"), named("ttt-reset"))
        .bind(template("urn:ttt:template:{name}"), named("ttt-template"))
        .bind(template("urn:ttt:view:board"), named("ttt-view"))
        .bind(
            template("urn:ttt:view:square:{x}:{y}"),
            named("ttt-view-square"),
        )
        .bind(template("urn:ttt:view:status"), named("ttt-view-status"))
        .bind(template("urn:ttt:view:game"), named("ttt-view-game"));
    let store = EndpointSpace::new().bind(template("urn:ttt:stored:{x}:{y}"), named("ttt-stored"));
    let table = AliasTable::parse(LINES).expect("the lines parse");
    let space = Alias::new(
        Arc::new(table),
        Arc::new(Fallback::new(vec![Arc::new(composites), Arc::new(store)])),
    );
    Kernel::new(Arc::new(space))
}

/// A host arrangement holding a SELF-NAMED module space, `ikigai_diagram::space()`
/// (`urn:iki:space:diagram`), reached twice: as the first layer of a fallback, and
/// again under a mount. A name is a claim (same name, same doors), so the picture
/// draws the space once, where it is first met, and refers to it after.
pub fn named_module_kernel() -> Kernel {
    let diagram: Arc<dyn Space> = Arc::new(ikigai_diagram::space());
    let echo = EndpointSpace::new().bind(template("urn:example:echo:{text}"), named("echo"));
    let host = Fallback::new(vec![
        diagram.clone(),
        Arc::new(echo),
        Arc::new(Mount::new("urn:mirror:", diagram)),
    ]);
    Kernel::new(Arc::new(host))
}
