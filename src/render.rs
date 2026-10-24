//! The pure half: a [`Topology`] in, SVG text out.
//!
//! The arrangement is a TREE, so the picture is nested boxes in a vertical flow —
//! no general graph layout, no crossing edges, nothing to optimize. Every box is a
//! header band (what kind of space, the few facts that kind carries, its IRI when
//! it claims one) over a body (its rows, then the spaces it encloses, indented).
//! The layout is integer arithmetic over a fixed monospace advance, so the same
//! tree is the same bytes on every machine.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ikigai_core::{Door, SpaceKind, Topology, TopologyRule};

use crate::palette::{self, Kind};
use crate::state::{span, Board, Governor, GovernorKind, KeyState, Mode, Tone};

/// The picture's width: the bound the layout fits every box and every line into.
pub const WIDTH: u32 = 960;
/// Space around the outermost box.
const MARGIN: u32 = 16;
/// The advance of one monospace character at the picture's 13px, in pixels. Every
/// common monospace face advances at most 0.602em (Menlo, DejaVu Sans Mono; SF Mono
/// and Liberation Mono 0.60em, Consolas 0.55em) — 7.83px or less — so 8 slightly
/// over-estimates, which is the safe side: text measured wider than it renders
/// cannot overlap its neighbor. An integer advance keeps every coordinate an
/// integer, which is what makes the output byte-stable without float formatting.
const ADVANCE: u32 = 8;
/// One line of text: rows, header lines.
const LINE: u32 = 20;
/// Where a line's baseline sits below the line's top.
const BASELINE: u32 = 14;
/// Inner padding of a box.
const PAD: u32 = 8;
/// How far an enclosed space is indented inside its parent.
const INDENT: u32 = 16;
/// Vertical space between sibling boxes.
const GAP: u32 = 8;
/// Below this width a nested box stops indenting and takes its parent's width, so a
/// very deep tree degrades to aligned boxes rather than to boxes of no width.
const MIN_BOX: u32 = 240;
/// The ellipsis a truncated line ends with; the full text is in a `<title>`.
const ELLIPSIS: char = '…';

/// Render an arrangement as a standalone SVG document.
///
/// Deterministic: the same tree renders byte-identical SVG. Accessible: the root
/// `<svg>` is `role="img"` with a `<title>` and a `<desc>` that states the
/// arrangement in words (ending with the names its spaces claim, in the order they
/// are drawn, when any below the outermost claims one), every label is real
/// `<text>`, and every color is a token from [`palette`] (light, and dark under
/// `prefers-color-scheme`).
///
/// ```
/// use ikigai_core::{Door, MatchKind, SpaceKind, Topology};
///
/// let leaf = Topology::new(SpaceKind::EndpointSpace {
///     doors: vec![Door::new("urn:hello", MatchKind::Exact, "hello")],
/// });
/// let svg = ikigai_diagram::render(&leaf);
/// assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
/// assert!(svg.contains("role=\"img\""));
/// assert!(svg.contains(">urn:hello</tspan></text>")); // every label is real text
/// assert_eq!(svg, ikigai_diagram::render(&leaf)); // the same bytes, every time
///
/// // A named space shows its name in its header band, and the desc states it.
/// let named = leaf.clone().with_id(Some(ikigai_core::space_iri("hello")));
/// let host = Topology::new(SpaceKind::Fallback).child(named);
/// let svg = ikigai_diagram::render(&host);
/// assert!(svg.contains("<tspan class=\"ikd-m\">urn:iki:space:hello</tspan></text>"));
/// assert!(svg.contains("Named spaces, in the order drawn: urn:iki:space:hello.</desc>"));
/// ```
pub fn render(topology: &Topology) -> String {
    Layout::of(topology).svg()
}

/// Render an arrangement with a STATE LAYER: what the governors on `board` hold,
/// drawn over the spaces they govern, and the same in words below the picture.
///
/// The join is `thr:governs`: a governor's governed space is the node whose IRI it
/// names, so its state is drawn in that node's box, under the header band. A
/// failover is opaque in topology and governs nothing itself; each of its candidates
/// governs a space and is drawn there when the arrangement shows it. A governor whose
/// space the arrangement does not show is still stated in the words.
///
/// Every state is a WORD (`OPEN`, `FULL`, `HALF-OPEN`, ...) placed first on its row,
/// so a long target cut with an ellipsis never cuts the state; the color repeats the
/// word. The words below the picture are a `role="status"` region with
/// `aria-live="polite"`, and say everything the layer shows.
///
/// **An empty board draws the plain arrangement**, byte for byte what [`render`]
/// draws: a caller the board shows no governor gets no sign that governors exist.
///
/// Pure and deterministic, like [`render`]: the state is an input, never fetched.
///
/// ```
/// use ikigai_core::{Door, MatchKind, SpaceKind, Topology};
/// use ikigai_diagram::state::Board;
///
/// let space = Topology::new(SpaceKind::EndpointSpace {
///     doors: vec![Door::new("urn:demo:down", MatchKind::Exact, "down")],
/// })
/// .with_id(Some(ikigai_core::space_iri("demo")));
/// let board = Board::from_turtle(&[
///     "@prefix thr: <https://ikigai-rs.dev/ns/throttle#> .",
///     "@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .",
///     "<urn:throttle:state> a thr:StateBoard ; thr:governor <urn:throttle:state:b> .",
///     "<urn:throttle:state:b> a thr:CircuitBreaker ; thr:name \"b\" ;",
///     "    thr:governs <urn:iki:space:demo> ; thr:key <urn:throttle:state:b:key:urn:demo:down> .",
///     "<urn:throttle:state:b:key:urn:demo:down> a thr:Circuit ; thr:target <urn:demo:down> ;",
///     "    thr:failures \"2\"^^xsd:nonNegativeInteger ; thr:state thr:Open .",
/// ].join("\n")).unwrap();
///
/// let svg = ikigai_diagram::render_with_state(&space, &board);
/// assert!(svg.contains(">OPEN</tspan>")); // the state is a word on the governed box
/// assert!(svg.contains("role=\"status\" aria-live=\"polite\""));
/// assert!(svg.contains("Circuit urn:demo:down: open, 2 consecutive transient failures."));
///
/// // No governors on the board: the plain picture, with no hint of the layer.
/// assert_eq!(ikigai_diagram::render_with_state(&space, &Board::empty()), ikigai_diagram::render(&space));
/// ```
pub fn render_with_state(topology: &Topology, board: &Board) -> String {
    if board.is_empty() {
        return render(topology);
    }
    Layout::with_state(topology, board).svg()
}

/// A laid-out picture: the body, its height, and what the prose summary needs.
pub(crate) struct Layout {
    body: String,
    height: u32,
    title: String,
    desc: String,
    /// The state layer's words, when the picture has one: the summary box's SVG, and
    /// the id the picture's ids derive from (the PLAIN arrangement's, so a redraw of the
    /// same arrangement with new state keeps the status region's id).
    status: Option<Status>,
    /// Every line of text placed, for the legibility tests. Recorded in every build,
    /// not only under `cfg(test)`, so the layout the tests check is the one that
    /// ships; only the tests read it, hence the allow outside them.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) texts: Vec<Placed>,
}

/// One line of text as the layout placed it: its left edge, top, measured width,
/// and the right edge of the box it must stay inside.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(not(test), allow(dead_code))] // read only by the legibility tests (above)
pub(crate) struct Placed {
    pub(crate) x: u32,
    pub(crate) top: u32,
    pub(crate) width: u32,
    pub(crate) limit: u32,
}

/// The state layer's summary, laid out: its box and its ids' seed.
pub(crate) struct Status {
    svg: String,
    id: String,
}

impl Layout {
    pub(crate) fn of(topology: &Topology) -> Layout {
        let mut canvas = Canvas::default();
        let frag = canvas.node(topology, MARGIN, MARGIN, WIDTH - 2 * MARGIN, None);
        let height = frag.height + 2 * MARGIN;
        Layout {
            body: frag.svg,
            height,
            title: title(topology),
            desc: canvas.counts.describe(topology),
            status: None,
            texts: canvas.texts,
        }
    }

    /// The arrangement with the board's state drawn over it, and the summary below.
    pub(crate) fn with_state(topology: &Topology, board: &Board) -> Layout {
        let plain = Layout::of(topology);
        let mut canvas = Canvas {
            overlay: Overlay::of(board),
            ..Canvas::default()
        };
        let w = WIDTH - 2 * MARGIN;
        let frag = canvas.node(topology, MARGIN, MARGIN, w, None);
        let top = MARGIN + frag.height + GAP;
        let summary = canvas.summary(board, MARGIN, top, w);
        let mut desc = canvas.counts.describe(topology);
        desc.push_str(&format!(
            " Over it, the state of {}, drawn in the box of each space governed and stated in \
             words below the picture.",
            plural(board.governors().len(), "governor", "governors")
        ));
        Layout {
            body: frag.svg,
            height: top + summary.height + MARGIN,
            title: format!("{}, with governor state", title(topology)),
            desc,
            status: Some(Status {
                svg: summary.svg,
                id: format!("ikds-{:016x}", fnv1a(plain.body.as_bytes())),
            }),
            texts: canvas.texts,
        }
    }

    pub(crate) fn svg(&self) -> String {
        if let Some(status) = &self.status {
            return self.svg_with_state(status);
        }
        let id = format!("ikd-{:016x}", fnv1a(self.body.as_bytes()));
        let mut out = String::with_capacity(self.body.len() + 4096);
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xml:lang=\"en\" lang=\"en\" width=\"{WIDTH}\" height=\"{h}\" \
             viewBox=\"0 0 {WIDTH} {h}\" role=\"img\" aria-labelledby=\"{id}-title {id}-desc\" \
             class=\"ikd\">\n<title id=\"{id}-title\">{title}</title>\n\
             <desc id=\"{id}-desc\">{desc}</desc>\n<style>{style}</style>\n\
             <rect class=\"ikd-bg\" width=\"{WIDTH}\" height=\"{h}\"/>\n",
            h = self.height,
            title = xml(&self.title),
            desc = xml(&self.desc),
            style = palette::stylesheet(),
        );
        out.push_str(&self.body);
        out.push_str("</svg>\n");
        out
    }

    /// A picture with a state layer: a `role="group"` named by its title, holding the
    /// arrangement as a `role="img"` (its own text is drawn, not read) and the words as
    /// a `role="status"` live region of real text a screen reader reaches.
    fn svg_with_state(&self, status: &Status) -> String {
        let id = &status.id;
        let mut out = String::with_capacity(self.body.len() + status.svg.len() + 4096);
        let _ = write!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" xml:lang=\"en\" lang=\"en\" width=\"{WIDTH}\" height=\"{h}\" \
             viewBox=\"0 0 {WIDTH} {h}\" role=\"group\" aria-labelledby=\"{id}-title\" \
             class=\"ikd\">\n<title id=\"{id}-title\">{title}</title>\n\
             <desc id=\"{id}-desc\">{desc}</desc>\n<style>{style}</style>\n\
             <rect class=\"ikd-bg\" width=\"{WIDTH}\" height=\"{h}\"/>\n\
             <g role=\"img\" aria-labelledby=\"{id}-title {id}-desc\">\n",
            h = self.height,
            title = xml(&self.title),
            desc = xml(&self.desc),
            style = palette::stylesheet_with_state(),
        );
        out.push_str(&self.body);
        let _ = write!(
            out,
            "</g>\n<g id=\"{id}-status\" role=\"status\" aria-live=\"polite\" aria-atomic=\"true\">\n"
        );
        out.push_str(&status.svg);
        out.push_str("</g>\n</svg>\n");
        out
    }
}

/// The picture of a resource that does not declare an arrangement: `subject` and
/// why, in a box of its own. A picture rather than an error, because "this file is
/// not an arrangement" is a legitimate answer about the file, and an answer is
/// cacheable where a refusal is not: it hangs from the thread of what was read, so
/// fixing the file redraws the picture.
pub(crate) fn problem(subject: &str, reason: &str) -> String {
    let mut canvas = Canvas::default();
    let (x, y, w) = (MARGIN, MARGIN, WIDTH - 2 * MARGIN);
    let mut svg = String::new();
    let head = Head {
        kind: Kind::Problem,
        tag: None,
        label: "Not an arrangement",
        detail: String::new(),
        iri: Some(subject.to_string()),
    };
    let mut cursor = canvas.header(&mut svg, x, y, w, head) + PAD / 2;
    for line in wrap(&clean(reason), chars(w - 2 * PAD)) {
        canvas.text(
            &mut svg,
            x + PAD,
            y + cursor,
            w - 2 * PAD,
            &[("ikd-x", line)],
        );
        cursor += LINE;
    }
    let frag = boxed(svg, x, y, w, cursor + PAD / 2, Kind::Problem);
    Layout {
        body: frag.svg,
        height: frag.height + 2 * MARGIN,
        title: format!("Not an arrangement: {subject}"),
        desc: format!("{subject} does not declare an arrangement: {reason}"),
        status: None,
        texts: canvas.texts,
    }
    .svg()
}

/// Break text into lines of at most `budget` characters, at spaces where it can and
/// mid-word where a word alone is longer than a line.
fn wrap(text: &str, budget: usize) -> Vec<String> {
    let budget = budget.max(1);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let used = line.chars().count();
            let gap = usize::from(used > 0);
            if used + gap + word.len() <= budget {
                if gap == 1 {
                    line.push(' ');
                }
                line.extend(word.iter());
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            let rest = word.split_off(budget);
            lines.push(word.iter().collect());
            word = rest;
        }
    }
    if !line.is_empty() || lines.is_empty() {
        lines.push(line);
    }
    lines
}

/// A drawn box: its height and its SVG, positioned absolutely.
struct Frag {
    height: u32,
    svg: String,
}

/// The layout's running state.
#[derive(Default)]
struct Canvas {
    /// Named nodes already drawn. A named space reached twice (a shared space under
    /// two mounts) is drawn where it is first met and referenced after — the rule
    /// `Topology::to_turtle` follows for the same reason: a name is a claim, and the
    /// second occurrence has nothing to add.
    seen: BTreeSet<String>,
    counts: Counts,
    texts: Vec<Placed>,
    /// The state layer: empty for a plain picture.
    overlay: Overlay,
}

/// What the state layer draws where: every governed space's governors, by the
/// space's IRI (`thr:governs`), and which governors were drawn on a box.
#[derive(Default)]
struct Overlay {
    on: BTreeMap<String, Vec<Badge>>,
    /// The governed spaces whose boxes the layer was drawn in.
    drawn: BTreeSet<String>,
}

/// One governor's mark on a space: it governs the space, or the space is one of a
/// failover's candidates.
#[derive(Clone)]
enum Badge {
    Governs(Governor),
    Candidate { failover: Governor, order: u64 },
}

impl Overlay {
    fn of(board: &Board) -> Overlay {
        let mut on: BTreeMap<String, Vec<Badge>> = BTreeMap::new();
        for g in board.governors() {
            if let Some(space) = g.governs() {
                on.entry(space.to_string())
                    .or_default()
                    .push(Badge::Governs(g.clone()));
            }
            if let GovernorKind::Failover { candidates } = g.kind() {
                for c in candidates {
                    if let Some(space) = &c.governs {
                        on.entry(space.clone()).or_default().push(Badge::Candidate {
                            failover: g.clone(),
                            order: c.order,
                        });
                    }
                }
            }
        }
        Overlay {
            on,
            drawn: BTreeSet::new(),
        }
    }
}

/// A styled run of text: the classes it carries and the text itself.
type Run = (&'static str, String);

/// What a box's header band says: the kind, what the parent calls it, the facts the
/// kind carries, and the node's IRI when it claims one.
struct Head {
    kind: Kind,
    tag: Option<String>,
    label: &'static str,
    detail: String,
    iri: Option<String>,
}

impl Canvas {
    /// Lay out one node at `(x, y)` in a box `w` wide. `tag` is what the parent
    /// calls it (`layer 2`, `root`, `confined to`).
    fn node(&mut self, t: &Topology, x: u32, y: u32, w: u32, tag: Option<String>) -> Frag {
        let (kind, label, detail) = describe(t);
        let iri = t.id.as_ref().map(|i| i.as_str().to_string());

        // A named node already drawn: a compact reference, not a second copy.
        if let Some(name) = &iri {
            if !self.seen.insert(name.clone()) {
                let mut svg = String::new();
                let head = Head {
                    kind,
                    tag,
                    label,
                    detail: String::new(),
                    iri: iri.clone(),
                };
                let mut cursor = self.header(&mut svg, x, y, w, head) + PAD / 2;
                self.text(
                    &mut svg,
                    x + PAD,
                    y + cursor,
                    w - 2 * PAD,
                    &[("ikd-m", "drawn above: the same space, reached again".into())],
                );
                cursor += LINE + PAD / 2;
                return boxed(svg, x, y, w, cursor, kind);
            }
        }
        self.counts.count(t);

        let mut svg = String::new();
        let governed = iri.clone();
        let head = Head {
            kind,
            tag,
            label,
            detail,
            iri,
        };
        let mut cursor = self.header(&mut svg, x, y, w, head) + PAD / 2;

        // The state layer: what governs this space, under its header band.
        if let Some(space) = &governed {
            cursor = self.badges(&mut svg, space, x, y + cursor, w) - y;
        }

        // Rows the kind carries.
        let note = |text: &str| vec![("ikd-m", text.to_string())];
        match &t.kind {
            SpaceKind::EndpointSpace { doors } => {
                cursor = self.doors(&mut svg, doors, x, y + cursor, w) - y;
            }
            SpaceKind::Alias { rules, .. } => {
                cursor = self.rules(&mut svg, rules, x, y + cursor, w) - y;
            }
            SpaceKind::Level { seals, namespace } => {
                let listed = if seals.is_empty() {
                    "none".to_string()
                } else {
                    seals.join("  ")
                };
                let row = [("ikd-m", "seals ".to_string()), ("ikd-x", listed)];
                self.text(&mut svg, x + PAD, y + cursor, w - 2 * PAD, &row);
                cursor += LINE;
                if let Some(namespace) = namespace {
                    let row = [
                        ("ikd-m", "namespace ".to_string()),
                        ("ikd-x", namespace.clone()),
                    ];
                    self.text(&mut svg, x + PAD, y + cursor, w - 2 * PAD, &row);
                    cursor += LINE;
                }
            }
            SpaceKind::Limit { .. } => {
                let row = note("a name in this family resolves as if nothing were bound there");
                self.text(&mut svg, x + PAD, y + cursor, w - 2 * PAD, &row);
                cursor += LINE;
            }
            SpaceKind::Opaque => {
                let row = note("reports no structure: what the graph knows stops here");
                self.text(&mut svg, x + PAD, y + cursor, w - 2 * PAD, &row);
                cursor += LINE;
            }
            _ => {}
        }

        // The spaces it encloses, in the order they are consulted.
        let (cx, cw) = inset(x, w);
        let tags = child_tags(t);
        for (child, tag) in t.children.iter().zip(tags) {
            cursor += GAP / 2;
            let frag = self.node(child, cx, y + cursor, cw, tag);
            svg.push_str(&frag.svg);
            cursor += frag.height + GAP / 2;
        }
        boxed(svg, x, y, w, cursor + PAD / 2, kind)
    }

    /// Draw the header band — one line, or two when the node is named — and return
    /// its height.
    fn header(&mut self, svg: &mut String, x: u32, y: u32, w: u32, head: Head) -> u32 {
        let mut runs: Vec<Run> = Vec::new();
        if let Some(tag) = head.tag {
            runs.push(("ikd-m", format!("{tag} · ")));
        }
        runs.push(("ikd-x ikd-b", head.label.to_string()));
        if !head.detail.is_empty() {
            runs.push(("ikd-x", format!("  {}", head.detail)));
        }
        let lines = if head.iri.is_some() { 2 } else { 1 };
        let height = lines * LINE + PAD;
        // The band goes under the text: written first.
        let _ = writeln!(
            svg,
            "<rect class=\"ikd-t-{c}\" x=\"{bx}\" y=\"{by}\" width=\"{bw}\" height=\"{bh}\" rx=\"3\"/>",
            c = head.kind.class(),
            bx = x + 1,
            by = y + 1,
            bw = w - 2,
            bh = height - 2,
        );
        self.text(svg, x + PAD, y + PAD / 2, w - 2 * PAD, &runs);
        if let Some(iri) = head.iri {
            self.text(
                svg,
                x + PAD,
                y + PAD / 2 + LINE,
                w - 2 * PAD,
                &[("ikd-m", iri)],
            );
        }
        height
    }

    /// Door rows: number, pattern, the endpoint that answers it, the match kind —
    /// and under a confined door, the corridor it severs into.
    fn doors(&mut self, svg: &mut String, doors: &[Door], x: u32, mut y: u32, w: u32) -> u32 {
        if doors.is_empty() {
            self.text(
                svg,
                x + PAD,
                y,
                w - 2 * PAD,
                &[("ikd-m", "no doors".into())],
            );
            return y + LINE;
        }
        let left = x + PAD;
        let columns = chars(w - 2 * PAD);
        let number = digits(doors.len()) + 1;
        let mark = "template".len(); // the longest match-kind keyword
        let rest = columns.saturating_sub(number + ARROW_COLUMNS + 1 + mark);
        // Columns fit their content, up to a share of the row: the arrow sits right
        // after the longest pattern rather than at a fixed fraction of a wide box.
        let pattern = widest(doors.iter().map(|d| &d.pattern)).min(rest * 3 / 5);
        let endpoint = widest(doors.iter().map(|d| &d.endpoint)).min(rest - pattern);
        let col = |c: usize| left + advance(c);
        for (i, door) in doors.iter().enumerate() {
            let n = i + 1;
            let keyword = door.kind.keyword();
            let mut row = String::new();
            self.cell(&mut row, col(0), y, number, ("ikd-m", n.to_string()));
            self.cell(
                &mut row,
                col(number),
                y,
                pattern,
                ("ikd-x", door.pattern.clone()),
            );
            self.cell(
                &mut row,
                col(number + pattern + 1),
                y,
                1,
                ("ikd-m", ARROW.into()),
            );
            self.cell(
                &mut row,
                col(number + pattern + ARROW_COLUMNS),
                y,
                endpoint,
                ("ikd-x ikd-b", door.endpoint.clone()),
            );
            self.cell(
                &mut row,
                col(number + pattern + ARROW_COLUMNS + endpoint + 1),
                y,
                mark,
                (mark_class(keyword), keyword.to_string()),
            );
            let mut title = format!(
                "door {n}: {} ({keyword}) is answered by {}",
                door.pattern, door.endpoint
            );
            if let Some(corridor) = &door.confined {
                let name = corridor
                    .id
                    .as_ref()
                    .map(|i| i.as_str().to_string())
                    .unwrap_or_else(|| "an anonymous corridor".into());
                let _ = write!(title, ", which confines its sub-requests to {name}");
            }
            let _ = write!(svg, "<g><title>{}</title>\n{row}</g>\n", xml(&title));
            y += LINE;
            if let Some(corridor) = &door.confined {
                // One indent deeper than the enclosed spaces, so the corridor reads as
                // belonging to the door above it rather than to the space.
                let (cx, cw) = inset(x, w);
                let (cx, cw) = inset(cx, cw);
                y += GAP / 2;
                let frag = self.node(corridor, cx, y, cw, Some("confined to".into()));
                svg.push_str(&frag.svg);
                y += frag.height + GAP / 2;
            }
        }
        y
    }

    /// Alias rules, in table order: number, kind, logical → canonical.
    fn rules(
        &mut self,
        svg: &mut String,
        rules: &[TopologyRule],
        x: u32,
        mut y: u32,
        w: u32,
    ) -> u32 {
        let left = x + PAD;
        let columns = chars(w - 2 * PAD);
        let number = digits(rules.len()) + 1;
        let kind = "prefix ".len(); // the longest rule-kind keyword, and a space
        let rest = columns.saturating_sub(number + kind + ARROW_COLUMNS);
        let from = widest(rules.iter().map(|r| &r.from)).min(rest / 2);
        let to = rest - from;
        let col = |c: usize| left + advance(c);
        for (i, rule) in rules.iter().enumerate() {
            let n = i + 1;
            let keyword = rule.kind.keyword();
            let mut row = String::new();
            self.cell(&mut row, col(0), y, number, ("ikd-m", n.to_string()));
            self.cell(
                &mut row,
                col(number),
                y,
                kind,
                (mark_class(keyword), keyword.to_string()),
            );
            self.cell(
                &mut row,
                col(number + kind),
                y,
                from,
                ("ikd-x", rule.from.clone()),
            );
            self.cell(
                &mut row,
                col(number + kind + from + 1),
                y,
                1,
                ("ikd-m", ARROW.into()),
            );
            self.cell(
                &mut row,
                col(number + kind + from + ARROW_COLUMNS),
                y,
                to,
                ("ikd-x", rule.to.clone()),
            );
            let title = format!(
                "rule {n} ({keyword}): {} rewrites to {}",
                rule.from, rule.to
            );
            let _ = write!(svg, "<g><title>{}</title>\n{row}</g>\n", xml(&title));
            y += LINE;
        }
        y
    }

    /// The governors over `space`, each a band naming it and a row per key, state
    /// word first. Returns the next free line.
    fn badges(&mut self, svg: &mut String, space: &str, x: u32, mut y: u32, w: u32) -> u32 {
        let Some(badges) = self.overlay.on.get(space).cloned() else {
            return y;
        };
        self.overlay.drawn.insert(space.to_string());
        for badge in badges {
            let (head, rows) = match &badge {
                Badge::Governs(g) => {
                    let mut head = vec![
                        ("ikd-m", "governed by ".to_string()),
                        ("ikd-x ikd-b", g.kind_word().to_string()),
                        ("ikd-x", format!(" {}", g.name())),
                    ];
                    if let Some(facts) = facts(g) {
                        head.push(("ikd-m", format!(" · {facts}")));
                    }
                    let rows: Vec<Vec<Run>> = if g.keys().is_empty() {
                        vec![vec![("ikd-m", nothing_yet(g).to_string())]]
                    } else {
                        g.keys().iter().map(key_runs).collect()
                    };
                    (head, rows)
                }
                Badge::Candidate { failover, order } => {
                    let head = vec![
                        ("ikd-m", "candidate ".to_string()),
                        ("ikd-x ikd-b", order.to_string()),
                        ("ikd-m", " of failover ".to_string()),
                        ("ikd-x", failover.name().to_string()),
                    ];
                    let mut rows: Vec<Vec<Run>> = failover
                        .keys()
                        .iter()
                        .filter_map(|k| match k {
                            KeyState::Route {
                                target,
                                last_candidate: Some(n),
                                mode,
                            } if n == order => Some(vec![
                                mark(mode),
                                ("ikd-m", " route ".to_string()),
                                ("ikd-x", target.clone()),
                                ("ikd-m", " · its last call ended here".to_string()),
                            ]),
                            _ => None,
                        })
                        .collect();
                    if rows.is_empty() {
                        rows.push(vec![("ikd-m", "no route's last call ended here".into())]);
                    }
                    (head, rows)
                }
            };
            // The band goes under the governor's line: written first.
            let _ = writeln!(
                svg,
                "<rect class=\"ikd-t-state\" x=\"{bx}\" y=\"{y}\" width=\"{bw}\" height=\"{LINE}\" rx=\"2\"/>",
                bx = x + PAD / 2,
                bw = w - PAD,
            );
            self.text(svg, x + PAD, y, w - 2 * PAD, &head);
            y += LINE;
            for row in rows {
                self.text(svg, x + PAD + INDENT, y, w - 2 * PAD - INDENT, &row);
                y += LINE;
            }
            y += PAD / 2;
        }
        y
    }

    /// The summary box: the state in words, every governor and key the layer shows,
    /// and where each governor is drawn. Its lines are real text in the status region.
    fn summary(&mut self, board: &Board, x: u32, y: u32, w: u32) -> Frag {
        let mut svg = String::new();
        let head = Head {
            kind: Kind::State,
            tag: None,
            label: "Governor state",
            detail: plural(board.governors().len(), "governor", "governors"),
            iri: None,
        };
        let mut cursor = self.header(&mut svg, x, y, w, head) + PAD / 2;
        let budget = chars(w - 2 * PAD);
        for g in board.governors() {
            let mut paragraphs = vec![(true, governor_sentence(g, &self.overlay.drawn))];
            if g.keys().is_empty() {
                paragraphs.push((false, format!("{}.", capitalize(nothing_yet(g)))));
            }
            for k in g.keys() {
                paragraphs.push((false, key_sentence(k)));
            }
            for (first, text) in paragraphs {
                let indent = if first { 0 } else { INDENT };
                let lines = wrap(&clean(&text), budget - chars(indent));
                for line in lines {
                    let class = if first { "ikd-x ikd-b" } else { "ikd-x" };
                    self.text(
                        &mut svg,
                        x + PAD + indent,
                        y + cursor,
                        w - 2 * PAD - indent,
                        &[(class, line)],
                    );
                    cursor += LINE;
                }
            }
            cursor += PAD / 2;
        }
        boxed(svg, x, y, w, cursor + PAD / 2, Kind::State)
    }

    /// One column of a row: a single run fitted to `width` characters.
    fn cell(&mut self, svg: &mut String, x: u32, top: u32, width: usize, run: Run) {
        self.text(svg, x, top, advance(width), &[run]);
    }

    /// Place one line of runs at `(x, top)`, fitted to `width` pixels: truncated
    /// with an ellipsis when it does not fit, and then carrying its full text in a
    /// `<title>`.
    fn text(&mut self, svg: &mut String, x: u32, top: u32, width: u32, runs: &[Run]) {
        let full: String = runs.iter().map(|(_, s)| clean(s)).collect();
        let (fitted, truncated) = fit(runs, chars(width));
        let used: usize = fitted.iter().map(|(_, s)| s.chars().count()).sum();
        self.texts.push(Placed {
            x,
            top,
            width: advance(used),
            limit: x + width,
        });
        let mut line = format!("<text x=\"{x}\" y=\"{}\">", top + BASELINE);
        for (class, s) in &fitted {
            let _ = write!(line, "<tspan class=\"{class}\">{}</tspan>", xml(s));
        }
        line.push_str("</text>");
        if truncated {
            let _ = writeln!(svg, "<g><title>{}</title>{line}</g>", xml(&full));
        } else {
            let _ = writeln!(svg, "{line}");
        }
    }
}

/// A state as a mark: the word, upper-case and bold, in its tone's color.
fn mark(mode: &Mode) -> Run {
    let class = match mode.tone() {
        Tone::Go => "ikd-st-go ikd-b",
        Tone::Wait => "ikd-st-wait ikd-b",
        Tone::Stop => "ikd-st-stop ikd-b",
        Tone::Unknown => "ikd-st-unknown ikd-b",
    };
    (class, mode.word().to_uppercase())
}

/// A key's row on its governed box: the state word first, so a cut never takes it.
fn key_runs(key: &KeyState) -> Vec<Run> {
    match key {
        KeyState::Window {
            prefix,
            max,
            window_millis,
            hits,
            mode,
        } => vec![
            mark(mode),
            ("ikd-m", " window ".into()),
            ("ikd-x", prefix.clone()),
            ("ikd-m", " · ".into()),
            (
                "ikd-x",
                format!("{hits} of {max} per {}", span(*window_millis)),
            ),
        ],
        KeyState::Circuit {
            target,
            failures,
            mode,
        } => vec![
            mark(mode),
            ("ikd-m", " circuit ".into()),
            ("ikd-x", target.clone()),
            ("ikd-m", " · ".into()),
            ("ikd-x", failures_phrase(*failures)),
        ],
        KeyState::Bulkhead {
            prefix,
            max,
            running,
            parked,
            mode,
        } => vec![
            mark(mode),
            ("ikd-m", " bulkhead ".into()),
            ("ikd-x", prefix.clone()),
            ("ikd-m", " · ".into()),
            (
                "ikd-x",
                format!("{running} of {max} running, {parked} parked"),
            ),
        ],
        KeyState::Route {
            target,
            last_candidate,
            mode,
        } => vec![
            mark(mode),
            ("ikd-m", " route ".into()),
            ("ikd-x", target.clone()),
            ("ikd-m", " · ".into()),
            ("ikd-x", route_phrase(*last_candidate)),
        ],
        KeyState::Other { class } => {
            vec![("ikd-m", format!("a {class} this renderer does not know"))]
        }
    }
}

/// A key in words, for the summary.
fn key_sentence(key: &KeyState) -> String {
    match key {
        KeyState::Window {
            prefix,
            max,
            window_millis,
            hits,
            mode,
        } => format!(
            "Window {prefix}: {}, {hits} of {max} per {}.",
            mode.word(),
            span(*window_millis)
        ),
        KeyState::Circuit {
            target,
            failures,
            mode,
        } => format!(
            "Circuit {target}: {}, {}.",
            mode.word(),
            failures_phrase(*failures)
        ),
        KeyState::Bulkhead {
            prefix,
            max,
            running,
            parked,
            mode,
        } => format!(
            "Bulkhead {prefix}: {}, {running} of {max} running, {parked} parked.",
            mode.word()
        ),
        KeyState::Route {
            target,
            last_candidate,
            mode,
        } => format!(
            "Route {target}: {}, {}.",
            mode.word(),
            route_phrase(*last_candidate)
        ),
        KeyState::Other { class } => format!("A {class} this renderer does not know."),
    }
}

fn failures_phrase(failures: u64) -> String {
    format!(
        "{failures} consecutive transient failure{}",
        if failures == 1 { "" } else { "s" }
    )
}

fn route_phrase(last: Option<u64>) -> String {
    match last {
        Some(n) => format!("last call ended at candidate {n}"),
        None => "no last candidate named".into(),
    }
}

/// The facts a governor's kind carries, for its band.
fn facts(g: &Governor) -> Option<String> {
    match g.kind() {
        GovernorKind::CircuitBreaker {
            threshold,
            cooldown_millis,
        } => {
            let mut parts = Vec::new();
            if let Some(t) = threshold {
                parts.push(format!("trips after {t}"));
            }
            if let Some(c) = cooldown_millis {
                parts.push(format!("cooldown {}", span(*c)));
            }
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

/// What a governor with no keys has to say.
fn nothing_yet(g: &Governor) -> &'static str {
    match g.kind() {
        GovernorKind::CircuitBreaker { .. } => "no target has failed: every circuit is closed",
        GovernorKind::Failover { .. } => "no call has been routed yet",
        _ => "no key is limited",
    }
}

/// A governor in words: what it is, what it governs, and whether the picture shows it.
fn governor_sentence(g: &Governor, drawn: &BTreeSet<String>) -> String {
    let shown = |space: &str| {
        if drawn.contains(space) {
            "drawn above"
        } else {
            "not in this arrangement"
        }
    };
    let mut out = format!("{}, a {}", g.name(), g.kind_word());
    match g.kind() {
        GovernorKind::Failover { candidates } => {
            let _ = write!(
                out,
                " over {}",
                plural(candidates.len(), "candidate", "candidates")
            );
            let cells: Vec<String> = candidates
                .iter()
                .map(|c| match &c.governs {
                    Some(space) => {
                        format!("candidate {} governs {space} ({})", c.order, shown(space))
                    }
                    None => format!("candidate {} governs an anonymous space", c.order),
                })
                .collect();
            if !cells.is_empty() {
                let _ = write!(out, ": {}", cells.join("; "));
            }
        }
        _ => match g.governs() {
            Some(space) => {
                let _ = write!(out, ", governs {space} ({})", shown(space));
            }
            None => out.push_str(", governs an anonymous space, which the picture cannot place"),
        },
    }
    if let Some(facts) = facts(g) {
        let _ = write!(out, "; {facts}");
    }
    out.push('.');
    out
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The arrow between a pattern and its endpoint, or a rule's two sides: one
/// character, centered in a column of three. Placed by position rather than padded
/// with spaces, because SVG collapses a text node's leading and trailing whitespace.
const ARROW: &str = "→";
const ARROW_COLUMNS: usize = 3;

/// Wrap a body in its box: a page-colored fill under everything, and the kind's
/// outline written LAST so it sits over the edge of the header band. A limit's
/// outline is dashed (a hole), an opaque space's dotted (where knowledge stops).
fn boxed(body: String, x: u32, y: u32, w: u32, h: u32, kind: Kind) -> Frag {
    let pattern = match kind {
        Kind::Limit => " ikd-hole",
        Kind::Opaque => " ikd-dim",
        _ => "",
    };
    let mut svg = format!(
        "<g>\n<rect class=\"ikd-box\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" rx=\"4\"/>\n"
    );
    svg.push_str(&body);
    let _ = writeln!(
        svg,
        "<rect class=\"ikd-k-{c}{pattern}\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" \
         rx=\"4\" fill=\"none\" stroke-width=\"1.5\"/>\n</g>",
        c = kind.class(),
    );
    Frag { height: h, svg }
}

/// The kind, its label, and the one-line detail for a node's header.
fn describe(t: &Topology) -> (Kind, &'static str, String) {
    match &t.kind {
        SpaceKind::Chain { severed } => (
            Kind::Chain,
            "Chain",
            if *severed {
                "the caller's view: corridors innermost first; severed from the root".into()
            } else {
                "the caller's view: corridors innermost first, then the root".into()
            },
        ),
        SpaceKind::Fallback => (
            Kind::Fallback,
            "Fallback",
            format!(
                "{} consulted in order; the first hit wins",
                plural(t.children.len(), "layer", "layers")
            ),
        ),
        SpaceKind::Mount { prefix } => (Kind::Mount, "Mount", prefix.clone()),
        SpaceKind::Dynamic { declaration } => (
            Kind::Dynamic,
            "Dynamic space",
            declaration.as_str().to_string(),
        ),
        SpaceKind::Alias { rules, max_hops } => (
            Kind::Alias,
            "Alias",
            format!(
                "{} tried in order; at most {}",
                plural(rules.len(), "rule", "rules"),
                plural(*max_hops, "hop", "hops")
            ),
        ),
        SpaceKind::Limit { family, kind } => (
            Kind::Limit,
            "Limit",
            format!("a hole over {family} ({})", kind.keyword()),
        ),
        SpaceKind::Level { seals, .. } => (
            Kind::Level,
            "Level",
            format!(
                "{}; endpoints under it resolve their sub-requests here first",
                plural(seals.len(), "seal", "seals")
            ),
        ),
        SpaceKind::EndpointSpace { doors } => (
            Kind::Endpoints,
            "Endpoints",
            format!(
                "{} tried in order; the first match wins",
                plural(doors.len(), "door", "doors")
            ),
        ),
        SpaceKind::Opaque => (Kind::Opaque, "Opaque", String::new()),
        SpaceKind::Rewrite => (
            Kind::Rewrite,
            "Rewrite",
            "a closure: its rules are code, not data".into(),
        ),
        SpaceKind::Confine => (
            Kind::Confine,
            "Confine",
            "sub-requests are severed into this corridor".into(),
        ),
        _ => (
            Kind::Unknown,
            "Unknown kind",
            "a kind of space newer than this renderer".into(),
        ),
    }
}

/// What a parent calls each child: a chain's corridors and root, a fallback's
/// numbered layers.
fn child_tags(t: &Topology) -> Vec<Option<String>> {
    let n = t.children.len();
    match &t.kind {
        SpaceKind::Chain { severed } => (0..n)
            .map(|i| {
                let corridors = if *severed { n } else { n.saturating_sub(1) };
                Some(if i >= corridors {
                    "root".to_string()
                } else if i == 0 && corridors > 1 {
                    "corridor 1, innermost".to_string()
                } else {
                    format!("corridor {}", i + 1)
                })
            })
            .collect(),
        SpaceKind::Fallback => (0..n).map(|i| Some(format!("layer {}", i + 1))).collect(),
        _ => vec![None; n],
    }
}

/// The picture's `<title>`.
fn title(t: &Topology) -> String {
    let what = match &t.kind {
        SpaceKind::Chain { .. } => "Resolution arrangement",
        _ => "Space arrangement",
    };
    match &t.id {
        Some(id) => format!("{what}: {}", id.as_str()),
        None => what.to_string(),
    }
}

/// Running totals for the `<desc>`.
#[derive(Default)]
struct Counts {
    chains: usize,
    fallbacks: usize,
    mounts: usize,
    dynamics: usize,
    aliases: usize,
    rules: usize,
    limits: usize,
    levels: usize,
    spaces: usize,
    doors: usize,
    confined: usize,
    opaque: usize,
    rewrites: usize,
    confines: usize,
    unknown: usize,
    holes: Vec<String>,
    /// Every name a node claims, in the order the nodes are first drawn (a name
    /// reached again is drawn once, so it is listed once). The header band shows
    /// each one; the `<desc>` states them too, so the picture's text alternative
    /// says which spaces it shows, not only how many.
    names: Vec<String>,
}

impl Counts {
    fn count(&mut self, t: &Topology) {
        if let Some(id) = &t.id {
            self.names.push(id.as_str().to_string());
        }
        match &t.kind {
            SpaceKind::Chain { .. } => self.chains += 1,
            SpaceKind::Fallback => self.fallbacks += 1,
            SpaceKind::Mount { .. } => self.mounts += 1,
            SpaceKind::Dynamic { .. } => self.dynamics += 1,
            SpaceKind::Alias { rules, .. } => {
                self.aliases += 1;
                self.rules += rules.len();
            }
            SpaceKind::Limit { family, kind } => {
                self.limits += 1;
                self.holes.push(format!("{family} ({})", kind.keyword()));
            }
            SpaceKind::Level { .. } => self.levels += 1,
            SpaceKind::EndpointSpace { doors } => {
                self.spaces += 1;
                self.doors += doors.len();
                self.confined += doors.iter().filter(|d| d.confined.is_some()).count();
            }
            SpaceKind::Opaque => self.opaque += 1,
            SpaceKind::Rewrite => self.rewrites += 1,
            SpaceKind::Confine => self.confines += 1,
            _ => self.unknown += 1,
        }
    }

    /// The arrangement in words: what the entry is, what is in it, and where the
    /// holes are.
    fn describe(&self, root: &Topology) -> String {
        let n = root.children.len();
        let mut out = match &root.kind {
            SpaceKind::Chain { severed: false } if n <= 1 => {
                "The resolution chain one request sees: the root alone.".to_string()
            }
            SpaceKind::Chain { severed: false } => format!(
                "The resolution chain one request sees: {}, innermost first, then the root.",
                plural(n - 1, "corridor", "corridors")
            ),
            SpaceKind::Chain { severed: true } => format!(
                "The resolution chain one request sees: {}, severed from the root.",
                plural(n, "corridor", "corridors")
            ),
            _ => {
                let (_, label, _) = describe(root);
                format!(
                    "An arrangement whose outermost space is {} {}.",
                    article(label),
                    label.to_lowercase()
                )
            }
        };
        let mut parts: Vec<String> = Vec::new();
        let mut add = |count: usize, one: &str, many: &str| {
            if count > 0 {
                parts.push(plural(count, one, many));
            }
        };
        add(self.fallbacks, "fallback", "fallbacks");
        add(self.mounts, "mount", "mounts");
        add(self.dynamics, "dynamic space", "dynamic spaces");
        add(self.aliases, "alias", "aliases");
        add(self.levels, "level", "levels");
        add(self.limits, "limit", "limits");
        add(self.confines, "confine", "confines");
        add(self.rewrites, "rewrite", "rewrites");
        add(self.opaque, "opaque space", "opaque spaces");
        add(
            self.unknown,
            "space of an unknown kind",
            "spaces of unknown kinds",
        );
        let mut sentence = String::from(" In all: ");
        if !parts.is_empty() {
            sentence.push_str(&parts.join(", "));
            sentence.push_str("; and ");
        }
        let _ = write!(
            sentence,
            "{} across {}",
            plural(self.doors, "door", "doors"),
            plural(self.spaces, "endpoint space", "endpoint spaces")
        );
        if self.rules > 0 {
            let _ = write!(
                sentence,
                ", with {}",
                plural(self.rules, "alias rule", "alias rules")
            );
        }
        if self.confined > 0 {
            let _ = write!(
                sentence,
                "; {} its endpoint's sub-requests",
                if self.confined == 1 {
                    "1 door confines".to_string()
                } else {
                    format!("{} doors confine", self.confined)
                }
            );
        }
        sentence.push('.');
        out.push_str(&sentence);
        if !self.holes.is_empty() {
            let _ = write!(
                out,
                " Holes, where a name resolves as unbound: {}.",
                self.holes.join(", ")
            );
        }
        // The outermost node's name is already the `<title>`'s, so it is not repeated.
        let root_name = root.id.as_ref().map(|i| i.as_str());
        let names: Vec<&str> = self
            .names
            .iter()
            .map(String::as_str)
            .filter(|n| Some(*n) != root_name)
            .collect();
        if !names.is_empty() {
            let _ = write!(
                out,
                " Named spaces, in the order drawn: {}.",
                names.join(", ")
            );
        }
        out
    }
}

/// The box a child is drawn in: indented inside its parent, unless that would make
/// it narrower than [`MIN_BOX`].
fn inset(x: u32, w: u32) -> (u32, u32) {
    if w >= MIN_BOX + INDENT + PAD {
        (x + INDENT, w - INDENT - PAD)
    } else {
        (x, w)
    }
}

/// How many characters fit in `px` pixels.
fn chars(px: u32) -> usize {
    (px / ADVANCE) as usize
}

/// How many pixels `n` characters take.
fn advance(n: usize) -> u32 {
    n as u32 * ADVANCE
}

/// Fit runs into `budget` characters: whole runs while they fit, the first run
/// that does not is cut and ends in an ellipsis, and the rest are dropped.
fn fit(runs: &[Run], budget: usize) -> (Vec<Run>, bool) {
    let mut left = budget;
    let mut out = Vec::new();
    for (class, s) in runs {
        let s = clean(s);
        let n = s.chars().count();
        if n <= left {
            left -= n;
            out.push((*class, s));
            continue;
        }
        if left > 0 {
            let mut cut: String = s.chars().take(left - 1).collect();
            cut.push(ELLIPSIS);
            out.push((*class, cut));
        }
        return (out, true);
    }
    (out, false)
}

/// Replace what XML 1.0 cannot carry at all (C0 controls, even escaped) with
/// U+FFFD, so a pattern with a newline in it is still one well-formed line.
fn clean(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { '\u{fffd}' } else { c })
        .collect()
}

/// Escape text for an XML text node or attribute: [`clean`] first, then the ecosystem's one
/// escaper. `clean` stays a pre-pass because the shared escaper keeps tab, line feed and
/// carriage return (right for HTML), while here a pattern with a newline in it must show as
/// one visible U+FFFD, not as a break a `<title>` tooltip would honor and the column count
/// of [`wrap`] would not see.
fn xml(s: &str) -> String {
    ikigai_fn::html::html_text(&clean(s))
}

/// The class a match-kind or rule-kind keyword is marked with.
fn mark_class(keyword: &str) -> &'static str {
    match keyword {
        "template" => "ikd-mk-template",
        "prefix" => "ikd-mk-prefix",
        "custom" => "ikd-mk-custom",
        _ => "ikd-mk-exact",
    }
}

/// The longest of some texts, in characters (after [`clean`], which keeps the count).
fn widest<'a>(texts: impl Iterator<Item = &'a String>) -> usize {
    texts.map(|t| t.chars().count()).max().unwrap_or(0)
}

fn digits(n: usize) -> usize {
    n.to_string().len()
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn article(word: &str) -> &'static str {
    match word.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// FNV-1a, 64-bit: a stable, dependency-free hash for the ids that tie the root's
/// `aria-labelledby` to its `<title>` and `<desc>`. Derived from the body, so two
/// different pictures inlined in one page do not share ids, and the same picture is
/// still the same bytes.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use ikigai_core::{Iri, MatchKind, RuleKind};

    /// A host-sized arrangement, shaped like the ikigai host's own (measured
    /// 2026-09-30: an alias of four prefix rules over the root, 27 endpoint spaces,
    /// 138 doors, 7 opaque remote mounts): 27 modules of five or six doors under
    /// mounts and a nested fallback, with the long templates the browse family has.
    pub(crate) fn host_sized() -> Topology {
        const MODULES: [&str; 27] = [
            "fs", "http", "text", "rdf", "sparql", "jsonld", "xslt", "shacl", "sexpr", "compress",
            "markdown", "lisp", "ledger", "store", "personal", "org", "meeting", "llm", "secret",
            "sign", "encrypt", "throttle", "a11y", "log", "repo", "browse", "name",
        ];
        let mut doors_left = 138;
        let mut spaces = Vec::new();
        for (i, m) in MODULES.iter().enumerate() {
            let n = if i < 3 { 6 } else { 5 }.min(doors_left);
            doors_left -= n;
            let doors = (0..n)
                .map(|d| match d {
                    0 => Door::new(
                        format!("urn:{m}:list"),
                        MatchKind::Exact,
                        format!("{m}-list"),
                    ),
                    1 => Door::new(
                        format!("urn:{m}:item:{{id}}"),
                        MatchKind::Template,
                        format!("{m}-item"),
                    ),
                    2 => Door::new(
                        format!("urn:iki:{m}:pr:{{repo}}:{{number}}:explain:{{version}}:{{model}}"),
                        MatchKind::Template,
                        format!("{m}-pr-explain-versioned-with-model"),
                    ),
                    3 => Door::new(
                        format!("urn:{m}:config"),
                        MatchKind::Exact,
                        format!("{m}-config"),
                    ),
                    4 => Door::new(
                        format!("urn:{m}:"),
                        MatchKind::Custom,
                        format!("{m}-grammar"),
                    ),
                    _ => Door::new(
                        format!("urn:{m}:extra:{d}"),
                        MatchKind::Exact,
                        format!("{m}-extra"),
                    ),
                })
                .collect();
            let leaf = Topology::new(SpaceKind::EndpointSpace { doors });
            spaces.push(if i % 3 == 0 {
                Topology::new(SpaceKind::Mount {
                    prefix: format!("urn:{m}:"),
                })
                .child(leaf)
            } else {
                leaf
            });
        }
        assert_eq!(doors_left, 0, "138 doors placed");
        let mut root = Topology::new(SpaceKind::Fallback);
        let inner: Vec<Topology> = spaces.split_off(20);
        for space in spaces {
            root = root.child(space);
        }
        let mut nested = Topology::new(SpaceKind::Fallback);
        for space in inner {
            nested = nested.child(space);
        }
        root = root.child(nested);
        for p in 0..7 {
            let peer = Iri::parse(format!("urn:ikigai:peer:mount:{p}")).ok();
            root = root.child(
                Topology::new(SpaceKind::Mount {
                    prefix: format!("urn:iki:peer{p}:"),
                })
                .child(Topology::opaque(peer)),
            );
        }
        let alias = Topology::new(SpaceKind::Alias {
            rules: vec![
                TopologyRule::new(RuleKind::Prefix, "urn:fn:", "urn:iki:fn:"),
                TopologyRule::new(RuleKind::Prefix, "urn:ledger:", "urn:iki:ledger:"),
                TopologyRule::new(RuleKind::Prefix, "urn:store:", "urn:iki:store:"),
                TopologyRule::new(RuleKind::Prefix, "urn:annotation:", "urn:iki:annotation:"),
            ],
            max_hops: 8,
        })
        .child(root);
        Topology::new(SpaceKind::Chain { severed: false })
            .with_id(Iri::parse("urn:ikigai:chain:root").ok())
            .child(alias)
    }

    /// No two lines of text overlap, and every line stays inside its box and the
    /// picture's width bound.
    fn assert_legible(tree: &Topology) {
        let layout = Layout::of(tree);
        assert!(layout.texts.len() > 2, "the check saw the picture's text");
        for (i, a) in layout.texts.iter().enumerate() {
            assert!(a.x + a.width <= a.limit, "text overruns its box: {a:?}");
            assert!(
                a.limit <= WIDTH - MARGIN,
                "a box overruns the width bound: {a:?}"
            );
            for b in &layout.texts[i + 1..] {
                let same_line = a.top.abs_diff(b.top) < LINE;
                let apart = a.x + a.width <= b.x || b.x + b.width <= a.x;
                assert!(!same_line || apart, "overlapping text: {a:?} and {b:?}");
            }
        }
    }

    #[test]
    fn the_host_sized_arrangement_is_legible() {
        let tree = host_sized();
        assert_legible(&tree);
        let svg = render(&tree);
        assert!(
            svg.contains("138 doors across 27 endpoint spaces"),
            "the desc counts them"
        );
        // Vertical flow: one row per door, plus the boxes — a tall page, never a wide one.
        let layout = Layout::of(&tree);
        assert!(layout.height > 138 * LINE, "{}", layout.height);
        assert!(layout.height < 138 * LINE * 3, "{}", layout.height);
    }

    #[test]
    fn the_every_kind_fixture_is_legible() {
        let turtle = include_str!("../tests/fixtures/every-kind.ttl");
        assert_legible(&Topology::from_turtle(turtle).expect("the fixture parses"));
    }

    #[test]
    fn dynamic_declarations_are_legible_and_long_iris_keep_the_full_tooltip() {
        let declaration = format!("urn:example:declaration:{}", "long".repeat(80));
        let tree = Topology::new(SpaceKind::Dynamic {
            declaration: Iri::parse(&declaration).expect("a declaration IRI"),
        })
        .child(Topology::new(SpaceKind::Opaque));
        assert_legible(&tree);
        let svg = render(&tree);
        assert!(svg.contains(&format!("<title>Dynamic space  {declaration}</title>")));
        assert!(svg.contains('…'));
    }

    /// The state layer keeps the floor: no overlap, nothing past its box, every state
    /// word visible on its row (first, so a cut cannot take it), with every state value,
    /// a target long enough to be cut, and the host-sized arrangement under it.
    #[test]
    fn the_state_layer_is_legible() {
        let governed = Topology::from_turtle(include_str!("../tests/fixtures/state/governed.ttl"))
            .expect("the fixture parses");
        for board in [
            include_str!("../tests/fixtures/state/board.ttl").to_string(),
            include_str!("../tests/fixtures/state/every-state.ttl").to_string(),
            include_str!("../tests/fixtures/state/every-state.ttl").replace(
                "urn:demo:breaker:flaky>",
                &format!("urn:demo:breaker:{}>", "flaky".repeat(40)),
            ),
        ] {
            let board = Board::from_turtle(&board).expect("the board parses");
            let layout = Layout::with_state(&governed, &board);
            assert!(layout.texts.len() > 2);
            for (i, a) in layout.texts.iter().enumerate() {
                assert!(a.x + a.width <= a.limit, "text overruns its box: {a:?}");
                assert!(a.limit <= WIDTH - MARGIN, "past the width bound: {a:?}");
                for b in &layout.texts[i + 1..] {
                    let same_line = a.top.abs_diff(b.top) < LINE;
                    let apart = a.x + a.width <= b.x || b.x + b.width <= a.x;
                    assert!(!same_line || apart, "overlapping text: {a:?} and {b:?}");
                }
            }
            let svg = layout.svg();
            assert!(svg.contains(">HALF-OPEN</tspan>") || svg.contains(">OPEN</tspan>"));
        }
        // Over the host-sized arrangement, nothing governs a named space: the words
        // still say every governor, and say it is not in this arrangement.
        let board = Board::from_turtle(include_str!("../tests/fixtures/state/board.ttl"))
            .expect("the board parses");
        let svg = render_with_state(&host_sized(), &board);
        assert!(svg.contains(">Governor state</tspan>"), "{svg}");
        assert!(svg.contains("not in this arrangement"), "{svg}");
        assert!(!svg.contains("drawn above"), "{svg}");
    }

    #[test]
    fn a_tree_deeper_than_the_width_allows_stays_inside_the_bound() {
        let mut tree = Topology::new(SpaceKind::EndpointSpace {
            doors: vec![Door::new("urn:deep:{x}", MatchKind::Template, "deep")],
        });
        for i in 0..60 {
            tree = Topology::new(SpaceKind::Mount {
                prefix: format!("urn:deep:{i}:"),
            })
            .child(tree);
        }
        assert_legible(&tree);
    }

    /// For a human: `cargo test print_host_sized -- --ignored --nocapture > host.svg`
    /// (then drop the test harness's lines around it) to look at the host-sized case.
    #[test]
    #[ignore = "prints a picture for a human to look at; asserts nothing"]
    fn print_host_sized() {
        println!("{}", render(&host_sized()));
    }

    #[test]
    fn a_line_that_does_not_fit_ends_in_an_ellipsis() {
        let runs = [("a", "abcdef".to_string()), ("b", "ghij".to_string())];
        assert_eq!(fit(&runs, 10), (runs.to_vec(), false));
        let (cut, truncated) = fit(&runs, 8);
        assert!(truncated);
        assert_eq!(cut, vec![("a", "abcdef".into()), ("b", "g…".into())]);
        let (cut, _) = fit(&runs, 3);
        assert_eq!(cut, vec![("a", "ab…".into())]);
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_inside_long_words() {
        assert_eq!(wrap("ab cd ef", 5), vec!["ab cd", "ef"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert_eq!(wrap("", 4), vec![""]);
    }

    #[test]
    fn text_is_escaped_and_controls_are_replaced() {
        // Moved from the hand escaper; `'` is now the shared `&#39;` (was `&apos;`, equally
        // valid XML), and `clean` still turns the newline into U+FFFD.
        assert_eq!(xml("a<b>&\"c'\n"), "a&lt;b&gt;&amp;&quot;c&#39;\u{fffd}");
    }

    /// The shared escaper and `xml` differ in exactly two ways, over the ikigai-fn vectors'
    /// escape inputs and this crate's own: `'` is `&#39;` not `&apos;`, and `xml` also
    /// replaces tab, LF, CR, DEL and C1 controls (via `clean`). Nothing else.
    #[test]
    fn xml_is_the_shared_escaper_after_clean() {
        use ikigai_fn::html::{attr, html_text};
        let inputs = [
            "",
            "plain text",
            "&",
            "<",
            ">",
            "\"",
            "'",
            "<script>alert('x')</script>",
            "Tom & Jerry's \"show\"",
            "&amp; is already an entity",
            "&#39;",
            "tab\tline\nreturn\r",
            "nul\u{0}soh\u{1}esc\u{1b}us\u{1f}",
            "del\u{7f} stays",
            "caf\u{e9} \u{2713} \u{65e5}\u{672c} \u{1f600}",
            "nbsp\u{a0}and line sep\u{2028}",
            "$h{urn:example:x}",
            "a=b c`d",
            "it's \"quoted\" & <tagged>",
            "two\nlines",
            "a\tb",
            "a\r\nb",
            "c1\u{85}\u{9f}x",
            "\u{fffd} already",
        ];
        for i in inputs {
            assert_eq!(attr(i), html_text(i));
            let ours = xml(i);
            assert_eq!(ours, html_text(&clean(i)), "{i:?}");
            let shared = html_text(i);
            if ours != shared {
                // Only a control character that `clean` replaces may explain it.
                assert!(
                    i.chars().any(char::is_control),
                    "{i:?}: {ours:?} vs {shared:?}"
                );
                assert!(!ours.contains(|c: char| c.is_control()), "{ours:?}");
            }
            assert!(!ours.contains("&apos;"), "{ours:?}");
        }
    }

    #[test]
    fn widths_and_character_counts_agree() {
        assert_eq!(advance(10), 80);
        assert_eq!(chars(87), 10);
        assert_eq!(advance(chars(WIDTH)), WIDTH);
    }
}
