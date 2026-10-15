# Working in this repository

`ikigai-diagram` is an ikigai module: `urn:diagram:*` draws a kernel's arrangement as an
accessible SVG. These rules are for every agent and every person working here. `CLAUDE.md` is a
link to this file.

- **Read [ACCESSIBILITY.md](ACCESSIBILITY.md) before any change to the renderer, the palette or
  anything else in the picture.** It says what makes the picture accessible, how that is tested,
  and what is not.
- Security reports go through the organization's security policy (GitHub private vulnerability
  reporting), never a public issue.
- Every change lands through a pull request with green CI: `cargo fmt`, `cargo clippy
  --all-targets -- -D warnings` and `cargo test`, plus the wasm build of the library
  (`.github/workflows/ci.yml`). A change to the picture updates the golden files on purpose.
- Never invent testing, tool results or conformance evidence, and never weaken a test to make a
  change pass.
- Use American spelling in everything you write: code comments, docs, commit messages and pull
  requests.
