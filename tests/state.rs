//! The state overlay (ledger #2173): governor state drawn over the arrangement.
//!
//! This crate does not depend on ikigai-throttle (the state face, its PR 27, is merged but
//! not released). The boards here are Turtle:
//!
//! - `fixtures/state/board.ttl` is ikigai-throttle's pinned contract **verbatim**, copied
//!   from `tests/state.rs::the_board_in_turtle` at `c06ea47`;
//! - `fixtures/state/every-state.ttl` is the same shape, by hand, holding every state value
//!   the face writes (full, half-open, closed, failed) and work parked on a bulkhead;
//! - the rest are built here: a hostile prefix, an unlisted governor, a limited caller's
//!   board, an empty one.
//!
//! The endpoint tests serve a board from a stub at `urn:throttle:state` that declares the
//! face's family (`urn:cap:throttle:state:*`), so the kernel's floor refuses exactly as the
//! real one does. A live end-to-end test with the real `StateBoard` waits on a throttle
//! release.

mod common;

use std::sync::{Arc, Mutex};

use common::{committed, governed};
use futures::executor::block_on;
use ikigai_core::{
    encode_name, escape_iri_fragment, Capability, Description, EndpointSpace, Error, Exact,
    Fallback, FnEndpoint, Iri, Kernel, ReprType, Representation, Request, Space, Topology, Verb,
};
use ikigai_diagram::state::{Board, GovernorKind, KeyState, Mode, STATE_CAP_FAMILY, THR};
use ikigai_diagram::{render, render_with_state, CAP_INSPECT, KERNEL, STATE};

fn board(relative: &str) -> Board {
    Board::from_turtle(&committed(relative).expect("the board fixture")).expect("it parses")
}

fn pinned() -> Board {
    board("fixtures/state/board.ttl")
}

/// Compare `actual` with the committed `tests/{relative}`, writing the actual out when
/// they differ (the same rule as `tests/golden.rs`).
fn matches(relative: &str, actual: &str) {
    if committed(relative).as_deref() == Some(actual) {
        return;
    }
    let out = format!("{}/golden-actual/{relative}", env!("CARGO_TARGET_TMPDIR"));
    let dir = std::path::Path::new(&out).parent().expect("a parent");
    std::fs::create_dir_all(dir).expect("create the actual-output dir");
    std::fs::write(&out, actual).expect("write the actual output");
    panic!(
        "tests/{relative} differs from what the code produces (or is missing).\n\
         If the change is intended:  cp {out} tests/{relative}"
    );
}

// ---- the contract, read ------------------------------------------------------------------

#[test]
fn the_pinned_board_parses_into_what_it_says() {
    let board = pinned();
    let names: Vec<&str> = board.governors().iter().map(|g| g.name()).collect();
    assert_eq!(names, ["breaker", "dr", "limit", "pool"]);
    let by = |name: &str| board.governors().iter().find(|g| g.name() == name).unwrap();

    let breaker = by("breaker");
    assert_eq!(breaker.iri(), "urn:throttle:state:breaker");
    assert_eq!(breaker.governs(), Some("urn:iki:space:demo-breaker"));
    assert_eq!(
        breaker.kind(),
        &GovernorKind::CircuitBreaker {
            threshold: Some(2),
            cooldown_millis: Some(30_000)
        }
    );
    assert_eq!(
        breaker.keys(),
        [KeyState::Circuit {
            target: "urn:demo:breaker:down".into(),
            failures: 2,
            mode: Mode::Open
        }]
    );

    let dr = by("dr");
    assert_eq!(dr.governs(), None, "a failover is opaque in topology");
    let GovernorKind::Failover { candidates } = dr.kind() else {
        panic!("{:?}", dr.kind());
    };
    let spaces: Vec<_> = candidates
        .iter()
        .map(|c| (c.order, c.governs.as_deref()))
        .collect();
    assert_eq!(
        spaces,
        [
            (0, Some("urn:iki:space:demo-primary")),
            (1, Some("urn:iki:space:demo-backup"))
        ]
    );
    assert_eq!(
        dr.keys(),
        [KeyState::Route {
            target: "urn:demo:dr:x".into(),
            last_candidate: Some(1),
            mode: Mode::Answered
        }]
    );

    assert_eq!(
        by("limit").keys(),
        [KeyState::Window {
            prefix: "urn:demo:limit:".into(),
            max: 3,
            window_millis: 10_000,
            hits: 2,
            mode: Mode::Admitting
        }]
    );
    assert_eq!(
        by("pool").keys(),
        [KeyState::Bulkhead {
            prefix: "urn:demo:pool:".into(),
            max: 2,
            running: 0,
            parked: 0,
            mode: Mode::Admitting
        }]
    );
}

#[test]
fn the_namespace_is_one_constant_and_matches_the_contract() {
    // ledger #2175 may fold thr: into ik:; this is the line that moves with it.
    assert_eq!(THR, "https://ikigai-rs.dev/ns/throttle#");
    let fixture = committed("fixtures/state/board.ttl").unwrap();
    assert!(fixture.contains(&format!("@prefix thr: <{THR}> .")));
}

#[test]
fn every_state_value_has_a_word() {
    let board = board("fixtures/state/every-state.ttl");
    let mut words: Vec<String> = Vec::new();
    for g in board.governors() {
        for k in g.keys() {
            let mode = match k {
                KeyState::Window { mode, .. }
                | KeyState::Circuit { mode, .. }
                | KeyState::Bulkhead { mode, .. }
                | KeyState::Route { mode, .. } => mode,
                other => panic!("{other:?}"),
            };
            words.push(mode.word().to_string());
        }
    }
    words.sort();
    assert_eq!(words, ["closed", "failed", "full", "full", "half-open"]);
    // Together with the pinned board: every term the face writes.
    let svg = render_with_state(&governed(), &board);
    for word in ["HALF-OPEN", "CLOSED", "FULL", "FAILED"] {
        assert!(svg.contains(&format!(">{word}</tspan>")), "{word}");
    }
    let svg = render_with_state(&governed(), &pinned());
    for word in ["OPEN", "ADMITTING", "ANSWERED"] {
        assert!(svg.contains(&format!(">{word}</tspan>")), "{word}");
    }
}

#[test]
fn a_governor_the_board_does_not_list_is_never_drawn() {
    // A document that DESCRIBES a governor without listing it: the board's list is the
    // whole of what the caller may see, so the picture draws only what it lists.
    let mut turtle = committed("fixtures/state/board.ttl").unwrap();
    turtle = turtle.replace("    thr:governor <urn:throttle:state:breaker> ;\n", "");
    let board = Board::from_turtle(&turtle).expect("parses");
    assert!(board.governors().iter().all(|g| g.name() != "breaker"));
    let svg = render_with_state(&governed(), &board);
    assert!(!svg.contains("circuit breaker"), "no breaker band: {svg}");
    // The arrangement still draws the breaker's DOOR `urn:demo:breaker:down`; the layer
    // draws no circuit row for it, and the words never name it.
    assert!(!svg.contains(" circuit </tspan>"), "no circuit row: {svg}");
    assert!(!svg.contains("OPEN</tspan>"), "{svg}");
    assert!(!svg.contains("Circuit urn:demo:breaker:down"), "{svg}");
}

#[test]
fn a_board_that_is_not_one_is_refused() {
    for (turtle, why) in [
        (
            "@prefix ex: <urn:ex:> . ex:a a ex:Thing .",
            "no <urn:throttle:state>",
        ),
        ("not turtle at all <", "not Turtle"),
        (
            "<urn:throttle:state> a <https://ikigai-rs.dev/ns/throttle#StateBoard> ; \
             <https://ikigai-rs.dev/ns/throttle#governor> [] .",
            "blank node",
        ),
        (
            "<urn:throttle:state> a <https://ikigai-rs.dev/ns/throttle#StateBoard> ; \
             <https://ikigai-rs.dev/ns/throttle#governor> <urn:throttle:state:ghost> .",
            "does not describe it",
        ),
    ] {
        let error = Board::from_turtle(turtle).expect_err(turtle);
        assert!(error.to_string().contains(why), "{turtle}: {error}");
    }
}

// ---- hostile text ------------------------------------------------------------------------

/// A board for one rate limit over `prefix`, spelled the way the face spells it: the key's
/// IRI is `escape_iri_fragment(encode_name(prefix))`, the prefix literal
/// `escape_iri_fragment(prefix)` (ikigai-throttle `src/state.rs` at `c06ea47`).
fn one_window(prefix: &str) -> String {
    let key = format!(
        "urn:throttle:state:limit:key:{}",
        escape_iri_fragment(&encode_name(prefix))
    );
    [
        "@prefix thr: <https://ikigai-rs.dev/ns/throttle#> .".to_string(),
        "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .".to_string(),
        "<urn:throttle:state> a thr:StateBoard ; thr:governor <urn:throttle:state:limit> ."
            .to_string(),
        format!(
            "<urn:throttle:state:limit> a thr:RateLimit ; thr:name \"limit\" ; \
             thr:governs <urn:iki:space:demo-limit> ; thr:key <{key}> ."
        ),
        format!(
            "<{key}> a thr:Window ; thr:of <urn:throttle:state:limit> ; thr:prefix \"{}\" ; \
             thr:max \"1\"^^xsd:nonNegativeInteger ; thr:windowMillis \"1000\"^^xsd:nonNegativeInteger ; \
             thr:hits \"0\"^^xsd:nonNegativeInteger ; thr:state thr:Admitting .",
            escape_iri_fragment(prefix)
        ),
    ]
    .join("\n")
}

#[test]
fn a_hostile_prefix_is_text_never_markup() {
    // The prefix ikigai-throttle's own conformance test uses, on purpose.
    let hostile = "urn:x:\"evil> ; a <urn:y> .";
    let board = Board::from_turtle(&one_window(hostile)).expect("the face still parses");
    let KeyState::Window { prefix, .. } = &board.governors()[0].keys()[0] else {
        panic!()
    };
    assert_eq!(prefix, &escape_iri_fragment(hostile));
    let svg = render_with_state(&governed(), &board);
    assert!(svg.contains(&escape_iri_fragment(hostile).to_string()));
    assert!(!svg.contains("\"evil>"), "{svg}");
    assert!(!svg.contains("<urn:y>"), "{svg}");

    // Markup in a literal (the face escapes it away, but the picture does not rely on that).
    let scripted = one_window("urn:x:").replace(
        "thr:prefix \"urn:x:\"",
        "thr:prefix \"<script>alert(1)</script>\"",
    );
    let board = Board::from_turtle(&scripted).expect("parses");
    let svg = render_with_state(&governed(), &board);
    assert!(
        svg.contains("&lt;script&gt;alert(1)&lt;/script&gt;"),
        "{svg}"
    );
    assert!(!svg.contains("<script>"), "{svg}");
}

// ---- the picture -------------------------------------------------------------------------

#[test]
fn the_governed_fixture_is_what_core_writes_for_it() {
    let tree = governed();
    matches("fixtures/state/governed.ttl", &tree.to_turtle());
    let text = committed("fixtures/state/governed.ttl").expect("the fixture");
    assert_eq!(Topology::from_turtle(&text).expect("it parses"), tree);
}

#[test]
fn the_state_pictures_are_stable() {
    let tree = governed();
    for (board_file, golden) in [
        ("fixtures/state/board.ttl", "golden/state-board.svg"),
        (
            "fixtures/state/every-state.ttl",
            "golden/state-every-state.svg",
        ),
    ] {
        let board = board(board_file);
        let svg = render_with_state(&tree, &board);
        assert_eq!(svg, render_with_state(&tree, &board), "deterministic");
        matches(golden, &svg);
    }
}

#[test]
fn state_is_drawn_in_the_box_of_the_space_it_governs() {
    let svg = render_with_state(&governed(), &pinned());
    // Each governed space's name comes before its governor's band and its key row, and
    // before the next space's name.
    let at = |needle: &str| {
        svg.find(needle)
            .unwrap_or_else(|| panic!("{needle} not drawn"))
    };
    let order = [
        ">urn:iki:space:demo-limit</tspan>",
        ">rate limit</tspan>",
        " window </tspan><tspan class=\"ikd-x\">urn:demo:limit:</tspan>",
        ">urn:iki:space:demo-breaker</tspan>",
        ">circuit breaker</tspan>",
        ">OPEN</tspan>",
        ">urn:iki:space:demo-pool</tspan>",
        ">throttle</tspan>",
        ">urn:iki:space:demo-backup</tspan>",
        " of failover </tspan><tspan class=\"ikd-x\">dr</tspan>",
        ">ANSWERED</tspan>",
    ];
    for pair in order.windows(2) {
        assert!(at(pair[0]) < at(pair[1]), "{} before {}", pair[0], pair[1]);
    }
}

#[test]
fn the_words_say_everything_the_layer_shows_in_a_live_region() {
    let svg = render_with_state(&governed(), &pinned());
    let (picture, words) = svg
        .split_once("role=\"status\" aria-live=\"polite\" aria-atomic=\"true\">")
        .expect("a live region");
    assert!(picture.contains("role=\"group\""), "the root is a group");
    assert!(
        picture.contains("<g role=\"img\""),
        "the arrangement is an image"
    );
    for sentence in [
        "Governor state",
        "breaker, a circuit breaker, governs urn:iki:space:demo-breaker (drawn above); trips after 2, cooldown 30s.",
        "Circuit urn:demo:breaker:down: open, 2 consecutive transient failures.",
        "dr, a failover over 2 candidates: candidate 0 governs urn:iki:space:demo-primary (not in this arrangement); candidate 1 governs urn:iki:space:demo-backup (drawn above).",
        "Route urn:demo:dr:x: answered, last call ended at candidate 1.",
        "limit, a rate limit, governs urn:iki:space:demo-limit (drawn above).",
        "Window urn:demo:limit:: admitting, 2 of 3 per 10s.",
        "pool, a throttle, governs urn:iki:space:demo-pool (drawn above).",
        "Bulkhead urn:demo:pool:: admitting, 0 of 2 running, 0 parked.",
    ] {
        // Sentences may wrap across lines; join the region's text to read them.
        let text = text_of(words);
        assert!(text.contains(sentence), "missing: {sentence}\nin: {text}");
    }
    // Every state word drawn on a box is said in the words, too.
    for word in ["open", "answered", "admitting"] {
        assert!(text_of(words).contains(&format!(": {word},")), "{word}");
    }
}

/// The text of every `<tspan>` in `svg`, one line per `<text>`, joined by spaces, with
/// the five escapes undone.
fn text_of(svg: &str) -> String {
    let mut lines = Vec::new();
    for text in svg.split("<text ").skip(1) {
        let mut line = String::new();
        for span in text.split("<tspan").skip(1) {
            let body = span.split_once('>').map(|(_, b)| b).unwrap_or("");
            line.push_str(body.split("</tspan>").next().unwrap_or(""));
        }
        lines.push(line);
    }
    lines
        .join(" ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

#[test]
fn an_empty_board_draws_the_plain_picture_byte_for_byte() {
    let tree = governed();
    assert_eq!(render_with_state(&tree, &Board::empty()), render(&tree));
    let empty = Board::from_turtle(
        "<urn:throttle:state> a <https://ikigai-rs.dev/ns/throttle#StateBoard> .",
    )
    .expect("an empty board parses");
    assert!(empty.is_empty());
    assert_eq!(render_with_state(&tree, &empty), render(&tree));
}

#[test]
fn the_state_layer_does_not_change_the_plain_pictures_stylesheet() {
    let plain = render(&governed());
    assert!(
        !plain.contains("ikd-st-"),
        "no state rules in a plain picture"
    );
    let with = render_with_state(&governed(), &pinned());
    assert!(with.contains(".ikd-st-stop{fill:"));
}

// ---- the endpoint, through a kernel ------------------------------------------------------

/// What the stub board serves, and whether its answer is cacheable (attached) or live.
struct Stub {
    turtle: Mutex<String>,
    attached: bool,
}

/// A kernel over the diagram module, three governed spaces and a stub board.
fn host(stub: Option<Arc<Stub>>) -> Kernel {
    let leaf = |name: &str| {
        EndpointSpace::new()
            .bind(
                Exact::new(format!("urn:demo:{name}:ok")),
                FnEndpoint::new("ok", |_| {
                    Ok(Representation::new(ReprType::new("text/plain"), "ok"))
                }),
            )
            .named(Iri::parse(format!("urn:iki:space:demo-{name}")).unwrap())
    };
    let mut spaces: Vec<Arc<dyn Space>> = vec![
        Arc::new(ikigai_diagram::space()),
        Arc::new(leaf("limit")),
        Arc::new(leaf("breaker")),
        Arc::new(leaf("pool")),
    ];
    if let Some(stub) = stub {
        let board = EndpointSpace::new().bind(
            Exact::new("urn:throttle:state"),
            FnEndpoint::new("throttle-state", move |inv| {
                let turtle = stub.turtle.lock().unwrap().clone();
                let repr = Representation::new(ReprType::new("text/turtle"), turtle);
                Ok(if stub.attached {
                    // What `StateBoard::attach` does: cacheable, hanging from each
                    // governor's thread.
                    for g in ["breaker", "dr", "limit", "pool"] {
                        inv.depends_on(format!("urn:throttle:governor:{g}"));
                    }
                    repr.cacheable()
                } else {
                    repr
                })
            })
            .with_description(
                Description::new("throttle-state")
                    .verb(Verb::Source)
                    .requires(STATE_CAP_FAMILY),
            ),
        );
        spaces.push(Arc::new(board));
    }
    Kernel::new(Arc::new(Fallback::new(spaces)))
}

fn stub(attached: bool) -> Arc<Stub> {
    Arc::new(Stub {
        turtle: Mutex::new(committed("fixtures/state/board.ttl").unwrap()),
        attached,
    })
}

fn read(kernel: &Kernel, iri: &str, cap: &Capability) -> ikigai_core::Result<String> {
    block_on(kernel.issue(Request::new(Verb::Source, Iri::parse(iri).unwrap()), cap))
        .map(|r| String::from_utf8(r.bytes).unwrap())
}

fn cached(kernel: &Kernel, iri: &str, cap: &Capability) -> bool {
    kernel.is_cached(&Request::new(Verb::Source, Iri::parse(iri).unwrap()), cap)
}

#[test]
fn a_caller_with_the_grant_sees_the_state_layer() {
    let kernel = host(Some(stub(true)));
    let cap = Capability::scoped([CAP_INSPECT, "urn:cap:throttle:state:limit"]);
    let svg = read(&kernel, STATE, &cap).expect("drawn");
    assert!(svg.contains(">OPEN</tspan>"), "{svg}");
    assert!(svg.contains("role=\"status\""), "{svg}");
}

#[test]
fn a_caller_without_the_grant_gets_the_plain_picture_and_no_hint() {
    let kernel = host(Some(stub(true)));
    let cap = Capability::scoped([CAP_INSPECT]);
    let state = read(&kernel, STATE, &cap).expect("drawn");
    let plain = read(&kernel, KERNEL, &cap).expect("drawn");
    assert!(state == plain, "byte for byte urn:diagram:kernel's picture");
    for hint in ["governor", "Governor", "ikd-st-", "role=\"status\"", "OPEN"] {
        assert!(!state.contains(hint), "{hint}");
    }
    // The board was never asked, so nothing refused, and the picture is cacheable.
    assert!(cached(&kernel, STATE, &cap), "no refusal to keep it live");
}

#[test]
fn no_board_mounted_draws_the_plain_picture() {
    let kernel = host(None);
    let svg = read(&kernel, STATE, &Capability::root()).expect("drawn");
    assert!(svg == read(&kernel, KERNEL, &Capability::root()).unwrap());
}

#[test]
fn a_board_denied_anyway_draws_the_plain_picture() {
    // A grant the floor admits (an exclusion under the family) that the board's own
    // rule then refuses.
    let denying = EndpointSpace::new().bind(
        Exact::new("urn:throttle:state"),
        FnEndpoint::new("throttle-state", |_| Err(Error::Denied("no".into()))).with_description(
            Description::new("throttle-state")
                .verb(Verb::Source)
                .requires(STATE_CAP_FAMILY),
        ),
    );
    let kernel = Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_diagram::space()) as Arc<dyn Space>,
        Arc::new(denying),
    ])));
    let svg = read(&kernel, STATE, &Capability::root()).expect("drawn");
    assert!(svg == read(&kernel, KERNEL, &Capability::root()).unwrap());
}

#[test]
fn a_board_that_fails_or_is_not_a_board_is_an_error() {
    let failing = EndpointSpace::new().bind(
        Exact::new("urn:throttle:state"),
        FnEndpoint::new("throttle-state", |_| Err(Error::Unavailable("down".into()))),
    );
    let kernel = Kernel::new(Arc::new(Fallback::new(vec![
        Arc::new(ikigai_diagram::space()) as Arc<dyn Space>,
        Arc::new(failing),
    ])));
    let error = read(&kernel, STATE, &Capability::root()).expect_err("not an answer");
    assert!(matches!(error, Error::Unavailable(_)), "{error:?}");

    let s = stub(true);
    *s.turtle.lock().unwrap() = "@prefix ex: <urn:ex:> . ex:a a ex:Thing .".into();
    let kernel = host(Some(s));
    let error = read(&kernel, STATE, &Capability::root()).expect_err("not a board");
    assert!(
        error.to_string().contains("did not parse as a state board"),
        "{error}"
    );
}

#[test]
fn attached_the_overlay_is_cached_and_a_governor_cut_redraws_it() {
    let s = stub(true);
    let kernel = host(Some(Arc::clone(&s)));
    let root = Capability::root();
    let first = read(&kernel, STATE, &root).expect("drawn");
    assert!(first.contains(">OPEN</tspan>"));
    assert!(
        cached(&kernel, STATE, &root),
        "the board is cacheable, so is the picture"
    );

    // The breaker half-opens; the governor cuts its thread.
    let half_open = s
        .turtle
        .lock()
        .unwrap()
        .replace("thr:state thr:Open", "thr:state thr:HalfOpen");
    *s.turtle.lock().unwrap() = half_open;
    assert!(cached(&kernel, STATE, &root), "nothing cut yet");
    kernel.cut("urn:throttle:governor:breaker");
    assert!(
        !cached(&kernel, STATE, &root),
        "the cut reached the picture"
    );
    let second = read(&kernel, STATE, &root).expect("redrawn");
    assert!(second.contains(">HALF-OPEN</tspan>"), "{second}");

    // urn:diagram:kernel never sourced the board, so it is untouched by the cut.
    read(&kernel, KERNEL, &root).unwrap();
    kernel.cut("urn:throttle:governor:limit");
    assert!(
        cached(&kernel, KERNEL, &root),
        "the plain picture does not hang from state"
    );
}

#[test]
fn unattached_the_overlay_is_live_and_the_plain_picture_stays_cached() {
    let kernel = host(Some(stub(false)));
    let root = Capability::root();
    read(&kernel, STATE, &root).expect("drawn");
    assert!(
        !cached(&kernel, STATE, &root),
        "a live board makes a live picture"
    );
    read(&kernel, KERNEL, &root).expect("drawn");
    assert!(
        cached(&kernel, KERNEL, &root),
        "and leaves urn:diagram:kernel alone"
    );
}
