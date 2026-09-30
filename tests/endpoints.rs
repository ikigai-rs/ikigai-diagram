//! The two endpoints through a real kernel: what they source, under whose
//! authority, what they hang from, and what a resource that is not an arrangement
//! draws.

mod common;

use std::sync::{Arc, Mutex};

use common::{committed, tic_tac_toe_kernel};
use futures::executor::block_on;
use ikigai_core::{
    ArgRef, Capability, Door, EndpointSpace, Error, Exact, Fallback, FnEndpoint, Iri, Kernel,
    MatchKind, ReprType, Representation, Request, Scope, Space, SpaceKind, Topology,
};
use ikigai_diagram::{render, ARRANGEMENT, CAP_INSPECT, KERNEL, SVG};

fn iri(s: &str) -> Iri {
    Iri::parse(s).expect("a test IRI")
}

/// An endpoint serving fixed Turtle, cacheable like a file.
fn serving(name: &'static str, body: String) -> FnEndpoint {
    FnEndpoint::new(name, move |_| {
        Ok(
            Representation::new(ReprType::new("text/turtle"), body.clone().into_bytes())
                .cacheable(),
        )
    })
}

/// A kernel over the diagram module and a few arrangements to draw.
fn kernel_with(extra: EndpointSpace) -> Kernel {
    let fixtures = extra
        .bind(
            Exact::new("urn:fixture:tic-tac-toe"),
            serving(
                "fixture-tic-tac-toe",
                committed("fixtures/tic-tac-toe.ttl").expect("the fixture"),
            ),
        )
        .bind(
            Exact::new("urn:fixture:not-an-arrangement"),
            serving(
                "fixture-not-an-arrangement",
                "@prefix ex: <urn:ex:> .\nex:a a ex:Thing .\n".to_string(),
            ),
        );
    Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_diagram::space()) as Arc<dyn Space>,
        Arc::new(fixtures),
    ])))
}

fn draw(kernel: &Kernel, of: &str) -> ikigai_core::Result<Representation> {
    let request = Request::new(ikigai_core::Verb::Source, iri(ARRANGEMENT))
        .with_arg("of", ArgRef::Inline(of.as_bytes().to_vec()));
    block_on(kernel.issue(request, &Capability::root()))
}

fn text(repr: &Representation) -> String {
    String::from_utf8(repr.bytes.clone()).expect("SVG is UTF-8")
}

#[test]
fn an_arrangement_is_drawn_as_the_golden_picture() {
    let kernel = kernel_with(EndpointSpace::new());
    let picture = draw(&kernel, "urn:fixture:tic-tac-toe").expect("drawn");
    assert_eq!(picture.repr_type.media_type, SVG);
    // Not assert_eq!: a mismatch would print two whole SVG documents.
    assert!(
        text(&picture) == committed("golden/tic-tac-toe.svg").expect("the golden picture"),
        "the endpoint's picture differs from tests/golden/tic-tac-toe.svg"
    );
}

#[test]
fn a_picture_hangs_from_what_it_drew_and_is_redrawn_when_that_changes() {
    // A declaration that changes: the endpoint serves whatever the cell holds, and
    // hangs from its own name, as a watched file would.
    let cell = Arc::new(Mutex::new(Topology::new(SpaceKind::EndpointSpace {
        doors: vec![Door::new("urn:first", MatchKind::Exact, "first")],
    })));
    let served = Arc::clone(&cell);
    let space = EndpointSpace::new().bind(
        Exact::new("urn:fixture:mutable"),
        FnEndpoint::new("fixture-mutable", move |_| {
            let turtle = served.lock().expect("lock").to_turtle();
            Ok(Representation::new(ReprType::new("text/turtle"), turtle.into_bytes()).cacheable())
        }),
    );
    let kernel = kernel_with(space);
    let first = draw(&kernel, "urn:fixture:mutable").expect("drawn");
    assert!(
        text(&first).contains(">urn:first</tspan>"),
        "{}",
        text(&first)
    );
    assert!(
        first
            .threads()
            .iter()
            .any(|t| t.as_str() == "urn:fixture:mutable"),
        "the picture hangs from the declaration's thread: {:?}",
        first.threads()
    );

    *cell.lock().expect("lock") = Topology::new(SpaceKind::EndpointSpace {
        doors: vec![Door::new("urn:second", MatchKind::Exact, "second")],
    });
    // Cached: nothing has cut the declaration's thread yet.
    assert_eq!(
        draw(&kernel, "urn:fixture:mutable").expect("drawn").bytes,
        first.bytes
    );
    kernel.cut("urn:fixture:mutable");
    let second = draw(&kernel, "urn:fixture:mutable").expect("redrawn");
    assert!(
        text(&second).contains(">urn:second</tspan>"),
        "{}",
        text(&second)
    );
}

#[test]
fn a_resource_that_is_not_an_arrangement_is_a_picture_saying_why() {
    let kernel = kernel_with(EndpointSpace::new());
    let picture =
        draw(&kernel, "urn:fixture:not-an-arrangement").expect("an answer, not a refusal");
    let svg = text(&picture);
    assert!(
        svg.contains("Not an arrangement: urn:fixture:not-an-arrangement"),
        "{svg}"
    );
    assert!(svg.contains("role=\"img\""), "{svg}");
    // An answer about the resource, so it is cacheable and hangs from the resource.
    assert!(
        picture
            .threads()
            .iter()
            .any(|t| t.as_str() == "urn:fixture:not-an-arrangement"),
        "{:?}",
        picture.threads()
    );
}

#[test]
fn failing_to_read_of_is_an_error_not_a_picture() {
    let kernel = kernel_with(EndpointSpace::new());
    let missing = draw(&kernel, "urn:fixture:nothing-here").expect_err("nothing to draw");
    assert!(
        matches!(missing, Error::Unresolved(_) | Error::NotFound(_)),
        "{missing:?}"
    );
    let not_an_iri = draw(&kernel, "@prefix ik: <https://ikigai-rs.dev/ns#> .\n")
        .expect_err("Turtle where the IRI belongs");
    let message = not_an_iri.to_string();
    assert!(
        message.contains("pass its IRI, not its Turtle"),
        "{message}"
    );
}

#[test]
fn of_is_read_under_the_callers_capability() {
    // The fixture requires a scope the caller does not hold: the diagram adds no
    // authority of its own, so the sub-request is refused, and so is the picture.
    let guarded = EndpointSpace::new().bind(
        Exact::new("urn:fixture:guarded"),
        serving(
            "fixture-guarded",
            committed("fixtures/tic-tac-toe.ttl").expect("the fixture"),
        )
        .with_description(
            ikigai_core::Description::new("fixture-guarded")
                .verb(ikigai_core::Verb::Source)
                .requires("urn:cap:fixture:read"),
        ),
    );
    let kernel = kernel_with(guarded);
    let request = Request::new(ikigai_core::Verb::Source, iri(ARRANGEMENT))
        .with_arg("of", ArgRef::Inline(b"urn:fixture:guarded".to_vec()));
    let refused =
        block_on(kernel.issue(request.clone(), &Capability::scoped(Vec::<String>::new())))
            .expect_err("the caller cannot read the fixture");
    assert!(matches!(refused, Error::Denied(_)), "{refused:?}");
    let allowed = block_on(kernel.issue(request, &Capability::scoped(["urn:cap:fixture:read"])));
    assert!(allowed.is_ok(), "{allowed:?}");
}

#[test]
fn the_kernel_picture_is_the_callers_own_topology() {
    let kernel = kernel_with(EndpointSpace::new());
    let request = Request::new(ikigai_core::Verb::Source, iri(KERNEL));
    let picture = block_on(kernel.issue(request.clone(), &Capability::scoped([CAP_INSPECT])))
        .expect("drawn under inspect");
    assert!(
        text(&picture) == render(&kernel.topology()),
        "the endpoint draws exactly the kernel's topology"
    );
    assert!(
        text(&picture).contains(">diagram-kernel</tspan>"),
        "it draws itself"
    );
    assert!(
        picture
            .threads()
            .iter()
            .any(|t| t.as_str() == "urn:kernel:bindings"),
        "a rebind redraws it: {:?}",
        picture.threads()
    );

    let refused = block_on(kernel.issue(request, &Capability::scoped(Vec::<String>::new())))
        .expect_err("topology is inspect-gated");
    assert!(
        matches!(refused, Error::Denied(ref m) if m.contains(CAP_INSPECT)),
        "{refused:?}"
    );
}

#[test]
fn the_kernel_picture_is_per_request_and_shows_a_corridor() {
    let kernel = kernel_with(EndpointSpace::new());
    let corridor = EndpointSpace::new()
        .bind(
            Exact::new("urn:game:42:state"),
            serving("game-state", String::new()),
        )
        .named(iri("urn:example:corridor:game:42"));
    let scope = Scope::empty().with(Arc::new(corridor));
    let request = Request::new(ikigai_core::Verb::Source, iri(KERNEL));
    let inside = block_on(kernel.issue_in(request.clone(), &Capability::root(), scope))
        .expect("drawn in the corridor");
    let outside = block_on(kernel.issue(request, &Capability::root())).expect("drawn");
    let inside = text(&inside);
    assert!(inside.contains("corridor 1 · "), "{inside}");
    assert!(
        inside.contains(">urn:example:corridor:game:42</tspan>"),
        "{inside}"
    );
    assert!(!text(&outside).contains("urn:example:corridor:game:42"));
}

#[test]
fn the_same_tree_is_the_same_bytes_through_the_kernel() {
    // Two kernels, built separately: the picture is a function of the arrangement,
    // not of the process that drew it.
    let a = tic_tac_toe_kernel().topology();
    let b = tic_tac_toe_kernel().topology();
    assert!(render(&a) == render(&b), "two kernels, one picture");
}
