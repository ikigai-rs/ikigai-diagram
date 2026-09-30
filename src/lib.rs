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
//! ```
//!
//! ## Two names, one function
//!
//! | name | what it is |
//! |---|---|
//! | [`urn:diagram:arrangement`](ARRANGEMENT) `of=<iri>` | the picture of whatever arrangement `of` names — a declaration read from a file, a peer's topology, a corridor's |
//! | [`urn:diagram:kernel`](KERNEL) | the picture of `urn:kernel:topology`: the arrangement the CALLER sees, since topology is per request |
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
//! `of` and is enforced by whatever `of` names.
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
//! reached twice is drawn once and referenced after. Long text is cut with an
//! ellipsis and carried whole in a `<title>` tooltip; every door row has one.
//!
//! The root `<svg>` is `role="img"` with a `<title>` and a `<desc>` that states the
//! arrangement in words; every label is real `<text>`; every color is a token in
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

pub use render::{render, WIDTH};

use ikigai_core::{
    ArgRef, ArgSpec, AsyncFnEndpoint, Description, EndpointSpace, Error, Exact, Invocation, Iri,
    ReprType, Representation, Request, Result, Topology, Verb,
};

/// The picture of the arrangement `of` names.
pub const ARRANGEMENT: &str = "urn:diagram:arrangement";
/// The picture of the caller's own arrangement, `urn:kernel:topology`.
pub const KERNEL: &str = "urn:diagram:kernel";
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

/// The module's endpoints: [`ARRANGEMENT`] and [`KERNEL`].
pub fn space() -> EndpointSpace {
    EndpointSpace::new()
        .bind(Exact::new(ARRANGEMENT), arrangement())
        .bind(Exact::new(KERNEL), kernel())
}

/// `urn:diagram:arrangement of=<iri>` — source `of` and draw it.
pub fn arrangement() -> AsyncFnEndpoint {
    AsyncFnEndpoint::new("diagram-arrangement", |inv| {
        Box::pin(async move {
            let of = of(inv)?;
            let (bytes, media) = source_turtle(inv, &of).await?;
            Ok(picture(&of, &bytes, &media))
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
            let topology = Iri::parse(TOPOLOGY).expect("urn:kernel:topology is an IRI");
            let turtle = inv.source(&topology).await?;
            // The kernel's own rendering failing to parse is core's bug, not an answer
            // about anything the caller named: an error, not a picture.
            let text = std::str::from_utf8(&turtle.bytes)
                .map_err(|_| Error::Endpoint(format!("{TOPOLOGY} is not UTF-8")))?;
            let tree = Topology::from_turtle(text).map_err(|e| {
                Error::Endpoint(format!("{TOPOLOGY} did not parse as an arrangement: {e}"))
            })?;
            Ok(svg(render(&tree)))
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
            .requires(CAP_INSPECT)
            .output(SVG),
    )
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
fn picture(of: &Iri, bytes: &[u8], media: &str) -> Representation {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return svg(render::problem(
            of.as_str(),
            &format!("it is served as {media} and is not UTF-8 text, so it cannot be Turtle"),
        ));
    };
    match Topology::from_turtle(text) {
        Ok(tree) => svg(render(&tree)),
        Err(e) if media == TURTLE => svg(render::problem(of.as_str(), &e.to_string())),
        Err(e) => svg(render::problem(
            of.as_str(),
            &format!("it is served as {media}, which nothing converts to {TURTLE}, and it does not parse as one: {e}"),
        )),
    }
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
