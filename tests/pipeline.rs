//! Pipeline citizenship, through the **engine** — the only place it is visible.
//!
//! `of` is `urn:diagram:arrangement`'s one required input, so the engine routes a
//! positional value — typed after the name, or piped in — into it. Whether that
//! holds is decided by the engine's routing rule (a `Source` fills the ONE declared
//! argument left unnamed), which no kernel-level test can see. The kernel here has
//! a JSON meta renderer: without one the engine fails open, routes every value to
//! an input named `in`, and these tests would pass over a misnamed argument.
//!
//! What a pipe carries is the thing to note: `source urn:kernel:topology |
//! urn:diagram:arrangement` pipes the topology's TURTLE, not its name, and `of`
//! takes a name. The last test pins that the refusal says so rather than failing
//! on an opaque IRI parse. (A second, inline-Turtle input would make `of`
//! optional, and a `Source` with no required input cannot be piped into at all —
//! so the union would cost the positional form this file exists to keep.)

mod common;

use std::sync::Arc;

use common::committed;
use futures::executor::block_on;
use ikigai_core::{
    Description, EndpointSpace, Exact, Fallback, FnEndpoint, Kernel, MetaRenderer, ReprType,
    Representation, Space,
};
use ikigai_engine::{Action, Engine};

/// The contract renderer the engine actually asks for.
struct JsonRenderer;

impl MetaRenderer for JsonRenderer {
    fn render(
        &self,
        description: &Description,
        _target: &ReprType,
    ) -> ikigai_core::Result<Representation> {
        Ok(Representation::new(
            ReprType::new("application/json"),
            serde_json::to_vec(description).expect("serialize description"),
        ))
    }
}

fn engine() -> Engine {
    let turtle = committed("fixtures/tic-tac-toe.ttl").expect("the fixture");
    let fixtures = EndpointSpace::new().bind(
        Exact::new("urn:fixture:tic-tac-toe"),
        FnEndpoint::new("fixture-tic-tac-toe", move |_| {
            Ok(Representation::new(
                ReprType::new("text/turtle"),
                turtle.clone().into_bytes(),
            ))
        }),
    );
    let fixtures = fixtures.bind(
        Exact::new("urn:fixture:name"),
        FnEndpoint::new("fixture-name", |_| {
            Ok(Representation::new(
                ReprType::new("text/plain"),
                b"urn:fixture:tic-tac-toe".to_vec(),
            ))
        }),
    );
    Engine::new(Kernel::with_meta_renderer(
        Arc::new(Fallback::new(vec![
            Arc::new(ikigai_diagram::space()) as Arc<dyn Space>,
            Arc::new(fixtures),
        ])),
        Arc::new(JsonRenderer),
    ))
}

fn run(line: &str) -> Result<String, String> {
    match block_on(engine().eval_async(line)) {
        Action::Output(entry) => entry.result,
        _ => Err(format!("`{line}` produced no output")),
    }
}

fn golden() -> String {
    committed("golden/tic-tac-toe.svg").expect("the golden picture")
}

#[test]
fn a_positional_value_lands_in_of() {
    // Also the witness that the contract is really read: a fail-open engine would
    // route the value to `in`, and the endpoint would refuse for a missing `of`.
    let out = run("source urn:diagram:arrangement urn:fixture:tic-tac-toe")
        .expect("the positional form draws");
    assert_eq!(out.trim_end(), golden().trim_end());
}

#[test]
fn the_named_form_draws_the_same_picture() {
    let out = run("source urn:diagram:arrangement of=urn:fixture:tic-tac-toe").expect("drawn");
    assert_eq!(out.trim_end(), golden().trim_end());
}

#[test]
fn a_piped_name_lands_in_of() {
    // A resource whose representation is a NAME pipes into `of`.
    let out = run("source urn:fixture:name | urn:diagram:arrangement").expect("drawn");
    assert_eq!(out.trim_end(), golden().trim_end());
}

#[test]
fn piping_turtle_where_the_name_belongs_says_so() {
    let err = run("source urn:fixture:tic-tac-toe | urn:diagram:arrangement")
        .expect_err("Turtle is not an IRI");
    assert!(err.contains("pass its IRI, not its Turtle"), "{err}");
}
