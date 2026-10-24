# ikigai-diagram

**A kernel's arrangement as a picture.** In ikigai the arrangement a request is
resolved against — its corridors, fallbacks, mounts, aliases, limits, levels and
the doors of every endpoint space — is itself a resource: `urn:kernel:topology`
answers it as Turtle. This module draws that resource. Ask for
`urn:diagram:kernel` and you get the picture of the arrangement *your* request
sees, as SVG; ask for `urn:diagram:arrangement of=<iri>` and you get the picture of
any arrangement — a space declared in a file, a corridor's, a peer's.

Here is a tic-tac-toe game's space — fourteen doors under a fallback, wrapped in
an alias table that names the board's eight lines onto one `cells:{list}`
template:

![The tic-tac-toe arrangement: a chain whose root is an alias of eight exact rules
(rows, columns and diagonals onto urn:ttt:cells:{list}) over a fallback of two
endpoint spaces, thirteen composite doors and the stored cell](tests/golden/tic-tac-toe.svg)

That picture is `tests/golden/tic-tac-toe.svg`, the golden file the tests hold
the renderer to byte for byte, so it cannot drift from what the code draws.

## Calling it

In the REPL, `show` draws the picture: a terminal cannot render SVG, so it writes
the file to the temporary directory, opens it, and prints where it went.

```text
show urn:diagram:kernel                                  the arrangement you are in
show urn:diagram:arrangement urn:file:game-space.ttl     a declared space, as a file
show urn:diagram:arrangement of=urn:kernel:topology      the same as urn:diagram:kernel
```

`source` answers the SVG itself, so a REPL prints all of it (tens of kilobytes for
a real kernel). Use it when you want the bytes, redirected to a file:

```sh
ikigai -c 'source urn:diagram:kernel' > kernel.svg
```

`show` is in ikigai-cli 0.1.43 and later. The opener is the config home's
`show.opener`: the platform's own when unset, `none` to write the file and print
its path without opening it (over ssh, say), or any command line, which gets the
path appended.

| name | argument | needs |
|---|---|---|
| `urn:diagram:kernel` | — | `urn:cap:kernel:inspect`, because `urn:kernel:topology` does |
| `urn:diagram:arrangement` | `of`: the IRI of an arrangement's Turtle (required, so a positional or piped name lands in it) | whatever reading `of` needs, under the caller's own capability |
| `urn:diagram:state` | — | what `urn:diagram:kernel` needs; the state layer shows what the caller may read of `urn:throttle:state` |
| `urn:diagram:state:arrangement` | `of`, as above | what `urn:diagram:arrangement` needs; the same state layer |

All four answer `image/svg+xml`. `of` takes a NAME: piping a Turtle document into it
is refused with a message saying so.

From Rust, the renderer is a pure function over core's typed tree:

```rust
use ikigai_core::Topology;

let tree = Topology::from_turtle(turtle)?;   // ikigai-core, feature `declare`
let svg: String = ikigai_diagram::render(&tree);
```

A host mounts the endpoints with `ikigai_diagram::space()`, which names itself
`urn:iki:space:diagram` (`ikigai_diagram::SPACE_ID`).

## What the picture says

The arrangement is a tree, so the picture is nested boxes in a vertical flow, 960
pixels wide however large the arrangement grows:

- a **chain** lists its corridors innermost first, then the root;
- a **fallback** numbers its layers in the order they are consulted;
- a **mount** is labeled with its prefix, around the space it mounts;
- an **alias** lists its rules in the order they are tried, then what they
  rewrite into;
- a **limit** is a dashed box: a hole over its family;
- a **level** shows its name and its seals;
- an **endpoint space** lists its doors in order — the pattern, the endpoint that
  answers it, how it matches — and under a confined door, the corridor its
  endpoint's sub-requests are severed into;
- an **opaque** space is a dotted box: where the graph's knowledge stops.

A named space shows its IRI; an anonymous one shows no skolem. A named space
reached twice is drawn once, and referenced where it is met again. A module's
configuration-free space names itself `urn:iki:space:<module>`, so the picture of
a host says which modules it holds (`tests/golden/named-module.svg`). Text too long for its column ends in an ellipsis and
is carried whole in a tooltip, and every door row has one.

## Governor state over the arrangement

`urn:diagram:state` draws what ikigai-throttle's governors hold now (a circuit open or
half-open, a rate window's hits, a bulkhead's running and parked work, a failover's last
candidate) inside the box of the space each one governs, and says it in words below the
picture:

![A test host shaped like ikigai-throttle's, with governor state: a rate-limit window full, a
circuit half-open and another closed, a bulkhead full with work parked, and a failover
whose last call failed at its backup candidate, each drawn in its governed space's box,
and the same in words in a summary box below](tests/golden/state-every-state.svg)

It composes two resources, the arrangement and `urn:throttle:state` (the board), and
joins them on `thr:governs`: the IRI a governor names for the space it governs is the IRI
`urn:kernel:topology` gives that node. A failover is opaque in topology, so it is drawn
on whichever of its candidates' spaces the arrangement shows, and stated in words either
way.

- **Your authority shapes the picture.** The board is read under your capability and
  lists only the governors you may read, and only those are drawn. Without a grant under
  `urn:cap:throttle:state:*` you get `urn:diagram:kernel`'s picture byte for byte, with no
  sign that governors exist; the board is not even asked.
- **The plain pictures stay pure.** `urn:diagram:kernel` and `urn:diagram:arrangement`
  never read state, so they stay as cacheable as the arrangement. The state picture is as
  cacheable as the board: cached and redrawn on every state change when the host attaches
  the board to its kernel (`StateBoard::attach`), live on every read when it does not.
- **Never color alone.** Every state is a word, first on its row so a long target cut
  with an ellipsis never cuts it; the words below the picture are a `role="status"`
  region with `aria-live="polite"`.

From Rust: `ikigai_diagram::render_with_state(&tree, &board)`, with
`ikigai_diagram::state::Board::from_turtle(turtle)`. An empty board draws exactly what
`render` draws. This crate reads the board's Turtle and does not depend on
ikigai-throttle; its `thr:` namespace is one constant, `state::THR`.

## Properties it keeps

- **Cacheable, and redrawn when the arrangement changes.** The picture is a pure
  function of what was read, and hangs from it: a rebind cuts
  `urn:kernel:bindings` and redraws `urn:diagram:kernel`; editing a declaration
  file redraws its picture.
- **Not an arrangement is an answer.** A resource that does not declare an
  arrangement is drawn as a picture saying why — cacheable, and redrawn when the
  file is fixed. Failing to *read* `of` is still an error.
- **Deterministic.** The same tree renders the same bytes: integer layout over a
  fixed monospace advance, no floats, no clock.
- **Accessible.** The root `<svg>` is `role="img"` with a `<title>` and a `<desc>`
  that states the arrangement in words, the names its spaces claim included; every label is real `<text>`; every color
  is a token with a dark scheme under `prefers-color-scheme`, and every pair the
  picture draws meets the WCAG floor `ikigai-a11y` enforces (4.5:1 for text, 3:1
  for outlines), checked by `tests/contrast.rs`.
- **No authority of its own.** `of` is read by the kernel under the caller's
  capability, so a caller can draw only what it can already read.
- **wasm-clean.** The library builds for `wasm32-unknown-unknown`, so the diagram
  can render in the browser demo too.

It registers **no transreptor**: a `text/turtle → image/svg+xml` conversion would
be chosen for any Turtle at all, and most Turtle is not an arrangement.

## Accessibility

[ACCESSIBILITY.md](ACCESSIBILITY.md) says what makes the picture accessible, how that is tested,
what is not (the `<desc>` summarizes rather than listing every door, among other things), and how
to report a barrier.

## License

MIT OR Apache-2.0.
