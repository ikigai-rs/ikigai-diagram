//! `ikigai-diagram` — a kernel's arrangement as a picture.
//!
//! The arrangement a request is resolved against is itself a resource:
//! `urn:kernel:topology` answers it as Turtle — the chain the request sees, its
//! fallbacks, mounts, aliases, limits, levels and endpoint spaces, and since
//! `ikigai-core` 0.1.83 every door with the endpoint that answers it. This module
//! draws that graph. It is a standalone **ikigai module crate**: a host links it in
//! and mounts [`space`].
//!
//! ```text
//! source urn:diagram:kernel                          the caller's own arrangement
//! source urn:diagram:arrangement of=urn:file:game.ttl  a declared space, as a file
//! source urn:diagram:state                           the caller's arrangement, with governor state
//! ```
//!
//! ## Two names, one function — and two more with a state layer
//!
//! | name | what it is |
//! |---|---|
//! | [`urn:diagram:arrangement`](ARRANGEMENT) `of=<iri>` | the picture of whatever arrangement `of` names — a declaration read from a file, a peer's topology, a corridor's |
//! | [`urn:diagram:kernel`](KERNEL) | the picture of `urn:kernel:topology`: the arrangement the CALLER sees, since topology is per request |
//! | [`urn:diagram:state`](STATE) | `urn:diagram:kernel`'s picture with the governor state the caller may read drawn over it |
//! | [`urn:diagram:state:arrangement`](STATE_ARRANGEMENT) `of=<iri>` | `urn:diagram:arrangement`'s picture with the same state layer |
//!
//! ## The state layer
//!
//! The two state names compose the arrangement with `urn:throttle:state` (ikigai-throttle's
//! state face: a board of the governors the caller may read), join them on `thr:governs`
//! (the IRI a governor names for its space is the IRI topology gives that node) and draw
//! both with [`render_with_state`], a pure function. They are SEPARATE names on purpose:
//! state is live, and joining it into `urn:diagram:kernel` would make that picture only as
//! cacheable as the board (the field guide's cacheability trap). The plain pictures never
//! read the board and are unchanged.
//!
//! - **The caller's authority shapes the picture.** The board is read under the caller's
//!   capability and lists only the governors it may read; the picture draws only those. A
//!   caller with no grant under `urn:cap:throttle:state:*` gets the plain picture byte for
//!   byte, with no hint that governors exist (UX record D28): the board is not even asked.
//! - **Cacheable exactly when the board is.** With the host's `StateBoard::attach` the
//!   board is cacheable and hangs from each governor's thread, so the picture is cached
//!   and redrawn on each state change; without it every read of the board is live, and so
//!   is the picture. A caller with no grant reads no board, so its picture is as cacheable
//!   as `urn:diagram:kernel`'s.
//! - **Not color alone**: every state is a word (`OPEN`, `FULL`, `HALF-OPEN`, ...), and the
//!   words below the picture, a `role="status"` region with `aria-live="polite"`, say
//!   everything the layer shows.
//!
//! Both are the same composition: source a resource that is an arrangement's
//! Turtle, parse it with core's [`Topology::from_turtle`], and draw it with
//! [`render`], a pure function. `urn:diagram:kernel` exists as its own name, rather
//! than as `arrangement of=urn:kernel:topology`, because it is the one arrangement
//! every host has and the one a control page wants linked, and because it is the
//! only one whose authority is known in advance (below).
//!
//! **No transreptor, on purpose.** A `text/turtle → image/svg+xml` transreptor
//! would be selected for ANY Turtle, and most Turtle is not an arrangement. The
//! caller names what to draw.
//!
//! ## Cacheable, and redrawn when the arrangement changes
//!
//! [`render`] is a pure function of the tree, so every picture is `.cacheable()` —
//! and hangs from whatever it sourced. The kernel's topology hangs from
//! `urn:kernel:bindings`, which every rebind cuts, so a rebind redraws
//! `urn:diagram:kernel`; a declaration read from a file hangs from that file's
//! thread, so editing the file redraws its picture.
//!
//! ## A resource that is not an arrangement is a picture too
//!
//! When `of` resolves but its representation is not an arrangement (a typo in a
//! declaration file, a document of some other kind), the answer is a picture that
//! says so and why — not an error. "This file does not declare an arrangement" is a
//! legitimate answer ABOUT the file, and an answer is cacheable where a refusal is
//! never cached: it hangs from the file's thread, so fixing the file redraws it,
//! and a page showing the picture does not recompute it on every read. Failures to
//! READ `of` — a name that does not resolve, a denial — are still errors, because
//! they are not answers about anything.
//!
//! ## Capabilities
//!
//! This module holds no authority of its own and declares none it does not
//! inherit. `of` is sourced by the kernel under the **caller's** capability, so a
//! caller can draw only what it can already read, and the module adds nothing it
//! could lend. `urn:diagram:kernel` declares `urn:cap:kernel:inspect` because
//! `urn:kernel:topology` enforces it: a caller without it is refused by the
//! sub-request, and an undeclared requirement would make the action manifold offer
//! the picture to callers who cannot have it (declared = enforced).
//! `urn:diagram:arrangement` declares nothing, because what it needs depends on
//! `of` and is enforced by whatever `of` names. `urn:diagram:state` declares what
//! `urn:diagram:kernel` does and `urn:diagram:state:arrangement` what
//! `urn:diagram:arrangement` does: neither needs the state grant (a caller without it
//! gets the plain picture), so neither declares it.
//!
//! ## The picture
//!
//! Nested boxes in a vertical flow, [`WIDTH`] pixels wide, one box per space:
//!
//! - a **chain** shows its corridors innermost first, then the root;
//! - a **fallback** numbers its layers in the order they are consulted;
//! - a **mount** is labeled with its prefix, around the space it mounts;
//! - an **alias** lists its rules in order, then the space it rewrites into;
//! - a **limit** is a dashed box — a hole over its family;
//! - a **level** shows its name and its seals;
//! - an **endpoint space** lists its doors in order: pattern, the endpoint that
//!   answers it, the match kind, and under a confined door the corridor it severs
//!   into;
//! - an **opaque** space is a dotted box: where the graph's knowledge stops.
//!
//! A named node shows its IRI; an anonymous one shows no skolem. A named space
//! reached twice is drawn once and referenced after. A module's configuration-free
//! space names itself `urn:iki:space:<module>` (this one is [`SPACE_ID`]), so the
//! picture of a host says which modules it holds. Long text is cut with an
//! ellipsis and carried whole in a `<title>` tooltip; every door row has one.
//!
//! The root `<svg>` is `role="img"` with a `<title>` and a `<desc>` that states the
//! arrangement in words, the names its spaces claim included; every label is real `<text>`; every color is a token in
//! [`palette`], with a dark scheme under `prefers-color-scheme`, and every pair the
//! picture draws meets the WCAG floor `ikigai-a11y` enforces (`tests/contrast.rs`).
//! The same tree renders byte-identical SVG.
//!
//! ## wasm
//!
//! The library builds for `wasm32-unknown-unknown` (CI's `wasm32 lib clippy`), so
//! the picture can be drawn in the browser. Core's `declare` feature parses with
//! oxttl/oxrdf, which reach getrandom 0.3 through `rand`; core enables getrandom's
//! `wasm_js` feature, and from getrandom 0.3.4 that feature alone selects the
//! browser backend, so a consumer needs no `--cfg getrandom_backend` flag. (A lock
//! pinned to getrandom 0.3.0–0.3.3 still does; `cargo update -p getrandom` is the
//! cure.)
#![forbid(unsafe_code)]

pub mod palette;
mod render;
pub mod state;

pub use render::{
    render, render_in, render_with_changes_in, render_with_state, render_with_state_in, WIDTH,
};

use ikigai_core::{
    space_iri, ArgRef, ArgSpec, AsyncFnEndpoint, Description, EndpointSpace, Error, Exact,
    Invocation, Iri, ReprType, Representation, Request, Result, Topology, Verb,
};

use palette::Colors;
use state::{Board, BOARD, STATE_CAP_FAMILY};

/// The picture of the arrangement `of` names.
pub const ARRANGEMENT: &str = "urn:diagram:arrangement";
/// The picture of the caller's own arrangement, `urn:kernel:topology`.
pub const KERNEL: &str = "urn:diagram:kernel";
/// [`KERNEL`]'s picture with the governor state the caller may read drawn over it.
pub const STATE: &str = "urn:diagram:state";
/// [`ARRANGEMENT`]'s picture (`of=<iri>`) with the governor state drawn over it.
pub const STATE_ARRANGEMENT: &str = "urn:diagram:state:arrangement";
/// The resource `urn:diagram:kernel` draws.
pub const TOPOLOGY: &str = "urn:kernel:topology";
/// The one media type this module serves.
pub const SVG: &str = "image/svg+xml";
/// What `urn:kernel:topology` requires, and so what `urn:diagram:kernel` declares.
pub const CAP_INSPECT: &str = "urn:cap:kernel:inspect";

/// The media type an arrangement is read as.
const TURTLE: &str = "text/turtle";
/// The XSD `anyURI` datatype IRI — the `class` of `of`.
const XSD_ANY_URI: &str = "http://www.w3.org/2001/XMLSchema#anyURI";
/// The XSD `string` datatype IRI — the `class` of `scheme`.
const XSD_STRING: &str = "http://www.w3.org/2001/XMLSchema#string";

/// The name [`space`] claims: `urn:iki:space:diagram`.
pub const SPACE_ID: &str = "urn:iki:space:diagram";

/// The module's endpoints: [`ARRANGEMENT`], [`KERNEL`], [`STATE`] and
/// [`STATE_ARRANGEMENT`], named [`SPACE_ID`].
///
/// Configuration-free (no parameters, nothing read while building it), so it names
/// itself: every call holds the same four doors. The name goes on LAST, because a
/// later `bind` drops it (a space with other doors is a different space).
pub fn space() -> EndpointSpace {
    EndpointSpace::new()
        .bind(Exact::new(ARRANGEMENT), arrangement())
        .bind(Exact::new(KERNEL), kernel())
        .bind(Exact::new(STATE), state())
        .bind(Exact::new(STATE_ARRANGEMENT), state_arrangement())
        .named(space_iri("diagram"))
}

/// `urn:diagram:arrangement of=<iri>` — source `of` and draw it.
pub fn arrangement() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("diagram-arrangement", |inv| {
        Box::pin(async move {
            let of = of(inv)?;
            let colors = colors(inv)?;
            let (bytes, media) = source_turtle(inv, &of).await?;
            Ok(picture(&of, &bytes, &media, colors))
        })
    })
    .with_description(
        Description::new("diagram-arrangement")
            .title("Arrangement diagram")
            .summary(
                "The picture of an arrangement: the resource `of` names is sourced (under the \
                 caller's capability) as the Turtle `urn:kernel:topology` writes — a kernel's \
                 topology, or a declaration of a space — and drawn as nested boxes in SVG. \
                 Cacheable, and redrawn when `of` changes. A resource that is not an \
                 arrangement is drawn as a picture saying why.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(scheme_input())
            .input(
                ArgSpec::new("of")
                    .summary(
                        "the IRI of a resource whose representation is an arrangement's Turtle \
                         (urn:kernel:topology, or a declaration such as urn:file:space.ttl); the \
                         one required input, so a positional value lands here",
                    )
                    .class(XSD_ANY_URI),
            )
            .output(SVG),
    )
}

/// `urn:diagram:kernel` — the picture of `urn:kernel:topology`, the arrangement the
/// caller's own request sees.
pub fn kernel() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("diagram-kernel", |inv| {
        Box::pin(async move {
            let colors = colors(inv)?;
            let tree = focused(kernel_tree(inv).await?, inv)?;
            Ok(svg(render_in(&tree, colors)))
        })
    })
    .with_description(
        Description::new("diagram-kernel")
            .title("Kernel arrangement diagram")
            .summary(
                "The picture of urn:kernel:topology — the arrangement this request is resolved \
                 against: its corridors innermost first, then the root, every space as a box \
                 and every door with the endpoint that answers it, in SVG. Cacheable; a rebind \
                 redraws it.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(scheme_input())
            .input(focus_input())
            .requires(CAP_INSPECT)
            .output(SVG),
    )
}

/// `urn:diagram:state` — the picture of `urn:kernel:topology` with the governor state
/// the caller may read drawn over it.
pub fn state() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("diagram-state", |inv| {
        Box::pin(async move {
            let colors = colors(inv)?;
            let before = before(inv)?;
            let tree = focused(kernel_tree(inv).await?, inv)?;
            Ok(svg(draw_state(
                &tree,
                &board(inv).await?,
                before.as_ref(),
                colors,
            )))
        })
    })
    .with_description(
        Description::new("diagram-state")
            .title("Kernel arrangement diagram, with governor state")
            .summary(
                "The picture urn:diagram:kernel draws, with a state layer: what the governors on \
                 urn:throttle:state hold (a circuit open or closed, a window's hits, a bulkhead's \
                 running and parked work, a failover's last candidate), drawn in the box of the \
                 space each one governs and stated in words in a live region. The board is read \
                 under the caller's capability: a caller who may read no governor gets exactly \
                 urn:diagram:kernel's picture. Cacheable when the board is.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(scheme_input())
            .input(focus_input())
            .input(before_input())
            .requires(CAP_INSPECT)
            .output(SVG),
    )
}

/// `urn:diagram:state:arrangement of=<iri>` — the picture of the arrangement `of`
/// names with the governor state drawn over it.
pub fn state_arrangement() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("diagram-state-arrangement", |inv| {
        Box::pin(async move {
            let of = of(inv)?;
            let colors = colors(inv)?;
            let before = before(inv)?;
            let (bytes, media) = source_turtle(inv, &of).await?;
            let tree = match std::str::from_utf8(&bytes).map(Topology::from_turtle) {
                Ok(Ok(tree)) => focused(tree, inv)?,
                // Not an arrangement: the same picture saying why, and no state to draw.
                _ => return Ok(picture(&of, &bytes, &media, colors)),
            };
            Ok(svg(draw_state(
                &tree,
                &board(inv).await?,
                before.as_ref(),
                colors,
            )))
        })
    })
    .with_description(
        Description::new("diagram-state-arrangement")
            .title("Arrangement diagram, with governor state")
            .summary(
                "The picture urn:diagram:arrangement draws of `of`, with the state layer \
                 urn:diagram:state draws: the governors on urn:throttle:state the caller may \
                 read, joined to the arrangement's spaces on thr:governs. A caller who may read \
                 no governor gets exactly urn:diagram:arrangement's picture.",
            )
            .verb(Verb::Source)
            .verb(Verb::Meta)
            .input(scheme_input())
            .input(focus_input())
            .input(before_input())
            .input(
                ArgSpec::new("of")
                    .summary(
                        "the IRI of a resource whose representation is an arrangement's Turtle; \
                         the one required input, so a positional value lands here",
                    )
                    .class(XSD_ANY_URI),
            )
            .output(SVG),
    )
}

/// The caller's own arrangement, parsed. The kernel's own rendering failing to parse is
/// core's bug, not an answer about anything the caller named: an error, not a picture.
async fn kernel_tree(inv: &Invocation<'_>) -> Result<Topology> {
    let topology = Iri::parse(TOPOLOGY).expect("urn:kernel:topology is an IRI");
    let turtle = inv.source(&topology).await?;
    let text = std::str::from_utf8(&turtle.bytes)
        .map_err(|_| Error::Endpoint(format!("{TOPOLOGY} is not UTF-8")))?;
    Topology::from_turtle(text)
        .map_err(|e| Error::Endpoint(format!("{TOPOLOGY} did not parse as an arrangement: {e}")))
}

/// The governor state this caller may read, or an empty board.
///
/// ★ **A caller who may read no governor gets no sign that governors exist** (UX record
/// D28, ledger #2176). Three ways to be that caller, and each gives an EMPTY board, which
/// draws the plain picture byte for byte:
///
/// - **No grant under [`STATE_CAP_FAMILY`].** The board's floor would refuse it, so the
///   board is not asked at all ([`Capability::satisfies`](ikigai_core::Capability::satisfies)
///   is the kernel's own pre-flight predicate). No refusal is issued, logged or traced, and
///   the picture depends on the topology alone, so it stays cacheable: a refusal never is.
/// - **No board** (nothing answers `urn:throttle:state`: `Unresolved` or `NotFound`). Its
///   absence hangs from the bindings thread the topology already hangs from, so mounting
///   one redraws the picture.
/// - **Denied anyway** (a grant the floor admits and the board's own rule does not).
///
/// Any other failure to read the board, and a board that is not one, is an error: it is
/// not an answer about anything, the same rule `of` follows.
///
/// The board is read under the caller's capability, never more, and lists only the
/// governors that capability may read; the picture draws only those.
async fn board(inv: &Invocation<'_>) -> Result<Board> {
    if !inv.capability.satisfies(STATE_CAP_FAMILY) {
        return Ok(Board::empty());
    }
    let board = Iri::parse(BOARD).expect("urn:throttle:state is an IRI");
    let request = Request::new(Verb::Source, board)
        .with_arg("as", ArgRef::Inline(TURTLE.as_bytes().to_vec()));
    let repr = match inv.issue(request).await {
        Ok(repr) => repr,
        Err(Error::Denied(_) | Error::NotFound(_) | Error::Unresolved(_)) => {
            return Ok(Board::empty())
        }
        Err(e) => return Err(e),
    };
    let text = std::str::from_utf8(&repr.bytes)
        .map_err(|_| Error::Endpoint(format!("{BOARD} is not UTF-8")))?;
    Board::from_turtle(text)
        .map_err(|e| Error::Endpoint(format!("{BOARD} did not parse as a state board: {e}")))
}

/// The picture as a representation: SVG, cacheable (a pure function of what was
/// sourced, whose threads the kernel unions in).
fn svg(text: String) -> Representation {
    Representation::new(ReprType::new(SVG), text.into_bytes()).cacheable()
}

/// The `of` argument, as an IRI. A value that is not an IRI is refused as the
/// caller's mistake — nothing was read, so there is nothing to draw — and a
/// Turtle document piped in where the IRI belongs says so, since that is the shape
/// a REPL user reaches for first.
fn of(inv: &Invocation<'_>) -> Result<Iri> {
    let raw = inv.inline_str("of")?.trim();
    Iri::parse(raw).map_err(|e| {
        let hint = if raw.starts_with('@') || raw.contains('\n') {
            "; `of` names the resource to draw (e.g. of=urn:kernel:topology) — pass its IRI, \
             not its Turtle"
        } else {
            ""
        };
        Error::InvalidArgument {
            name: "of".to_string(),
            detail: format!("not an IRI ({e}){hint}"),
        }
    })
}

/// Source `of`, converting it to Turtle through the kernel's transreptors when it
/// is served as something else. Returns the bytes and the media type they arrived
/// as, which the picture of a problem names.
async fn source_turtle(inv: &Invocation<'_>, of: &Iri) -> Result<(Vec<u8>, String)> {
    let repr = inv.source(of).await?;
    let media = bare(&repr.repr_type.media_type);
    if media == TURTLE {
        return Ok((repr.bytes, media));
    }
    let Some(plan) = inv.select_transreptor(&media, TURTLE) else {
        // Nothing converts it: parse what came back. A file served as
        // application/octet-stream may still be Turtle, and when it is not, the
        // picture says what type it arrived as.
        return Ok((repr.bytes, media));
    };
    let mut bytes = repr.bytes;
    for step in plan {
        let endpoint = Iri::parse(&step.endpoint).map_err(|e| {
            Error::Endpoint(format!(
                "transreptor `{}` is not an IRI: {e}",
                step.endpoint
            ))
        })?;
        let request = Request::new(Verb::Source, endpoint)
            .with_arg("content", ArgRef::Inline(bytes))
            .with_arg("as", ArgRef::Inline(step.to.into_bytes()));
        bytes = inv.issue(request).await?.bytes;
    }
    Ok((bytes, TURTLE.to_string()))
}

/// The picture of what was read: the arrangement, or why it is not one.
fn picture(of: &Iri, bytes: &[u8], media: &str, colors: Colors) -> Representation {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return svg(render::problem_in(
            of.as_str(),
            &format!("it is served as {media} and is not UTF-8 text, so it cannot be Turtle"),
            colors,
        ));
    };
    match Topology::from_turtle(text) {
        Ok(tree) => svg(render_in(&tree, colors)),
        Err(e) if media == TURTLE => svg(render::problem_in(of.as_str(), &e.to_string(), colors)),
        Err(e) => svg(render::problem_in(
            of.as_str(),
            &format!("it is served as {media}, which nothing converts to {TURTLE}, and it does not parse as one: {e}"),
            colors,
        )),
    }
}

/// The `scheme` a caller asked for: `auto` (the default: both schemes, chosen by the
/// viewer's system setting), `light` or `dark` ([`Colors`]). Anything else is refused.
fn colors(inv: &Invocation<'_>) -> Result<Colors> {
    match inv.inline_str("scheme") {
        Err(_) => Ok(Colors::Auto),
        Ok(word) => Colors::parse(word).ok_or_else(|| Error::InvalidArgument {
            name: "scheme".to_string(),
            detail: format!("`{word}` is not a scheme: auto, light or dark"),
        }),
    }
}

/// The earlier board a caller passed as `before` (inline `text/turtle` of
/// `urn:throttle:state`), or `None`. One that does not parse as a board is refused.
fn before(inv: &Invocation<'_>) -> Result<Option<Board>> {
    match inv.inline_str("before") {
        Err(_) => Ok(None),
        Ok(text) => Board::from_turtle(text)
            .map(Some)
            .map_err(|e| Error::InvalidArgument {
                name: "before".to_string(),
                detail: format!("not a board as urn:throttle:state writes one: {e}"),
            }),
    }
}

/// The subtree whose space is named `focus`, or the whole tree when there is no `focus`.
/// A `focus` that names no space in the arrangement is refused, never drawn as nothing.
fn focused(tree: Topology, inv: &Invocation<'_>) -> Result<Topology> {
    let Ok(focus) = inv.inline_str("focus") else {
        return Ok(tree);
    };
    fn find(node: &Topology, focus: &str) -> Option<Topology> {
        if node.id.as_ref().is_some_and(|id| id.as_str() == focus) {
            return Some(node.clone());
        }
        node.children.iter().find_map(|c| find(c, focus))
    }
    find(&tree, focus).ok_or_else(|| Error::InvalidArgument {
        name: "focus".to_string(),
        detail: format!("`{focus}` names no space in this arrangement"),
    })
}

/// The state picture, with what changed since `before` marked when there is one.
fn draw_state(tree: &Topology, board: &Board, before: Option<&Board>, colors: Colors) -> String {
    match before {
        Some(before) => render_with_changes_in(tree, board, before, colors),
        None => render_with_state_in(tree, board, colors),
    }
}

/// The `focus` input: draw only the space with this IRI, and what it encloses.
fn focus_input() -> ArgSpec {
    ArgSpec::new("focus").optional().class(XSD_ANY_URI).summary(
        "the IRI of one space in the arrangement (a governed space, say): draw only it \
             and what it encloses, legible where the whole arrangement is too wide",
    )
}

/// The `before` input: an earlier board, to mark what changed since.
fn before_input() -> ArgSpec {
    ArgSpec::new("before").optional().class(XSD_STRING).summary(
        "an earlier board, as text/turtle of urn:throttle:state: each key that differs is \
             marked after its state word (was OPEN, new, GONE), and the words begin with what \
             changed",
    )
}

/// The `scheme` input every picture declares.
fn scheme_input() -> ArgSpec {
    ArgSpec::new("scheme")
        .optional()
        .class(XSD_STRING)
        .one_of(Colors::WORDS)
        .default_value("auto")
        .summary(
            "the colors to draw in: auto (both schemes, chosen by the viewer's system \
             setting), or light or dark, for a page with a theme of its own",
        )
}

/// A media type without its parameters, lowercased: `text/turtle; charset=utf-8`
/// is `text/turtle`.
fn bare(media: &str) -> String {
    media
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}
