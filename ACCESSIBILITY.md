# Accessibility

What `ikigai-diagram` draws that people read, what makes it accessible, how that is tested, what
is not, and how to report a barrier. It is not a conformance claim. Adapted from the
[ACCESSIBILITY.md template](https://github.com/mgifford/ACCESSIBILITY.md) (MIT).

| Field | Value |
|---|---|
| Target | WCAG 2.2 Level AA for the SVG this repository renders |
| Status | Evaluation in progress. No conformance claim. |
| Owner | the maintainers of this repository |
| Report a barrier | [a GitHub issue](https://github.com/ikigai-rs/ikigai-diagram/issues/new) with the `accessibility` label; anything security-sensitive through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-diagram/security/advisories/new) |
| Last reviewed | 2026-10-15 |

## What it emits that people read

One thing: an SVG picture of a kernel's arrangement, from `render` (`src/render.rs`) and the two
endpoints that answer it, `urn:diagram:kernel` and `urn:diagram:arrangement`. A host decides how
the picture reaches a person (ikigai-cli's `show` opens it as a file; the ikigai Book embeds one
as an `<img>` with its own `alt` text), and the host's page is the host's.

## What makes it accessible

- **A text alternative.** The root `<svg>` is `role="img"`, labeled by its `<title>` (the
  arrangement's kind and name) and its `<desc>` (the arrangement in words: what the outermost space
  is, how many of each kind of space, doors and alias rules, where the holes are, and the names its
  spaces claim) through `aria-labelledby`, with ids derived from the picture so two pictures in one
  page do not collide.
- **Real text.** Every label is a `<text>` element, never a path.
- **Not color alone.** Every box carries its kind as a word in its header band (Chain, Fallback,
  Mount, Alias, Limit, Level, ...), and door rows say their match kind in words (`exact`,
  `template`, `prefix`, `custom`); the colors and the dashed (limit) and dotted (opaque) outlines
  repeat what the words say.
- **Contrast in both schemes.** Every color is a token from `src/palette.rs`, light by default and
  dark under `prefers-color-scheme: dark`, and the page is painted, so dark text never lands on a
  dark host page through a transparent background. Text holds 4.5:1 on every surface it sits on
  (SC 1.4.3) and every box outline 3:1 against the page (SC 1.4.11), measured with `ikigai-a11y`.
- **Nothing overlaps or overruns.** Text too long for its column ends in an ellipsis.

## How it is tested

- `tests/contrast.rs` measures every pair the token tables define (text and muted text on the page
  and on every kind's header band, every outline, every match-kind mark) in both schemes, with
  `ikigai-a11y`'s `ratio`, and checks the dark scheme sits behind the media query.
- The unit tests in `src/render.rs` check that no two lines of text overlap and every line stays
  inside its box and the 960-pixel width, on a host-sized arrangement (138 doors), every kind of
  space, and a tree nested 60 deep; and that truncation, wrapping and escaping work.
- The doctest on `render` and `tests/space_name.rs` check `role="img"`, real text and the names in
  the `<desc>`; `tests/golden.rs` holds three pictures byte for byte, so any change to the markup
  shows up as a diff. All ran and passed on main at `43d3e67` on 2026-10-15 (31 tests and one
  doctest; one test that only prints a picture is ignored). CI runs them on every pull request.
- No accessibility checker (axe or similar), no browser and no assistive technology has been used
  on any picture.

## Known limitations

Found by reading the source and the golden files on 2026-10-15, not by testing with people or
assistive technology:

- The `<desc>` summarizes; it does not list doors, patterns or endpoints. With `role="img"` the
  picture's own text is not exposed to a screen reader, so the door rows a sighted reader sees are
  not available to one. The arrangement's Turtle (`urn:kernel:topology`, or the `of` resource)
  holds all of it as text.
- A truncated line's full text is only in a hover tooltip (`<title>`), which a keyboard or touch
  user cannot open and a screen reader does not reach inside `role="img"`.
- The picture is a fixed 960 pixels wide with 13-pixel text. Scaled down to fit a narrow screen,
  its text shrinks with it; at its own size it scrolls sideways.
- The `<svg>` states no language (`xml:lang`), so a standalone file opened in a browser has none.
- That no color pair outside the token tables is drawn is known from the source, not a test.
- Nothing above has been tried with a screen reader, magnifier or braille display.

## Change history

| Date | Change |
|---|---|
| 2026-09-30 | The renderer, with `role="img"`, `<title>`, `<desc>`, real text and the contrast test ([PR 1](https://github.com/ikigai-rs/ikigai-diagram/pull/1)). |
| 2026-10-10 | The `<desc>` states the names the spaces claim ([PR 2](https://github.com/ikigai-rs/ikigai-diagram/pull/2)). |
| 2026-10-15 | This file ([PR 6](https://github.com/ikigai-rs/ikigai-diagram/pull/6)). |

## Contributing

Read this file before changing the renderer or the palette. A new color is a token in both schemes
and passes `tests/contrast.rs`; a new kind of space gets a word in its header band and a phrase in
the `<desc>`; a change to the picture updates the golden files on purpose. Say in the pull request
what was tested, with what, and what was not.

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-diagram/issues/new) and give it the
`accessibility` label (if GitHub does not let you set labels here, put "accessibility" in the
title and a maintainer will add it): what you were doing, what happened, and your viewer and any
assistive technology, if you are comfortable sharing them. Anything security-sensitive goes
through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-diagram/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.
