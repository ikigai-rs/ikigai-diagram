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

Both answer `image/svg+xml`. `of` takes a NAME: piping a Turtle document into it
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

## License

MIT OR Apache-2.0.
