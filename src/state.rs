//! Governor state, read from `urn:throttle:state`'s Turtle face.
//!
//! The state overlay draws what the board says and nothing else. This module parses the
//! board into a [`Board`]: the governors it lists, their keys and a failover's candidates.
//! A governor node the board does not list (by `thr:governor`) is never read, so the
//! picture cannot show a governor the board did not offer this caller.
//!
//! The contract is ikigai-throttle's (PR 27, `c06ea47`, pinned there byte for byte in
//! `tests/state.rs::the_board_in_turtle`). This crate does NOT depend on ikigai-throttle:
//! it reads the Turtle, so the names below are spelled here, and the namespace lives in
//! ONE constant, [`THR`], because whether `thr:` stays its own vocabulary or folds into
//! `ik:` is still open (ledger #2175). A fold is a one-line change here.
//!
//! ```
//! use ikigai_diagram::state::{Board, KeyState, Mode};
//!
//! let turtle = [
//!     "@prefix thr: <https://ikigai-rs.dev/ns/throttle#> .",
//!     "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .",
//!     "<urn:throttle:state> a thr:StateBoard ; thr:governor <urn:throttle:state:b> .",
//!     "<urn:throttle:state:b> a thr:CircuitBreaker ; thr:name \"b\" ;",
//!     "    thr:governs <urn:iki:space:demo> ;",
//!     "    thr:threshold \"2\"^^xsd:nonNegativeInteger ;",
//!     "    thr:cooldownMillis \"30000\"^^xsd:nonNegativeInteger ;",
//!     "    thr:key <urn:throttle:state:b:key:urn:demo:down> .",
//!     "<urn:throttle:state:b:key:urn:demo:down> a thr:Circuit ;",
//!     "    thr:target <urn:demo:down> ;",
//!     "    thr:failures \"2\"^^xsd:nonNegativeInteger ; thr:state thr:Open .",
//! ]
//! .join("\n");
//! let board = Board::from_turtle(&turtle).unwrap();
//! let breaker = &board.governors()[0];
//! assert_eq!(breaker.name(), "b");
//! assert_eq!(breaker.governs(), Some("urn:iki:space:demo"));
//! match &breaker.keys()[0] {
//!     KeyState::Circuit { target, failures, mode } => {
//!         assert_eq!((target.as_str(), *failures), ("urn:demo:down", 2));
//!         assert_eq!(*mode, Mode::Open);
//!         assert_eq!(mode.word(), "open"); // every state has a word, never color alone
//!     }
//!     other => panic!("{other:?}"),
//! }
//! ```

use std::collections::BTreeMap;

use oxrdf::{NamedOrBlankNode, Term};

/// The throttle state vocabulary's namespace. ⚠ The ONE place it is spelled (ledger
/// #2175 may fold it into `ik:`).
pub const THR: &str = "https://ikigai-rs.dev/ns/throttle#";
/// The shared ikigai vocabulary, for `ik:order` on a failover's candidates.
const IK: &str = "https://ikigai-rs.dev/ns#";
const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";

/// The board: every governor this caller may read, in one resource.
pub const BOARD: &str = "urn:throttle:state";
/// The family every name of the board declares. A caller that does not satisfy it is
/// refused by the board's floor, so the overlay never asks (see `urn:diagram:state`).
pub const STATE_CAP_FAMILY: &str = "urn:cap:throttle:state:*";

/// What one governor's key, or a failover's route, is doing now.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Mode {
    /// A window or bulkhead with room (`thr:Admitting`).
    Admitting,
    /// A window or bulkhead at its limit (`thr:Full`).
    Full,
    /// A circuit letting calls through (`thr:Closed`).
    Closed,
    /// A circuit refusing calls (`thr:Open`).
    Open,
    /// A circuit letting one trial call through (`thr:HalfOpen`).
    HalfOpen,
    /// A route whose last call was answered (`thr:Answered`).
    Answered,
    /// A route whose last call failed at every candidate (`thr:Failed`).
    Failed,
    /// A state term newer than this renderer: its local name, drawn as it is.
    Other(String),
}

/// How a state reads at a glance, which picks its color. The word always says it too.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Letting work through.
    Go,
    /// Letting a little through: a half-open circuit.
    Wait,
    /// Refusing, or failed.
    Stop,
    /// A term this renderer does not know.
    Unknown,
}

impl Mode {
    fn from_iri(iri: &str) -> Mode {
        let local = iri.strip_prefix(THR).unwrap_or(iri);
        match local {
            "Admitting" => Mode::Admitting,
            "Full" => Mode::Full,
            "Closed" => Mode::Closed,
            "Open" => Mode::Open,
            "HalfOpen" => Mode::HalfOpen,
            "Answered" => Mode::Answered,
            "Failed" => Mode::Failed,
            other => Mode::Other(other.to_string()),
        }
    }

    /// The state in words, as the picture and its text summary say it.
    pub fn word(&self) -> &str {
        match self {
            Mode::Admitting => "admitting",
            Mode::Full => "full",
            Mode::Closed => "closed",
            Mode::Open => "open",
            Mode::HalfOpen => "half-open",
            Mode::Answered => "answered",
            Mode::Failed => "failed",
            Mode::Other(local) => local,
        }
    }

    pub(crate) fn tone(&self) -> Tone {
        match self {
            Mode::Admitting | Mode::Closed | Mode::Answered => Tone::Go,
            Mode::HalfOpen => Tone::Wait,
            Mode::Full | Mode::Open | Mode::Failed => Tone::Stop,
            Mode::Other(_) => Tone::Unknown,
        }
    }
}

/// One key of a governor: a rate window, a circuit, a bulkhead, or a failover's route.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeyState {
    /// A rate limit's window over a prefix (`thr:Window`).
    Window {
        /// The prefix, as the face spells it (IRI-escaped).
        prefix: String,
        max: u64,
        window_millis: u64,
        hits: u64,
        mode: Mode,
    },
    /// A breaker's circuit for one target (`thr:Circuit`).
    Circuit {
        target: String,
        failures: u64,
        mode: Mode,
    },
    /// A throttle's bulkhead over a prefix (`thr:Bulkhead`).
    Bulkhead {
        /// The prefix, as the face spells it (IRI-escaped).
        prefix: String,
        max: u64,
        running: u64,
        parked: u64,
        mode: Mode,
    },
    /// A failover's route for one target (`thr:Route`).
    Route {
        target: String,
        /// The `ik:order` of the candidate the last call ended at, when the board names
        /// one of this failover's candidates.
        last_candidate: Option<u64>,
        mode: Mode,
    },
    /// A key of a class newer than this renderer: its local name.
    Other { class: String },
}

/// One of a failover's spaces.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Candidate {
    /// `ik:order`: the order the failover tries it in.
    pub order: u64,
    /// The space it governs: the IRI `urn:kernel:topology` gives that node.
    pub governs: Option<String>,
}

/// What kind of governor, and the facts its kind carries.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GovernorKind {
    RateLimit,
    Throttle,
    CircuitBreaker {
        threshold: Option<u64>,
        cooldown_millis: Option<u64>,
    },
    Failover {
        candidates: Vec<Candidate>,
    },
    /// A governor class newer than this renderer: its local name.
    Other(String),
}

/// One governor the board lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Governor {
    iri: String,
    name: String,
    governs: Option<String>,
    kind: GovernorKind,
    keys: Vec<KeyState>,
}

impl Governor {
    /// The governor's own name on the board (`urn:throttle:state:{name}`).
    pub fn iri(&self) -> &str {
        &self.iri
    }
    /// `thr:name`, as the host named it.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// `thr:governs`: the space it governs, the same IRI topology gives that node. A
    /// failover has none (it is opaque in topology); its candidates do.
    pub fn governs(&self) -> Option<&str> {
        self.governs.as_deref()
    }
    pub fn kind(&self) -> &GovernorKind {
        &self.kind
    }
    /// Its keys, in the board's order (sorted by IRI).
    pub fn keys(&self) -> &[KeyState] {
        &self.keys
    }
    /// What kind of governor it is, in words.
    pub fn kind_word(&self) -> &str {
        match &self.kind {
            GovernorKind::RateLimit => "rate limit",
            GovernorKind::Throttle => "throttle",
            GovernorKind::CircuitBreaker { .. } => "circuit breaker",
            GovernorKind::Failover { .. } => "failover",
            GovernorKind::Other(local) => local,
        }
    }
}

/// The governors a board lists, sorted by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Board {
    governors: Vec<Governor>,
}

/// Why a representation is not a state board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BoardError(String);

impl std::fmt::Display for BoardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BoardError {}

/// The triples, by subject: predicate IRI and object, in document order.
type Graph = BTreeMap<String, Vec<(String, Term)>>;

impl Board {
    /// A board with no governors: the overlay draws nothing over the arrangement.
    pub fn empty() -> Board {
        Board::default()
    }

    /// The governors, sorted by name.
    pub fn governors(&self) -> &[Governor] {
        &self.governors
    }

    pub fn is_empty(&self) -> bool {
        self.governors.is_empty()
    }

    /// Parse the board's Turtle face (`urn:throttle:state as=text/turtle`).
    ///
    /// Only the governors `<urn:throttle:state>` lists by `thr:governor` are read; any
    /// other governor node in the document is ignored. A blank node anywhere, a board
    /// node that is missing, a listed governor that is not described, or a count that
    /// is not a non-negative integer is an error: the face writes none of those.
    pub fn from_turtle(turtle: &str) -> Result<Board, BoardError> {
        let graph = parse(turtle)?;
        let thr = |local: &str| format!("{THR}{local}");
        let board = graph
            .get(BOARD)
            .ok_or_else(|| BoardError(format!("no <{BOARD}> node: this is not the state board")))?;
        if !has_type(board, &thr("StateBoard")) {
            return Err(BoardError(format!("<{BOARD}> is not a thr:StateBoard")));
        }
        let mut governors = Vec::new();
        for iri in iris(board, &thr("governor")) {
            let node = graph.get(&iri).ok_or_else(|| {
                BoardError(format!("the board lists <{iri}> but does not describe it"))
            })?;
            governors.push(governor(&graph, &iri, node)?);
        }
        governors.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.iri.cmp(&b.iri)));
        governors.dedup_by(|a, b| a.iri == b.iri);
        Ok(Board { governors })
    }
}

fn parse(turtle: &str) -> Result<Graph, BoardError> {
    let mut graph: Graph = BTreeMap::new();
    for triple in oxttl::TurtleParser::new().for_reader(turtle.as_bytes()) {
        let triple = triple.map_err(|e| BoardError(format!("not Turtle: {e}")))?;
        let subject = match triple.subject {
            NamedOrBlankNode::NamedNode(n) => n.into_string(),
            other => {
                return Err(BoardError(format!(
                    "a blank node ({other}): the state face skolemizes every node"
                )))
            }
        };
        if let Term::BlankNode(b) = &triple.object {
            return Err(BoardError(format!(
                "a blank node ({b}): the state face skolemizes every node"
            )));
        }
        graph
            .entry(subject)
            .or_default()
            .push((triple.predicate.into_string(), triple.object));
    }
    Ok(graph)
}

fn has_type(props: &[(String, Term)], class: &str) -> bool {
    iris(props, RDF_TYPE).iter().any(|t| t == class)
}

/// The `thr:` local name of the node's first `thr:` type, if any.
fn thr_class(props: &[(String, Term)]) -> Option<String> {
    let mut classes: Vec<String> = iris(props, RDF_TYPE)
        .into_iter()
        .filter_map(|t| t.strip_prefix(THR).map(str::to_string))
        .collect();
    classes.sort();
    classes.into_iter().next()
}

/// Every IRI object of `predicate`, sorted (a graph is a set; the face sorts too).
fn iris(props: &[(String, Term)], predicate: &str) -> Vec<String> {
    let mut out: Vec<String> = props
        .iter()
        .filter(|(p, _)| p == predicate)
        .filter_map(|(_, o)| match o {
            Term::NamedNode(n) => Some(n.as_str().to_string()),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

fn iri(props: &[(String, Term)], predicate: &str) -> Option<String> {
    iris(props, predicate).into_iter().next()
}

fn literal(props: &[(String, Term)], predicate: &str) -> Option<String> {
    props.iter().find_map(|(p, o)| match o {
        Term::Literal(l) if p == predicate => Some(l.value().to_string()),
        _ => None,
    })
}

fn count(
    subject: &str,
    props: &[(String, Term)],
    predicate: &str,
) -> Result<Option<u64>, BoardError> {
    literal(props, predicate)
        .map(|v| {
            v.parse::<u64>().map_err(|_| {
                BoardError(format!(
                    "<{subject}> {predicate} is `{v}`, not a non-negative integer"
                ))
            })
        })
        .transpose()
}

fn required(subject: &str, props: &[(String, Term)], predicate: &str) -> Result<u64, BoardError> {
    count(subject, props, predicate)?
        .ok_or_else(|| BoardError(format!("<{subject}> has no {predicate}")))
}

fn mode(props: &[(String, Term)]) -> Mode {
    iri(props, &format!("{THR}state"))
        .map(|m| Mode::from_iri(&m))
        .unwrap_or_else(|| Mode::Other("no state".into()))
}

fn governor(graph: &Graph, iri: &str, props: &[(String, Term)]) -> Result<Governor, BoardError> {
    let thr = |local: &str| format!("{THR}{local}");
    let class = thr_class(props).unwrap_or_else(|| "Governor".into());
    let kind = match class.as_str() {
        "RateLimit" => GovernorKind::RateLimit,
        "Throttle" => GovernorKind::Throttle,
        "CircuitBreaker" => GovernorKind::CircuitBreaker {
            threshold: count(iri, props, &thr("threshold"))?,
            cooldown_millis: count(iri, props, &thr("cooldownMillis"))?,
        },
        "Failover" => {
            let mut candidates = Vec::new();
            for cell in iris(props, &thr("candidate")) {
                let Some(cell_props) = graph.get(&cell) else {
                    return Err(BoardError(format!(
                        "<{iri}> names candidate <{cell}> but does not describe it"
                    )));
                };
                candidates.push(Candidate {
                    order: required(&cell, cell_props, &format!("{IK}order"))?,
                    governs: self::iri(cell_props, &thr("governs")),
                });
            }
            candidates.sort_by_key(|c| c.order);
            GovernorKind::Failover { candidates }
        }
        other => GovernorKind::Other(other.to_string()),
    };
    let mut keys = Vec::new();
    for key_iri in iris(props, &thr("key")) {
        let Some(key_props) = graph.get(&key_iri) else {
            return Err(BoardError(format!(
                "<{iri}> names key <{key_iri}> but does not describe it"
            )));
        };
        keys.push(key(graph, &key_iri, key_props)?);
    }
    Ok(Governor {
        iri: iri.to_string(),
        name: literal(props, &thr("name")).unwrap_or_else(|| iri.to_string()),
        governs: self::iri(props, &thr("governs")),
        kind,
        keys,
    })
}

fn key(graph: &Graph, iri: &str, props: &[(String, Term)]) -> Result<KeyState, BoardError> {
    let thr = |local: &str| format!("{THR}{local}");
    let text = |predicate: &str| -> String {
        literal(props, &thr(predicate))
            .or_else(|| self::iri(props, &thr(predicate)))
            .unwrap_or_default()
    };
    let class = thr_class(props).unwrap_or_else(|| "Key".into());
    Ok(match class.as_str() {
        "Window" => KeyState::Window {
            prefix: text("prefix"),
            max: required(iri, props, &thr("max"))?,
            window_millis: required(iri, props, &thr("windowMillis"))?,
            hits: required(iri, props, &thr("hits"))?,
            mode: mode(props),
        },
        "Circuit" => KeyState::Circuit {
            target: text("target"),
            failures: required(iri, props, &thr("failures"))?,
            mode: mode(props),
        },
        "Bulkhead" => KeyState::Bulkhead {
            prefix: text("prefix"),
            max: required(iri, props, &thr("max"))?,
            running: required(iri, props, &thr("running"))?,
            parked: required(iri, props, &thr("parked"))?,
            mode: mode(props),
        },
        "Route" => KeyState::Route {
            target: text("target"),
            last_candidate: self::iri(props, &thr("lastCandidate"))
                .and_then(|c| graph.get(&c))
                .map(|c| required(iri, c, &format!("{IK}order")))
                .transpose()?,
            mode: mode(props),
        },
        other => KeyState::Other {
            class: other.to_string(),
        },
    })
}

/// A span of milliseconds the way the face's text says it: `10s`, `1500ms`.
pub(crate) fn span(millis: u64) -> String {
    if millis.is_multiple_of(1000) {
        format!("{}s", millis / 1000)
    } else {
        format!("{millis}ms")
    }
}

impl KeyState {
    /// What this key is over, the same across two boards: its kind and its target or prefix.
    pub(crate) fn id(&self) -> String {
        match self {
            KeyState::Window { prefix, .. } => format!("window {prefix}"),
            KeyState::Circuit { target, .. } => format!("circuit {target}"),
            KeyState::Bulkhead { prefix, .. } => format!("bulkhead {prefix}"),
            KeyState::Route { target, .. } => format!("route {target}"),
            KeyState::Other { class } => format!("other {class}"),
        }
    }

    /// Its state, when its kind has one.
    pub(crate) fn mode(&self) -> Option<&Mode> {
        match self {
            KeyState::Window { mode, .. }
            | KeyState::Circuit { mode, .. }
            | KeyState::Bulkhead { mode, .. }
            | KeyState::Route { mode, .. } => Some(mode),
            KeyState::Other { .. } => None,
        }
    }
}

/// How one key differs from an earlier board's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Change {
    /// The earlier board did not have it.
    New,
    /// The earlier board had it, in this state.
    Was(KeyState),
}

/// What changed between an earlier board and a later one, key by key: what is new, what
/// differs (and what it was), and what is gone. Drawn by
/// [`render_with_changes_in`](crate::render_with_changes_in) as a marker after each changed
/// key's state word, and stated first in the picture's words.
///
/// A key is the same key on both boards when its governor and what it is over (a circuit's
/// target, a window's prefix) are the same.
///
/// ```
/// use ikigai_diagram::state::{Board, Changes};
///
/// let board = |state: &str, failures: u32| Board::from_turtle(&[
///     "@prefix thr: <https://ikigai-rs.dev/ns/throttle#> .",
///     "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .",
///     "<urn:throttle:state> a thr:StateBoard ; thr:governor <urn:throttle:state:b> .",
///     "<urn:throttle:state:b> a thr:CircuitBreaker ; thr:name \"b\" ;",
///     "    thr:key <urn:throttle:state:b:key:urn:demo:down> .",
///     "<urn:throttle:state:b:key:urn:demo:down> a thr:Circuit ; thr:target <urn:demo:down> ;",
///     &format!("    thr:failures \"{failures}\"^^xsd:nonNegativeInteger ; thr:state thr:{state} ."),
/// ].join("\n")).unwrap();
///
/// let changes = Changes::between(&board("Closed", 0), &board("Open", 3));
/// assert_eq!(changes.sentences(), ["Circuit urn:demo:down: was closed, now open, 3 consecutive transient failures."]);
/// assert!(Changes::between(&board("Open", 3), &board("Open", 3)).is_empty());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// By (governor IRI, key id).
    keys: BTreeMap<(String, String), Change>,
    /// Keys the earlier board had and this one does not, by governor IRI.
    gone: BTreeMap<String, Vec<KeyState>>,
    /// What each change says, in the later board's order, then what is gone.
    sentences: Vec<String>,
}

impl Changes {
    /// What changed from `before` to `after`.
    pub fn between(before: &Board, after: &Board) -> Changes {
        let mut changes = Changes::default();
        let earlier = |g: &Governor| before.governors().iter().find(|b| b.iri() == g.iri());
        for g in after.governors() {
            let old = earlier(g);
            for k in g.keys() {
                let id = k.id();
                let was = old.and_then(|o| o.keys().iter().find(|ok| ok.id() == id));
                let change = match was {
                    None => Change::New,
                    Some(w) if w == k => continue,
                    Some(w) => Change::Was(w.clone()),
                };
                changes.sentences.push(change_sentence(k, &change));
                changes.keys.insert((g.iri().to_string(), id), change);
            }
            if let Some(o) = old {
                let gone: Vec<KeyState> = o
                    .keys()
                    .iter()
                    .filter(|ok| !g.keys().iter().any(|k| k.id() == ok.id()))
                    .cloned()
                    .collect();
                for k in &gone {
                    changes
                        .sentences
                        .push(format!("{} is gone.", capitalize_id(&k.id())));
                }
                if !gone.is_empty() {
                    changes.gone.insert(g.iri().to_string(), gone);
                }
            }
        }
        changes
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        self.sentences.is_empty()
    }

    /// What changed, one sentence each.
    pub fn sentences(&self) -> &[String] {
        &self.sentences
    }

    /// How `key` of `governor` differs from the earlier board, if it does.
    pub(crate) fn of(&self, governor: &str, key: &KeyState) -> Option<&Change> {
        self.keys.get(&(governor.to_string(), key.id()))
    }

    /// The keys of `governor` the earlier board had and this one does not.
    pub(crate) fn gone(&self, governor: &str) -> &[KeyState] {
        self.gone
            .get(governor)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

/// "Circuit urn:demo:down: was closed, now open, 3 consecutive transient failures."
fn change_sentence(key: &KeyState, change: &Change) -> String {
    let now = key_now(key);
    match change {
        Change::New => format!("{} is new: {now}.", capitalize_id(&key.id())),
        Change::Was(old) => match (old.mode(), key.mode()) {
            (Some(a), Some(b)) if a != b => {
                format!("{}: was {}, now {now}.", capitalize_id(&key.id()), a.word())
            }
            _ => format!("{}: now {now}.", capitalize_id(&key.id())),
        },
    }
}

/// A key's state and count, as the change sentence says them.
fn key_now(key: &KeyState) -> String {
    match key {
        KeyState::Window {
            max,
            window_millis,
            hits,
            mode,
            ..
        } => {
            format!(
                "{}, {hits} of {max} per {}",
                mode.word(),
                span(*window_millis)
            )
        }
        KeyState::Circuit { failures, mode, .. } => format!(
            "{}, {failures} consecutive transient failure{}",
            mode.word(),
            if *failures == 1 { "" } else { "s" }
        ),
        KeyState::Bulkhead {
            max,
            running,
            parked,
            mode,
            ..
        } => {
            format!(
                "{}, {running} of {max} running, {parked} parked",
                mode.word()
            )
        }
        KeyState::Route {
            last_candidate,
            mode,
            ..
        } => match last_candidate {
            Some(n) => format!("{}, last call ended at candidate {n}", mode.word()),
            None => mode.word().to_string(),
        },
        KeyState::Other { class } => format!("a {class}"),
    }
}

fn capitalize_id(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
