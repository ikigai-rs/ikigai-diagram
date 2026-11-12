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
| Last reviewed | 2026-11-12 |

## What it emits that people read

One thing: an SVG picture of a kernel's arrangement, from `render` (`src/render.rs`) and the two
endpoints that answer it, `urn:diagram:kernel` and `urn:diagram:arrangement`; and the same picture
with a state layer (governor state drawn over it, and stated in words below it), from
`render_with_state` and `urn:diagram:state` / `urn:diagram:state:arrangement`. A host decides how
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
  Mount, Dynamic space, Alias, Limit, Level, ...), and door rows say their match kind in words (`exact`,
  `template`, `prefix`, `custom`); the colors and the dashed (limit) and dotted (opaque) outlines
  repeat what the words say.
- **Contrast in both schemes.** Every color is a token from `src/palette.rs`, light by default and
  dark under `prefers-color-scheme: dark`, and the page is painted, so dark text never lands on a
  dark host page through a transparent background. Text holds 4.5:1 on every surface it sits on
  (SC 1.4.3) and every box outline 3:1 against the page (SC 1.4.11), measured with `ikigai-a11y`.
- **A language.** The root `<svg>` states `xml:lang="en"` and `lang="en"`, because the words it
  draws (kinds, match kinds, the `<desc>`) are English; names and patterns from the arrangement
  are drawn as they are.
- **Nothing overlaps or overruns.** Text too long for its column ends in an ellipsis.
- **The state layer, in words and live.** A picture with a state layer is a `role="group"` named
  by its `<title>`. The arrangement inside it is the `role="img"` above; the state is ALSO a
  `role="status"` region with `aria-live="polite"` and `aria-atomic="true"`, drawn as real text
  below the picture, that states every governor, what it governs, whether the picture shows that
  space, and every key's state and numbers: everything the layer draws. The region's id is derived
  from the arrangement alone, so a redraw of the same arrangement with new state keeps it.
- **State is never color alone.** Every state is a word (`OPEN`, `HALF-OPEN`, `CLOSED`, `FULL`,
  `ADMITTING`, `ANSWERED`, `FAILED`), placed first on its row so truncation never removes it; its
  color (go, wait, stop) repeats the word. With `before=` an earlier board, each key that
  changed is marked in words right after its state word (`(was CLOSED)`, `(new)`,
  `(changed)`), and the words below begin with what changed, or say nothing did.
- **The page's own colors.** `scheme=light` or `scheme=dark` draws one scheme with no media
  query, for a host page with a theme of its own; `auto`, the default, follows the viewer.

## How it is tested

Every named check has a result: Pass, Fail or Blocked, with its evidence. A Pass covers only the
stated check, not overall accessibility. Every Pass below was run on main at `b15311e` on
2026-11-12, with rustc and cargo 1.99.0: `cargo test` (67 tests and 7 doctests passed; one test
that only prints a picture is ignored), `cargo fmt --check`, `cargo clippy --all-targets -- -D
warnings` and `cargo clippy --lib --target wasm32-unknown-unknown -- -D warnings`. CI
(`.github/workflows/ci.yml`) runs the same gates on every pull request. These are tests of the
markup and the palette; none of them renders a picture in a browser or puts it in front of
assistive technology.

| Check | Result | Evidence |
|---|---|---|
| Text alternative: `role="img"` with `<title>` and `<desc>`, and the names the spaces claim in the `<desc>` | Pass | `tests/space_name.rs`, the doctest on `render` and `tests/endpoints.rs` (`role="img"`); `tests/golden.rs` holds seven pictures byte for byte, `aria-labelledby` included. Checks the markup, not what a screen reader announces. |
| Ids distinct between two pictures in one page, and the status region's id kept across a redraw | Blocked | No test asserts either; the golden files fix the ids' bytes, not their distinctness or stability. |
| Real text, and every kind and match kind as a word | Pass | `tests/golden.rs` holds the every-kind picture, every word in it, byte for byte; `tests/space_name.rs` checks the header band's text. |
| Text and outline contrast (SC 1.4.3, 1.4.11) in both schemes, state marks and the change marker included | Pass | `tests/contrast.rs`, `ikigai-a11y` 0.6.0's `ratio` and thresholds, over every pair the token tables define. Measures the tables, not a rendered picture or a host page. |
| Dark scheme behind `prefers-color-scheme`, and `scheme=` forcing one scheme | Pass | `tests/contrast.rs` (both media-query tests); `a_picture_takes_the_colors_its_page_asks_for` in `tests/endpoints.rs`. |
| No color outside the tokens, no inline `style`, `fill` and `stroke` keywords only | Pass | `the_picture_draws_no_color_outside_the_token_tables` in `tests/contrast.rs`, over the golden pictures. |
| Language stated | Pass | `the_picture_states_its_language` in `tests/space_name.rs`. |
| No overlap or overrun; truncation, wrapping and escaping | Pass | Unit tests in `src/render.rs`: the host-sized arrangement (138 doors), every kind, a tree 60 deep, the state layer; ellipsis, wrapping, escaping, XML-forbidden characters. Geometry is computed from character counts, not measured in a browser. |
| State as words in a polite, atomic live region that states what the layer draws; changes marked in words | Pass | `tests/state.rs`: `every_state_value_has_a_word`, `the_words_say_everything_the_layer_shows_in_a_live_region`, `before_marks_what_changed_since_an_earlier_board`, `a_hostile_prefix_is_text_never_markup`, two golden state pictures. Checks the markup; no announcement was heard. |
| Live region announces on a redraw in a host | Blocked | No host or screen-reader run; see Known limitations. |
| Browser rendering | Blocked | No browser run for this update. Historical: `tests/golden/state-every-state.svg` was looked at in a browser, light scheme only, on 2026-10-24; browser and version were not recorded, and CI does not repeat it. |
| Manual keyboard | Blocked | No person has performed a keyboard pass. The golden pictures hold no link or `tabindex` (read with `grep` on 2026-11-12), so nothing in them takes focus. |
| Automated accessibility checker (axe or similar) | Blocked | None has been run against any picture; no browser checker is configured. |
| Screen reader | Blocked | No screen reader has been used on any picture; no screen-reader session was available for this update. |
| Screen magnifier | Blocked | No screen magnifier has been used on any picture; no magnifier session was available for this update. |
| Speech input | Blocked | No speech-input software has been used on any picture; no speech-input session was available for this update. |
| Switch access | Blocked | No switch-access software has been used on any picture; no switch-access session was available for this update. |
| Braille display | Blocked | No braille display has been used on any picture; no braille display was available for this update. |

This repository uses none of ikigai-ui's per-kind accessibility criteria
(`urn:iki:ui:criterion:kind:{kind}:target:{target}`): it draws an SVG and has no ikigai-ui
kind, so no SEE or HEAR check of theirs applies to it.

## Known limitations

Found by reading the source and the golden files on 2026-10-15 and 2026-10-24, and read again
against main on 2026-11-12, not by testing with people or assistive technology:

- The `<desc>` summarizes; it does not list doors, patterns or endpoints. With `role="img"` the
  picture's own text is not exposed to a screen reader, so the door rows a sighted reader sees are
  not available to one. The arrangement's Turtle (`urn:kernel:topology`, or the `of` resource)
  holds all of it as text.
- A truncated line's full text is only in a hover tooltip (`<title>`), which a keyboard or touch
  user cannot open and a screen reader does not reach inside `role="img"`.
- **The live region announces only if the host keeps it.** A host that swaps the whole `<svg>`
  on a redraw replaces the region, and screen readers announce a NEW live region unreliably or not
  at all. A host that wants announcements keeps one region of its own and copies the summary's
  text into it, or morphs the SVG in place so the region (same id) persists. This is not tested.
- An `<img>` of a state picture exposes only its host-given `alt`: the live region and the words
  inside reach a screen reader only when the SVG is inline.
- The picture is a fixed 960 pixels wide with 13-pixel text. Scaled down to fit a narrow screen,
  its text shrinks with it; at its own size it scrolls sideways.

## Change history

| Date | Change |
|---|---|
| 2026-09-30 | The renderer, with `role="img"`, `<title>`, `<desc>`, real text and the contrast test ([PR 1](https://github.com/ikigai-rs/ikigai-diagram/pull/1)). |
| 2026-10-10 | The `<desc>` states the names the spaces claim ([PR 2](https://github.com/ikigai-rs/ikigai-diagram/pull/2)). |
| 2026-10-15 | This file ([PR 6](https://github.com/ikigai-rs/ikigai-diagram/pull/6), [PR 7](https://github.com/ikigai-rs/ikigai-diagram/pull/7)). |
| 2026-10-17 | The picture states its language; a test that no color is drawn outside the tokens ([PR 8](https://github.com/ikigai-rs/ikigai-diagram/pull/8)). |
| 2026-10-24 | The state layer: governor state drawn over the arrangement as words, and a live region of text ([PR 13](https://github.com/ikigai-rs/ikigai-diagram/pull/13)). |
| 2026-10-25 | `scheme=` on every picture ([PR 15](https://github.com/ikigai-rs/ikigai-diagram/pull/15)); `before=` marks what changed in words, `focus=` draws one space ([PR 16](https://github.com/ikigai-rs/ikigai-diagram/pull/16)). |
| 2026-11-12 | How it is tested, as a Check, Result and Evidence table: every check Pass, Fail or Blocked, each Pass run on main. |

## Contributing

Read this file before changing the renderer or the palette. A new color is a token in both schemes
and passes `tests/contrast.rs`; a new kind of space gets a word in its header band and a phrase in
the `<desc>`; a change to the picture updates the golden files on purpose. In the pull request's
`## Accessibility` section, give each check a row in a Check | Result | Evidence table: Pass, Fail
or Blocked, with the test or command, the tool and its version, or why it could not run. Keep an
unrun check Blocked, a screen reader nobody used included; never call it "not applicable".

## Reporting a barrier

[Open an issue](https://github.com/ikigai-rs/ikigai-diagram/issues/new) and give it the
`accessibility` label (if GitHub does not let you set labels here, put "accessibility" in the
title and a maintainer will add it): what you were doing, what happened, and your viewer and any
assistive technology, if you are comfortable sharing them. Anything security-sensitive goes
through [private vulnerability reporting](https://github.com/ikigai-rs/ikigai-diagram/security/advisories/new).
**We acknowledge a report within 7 days**, and promise nothing further.
