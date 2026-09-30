//! The module recipe as one test: `ikigai-conformance` walks every endpoint
//! `ikigai_diagram::space()` binds and reports every violation at once.
//!
//! Declarations, and why:
//!
//! - **`cacheable`, both** — a picture is a pure function of what was sourced, so
//!   both endpoints mark it `.cacheable()`; holding the suite to it turns a future
//!   dependency that silently downgraded the effective expiry into a red test.
//! - **not `pure`, either** — each one READS a resource through the kernel, and
//!   its result rightly carries that resource's threads (`urn:kernel:bindings` for
//!   the kernel's topology, the declaration's own thread for `of`). Declaring them
//!   pure would be the lie the suite exists to catch.
//! - **a fixture for `of`** — the suite's minimal `xsd:anyURI` names nothing the
//!   kernel binds, so the invoking checks would see only an unresolved sub-request.
//!   The fixture names a real arrangement served by a fixture endpoint.
//!
//! The fixture endpoint is walked too (the suite cannot tell it from the module's
//! own), so it is described like one: a kebab-case id, its verbs and its face.

mod common;

use std::sync::Arc;

use common::committed;
use ikigai_conformance::{Fixture, Suite};
use ikigai_core::{
    Description, EndpointSpace, Exact, Fallback, FnEndpoint, Kernel, ReprType, Representation,
    Space, Verb,
};

/// Every endpoint `space()` binds, by description id.
const ENDPOINTS: [&str; 2] = ["diagram-arrangement", "diagram-kernel"];
/// The arrangement the fixture serves, and its description id.
const FIXTURE_IRI: &str = "urn:fixture:tic-tac-toe";
const FIXTURE_ID: &str = "fixture-arrangement";

fn fixture_space() -> EndpointSpace {
    let turtle = committed("fixtures/tic-tac-toe.ttl").expect("the fixture");
    EndpointSpace::new().bind(
        Exact::new(FIXTURE_IRI),
        FnEndpoint::new(FIXTURE_ID, move |_| {
            Ok(
                Representation::new(ReprType::new("text/turtle"), turtle.clone().into_bytes())
                    .cacheable(),
            )
        })
        .with_description(
            Description::new(FIXTURE_ID)
                .summary("a tic-tac-toe arrangement, for the diagram to draw")
                .verb(Verb::Source)
                .verb(Verb::Meta)
                .output("text/turtle"),
        ),
    )
}

#[test]
fn conforms() {
    let kernel = Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_diagram::space()) as Arc<dyn Space>,
        Arc::new(fixture_space()),
    ])));

    let suite = Suite::new()
        .cacheable("diagram-arrangement")
        .cacheable("diagram-kernel")
        .fixture(Fixture::new("diagram-arrangement", Verb::Source).arg("of", FIXTURE_IRI));

    let report = suite.run_blocking(&kernel);
    println!("{report}"); // shown with --nocapture: what the walk probed and did not
    assert!(report.is_clean(), "{report}");

    // The walk saw exactly the module's endpoints and the fixture. A third binding
    // without a declaration would be held to a weaker standard; a declared id that
    // binds nothing is a stale list. Both change this count.
    assert_eq!(
        report.endpoints,
        ENDPOINTS.len() + 1,
        "every binding is declared: {report}"
    );
}
