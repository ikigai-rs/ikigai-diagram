//! Golden files: the same tree renders byte-identical SVG, and that SVG is the one
//! committed under `tests/golden/`.
//!
//! Two layers, each pinned:
//!
//! 1. **The fixtures are real.** `tests/fixtures/*.ttl` must be exactly what core's
//!    `Topology::to_turtle` writes for the trees in `common` — the every-kind tree
//!    from core's builders, the tic-tac-toe tree from a kernel over real spaces —
//!    and must parse back to the same tree. A fixture cannot drift into a graph
//!    core would never write.
//! 2. **The pictures are stable.** Parsing each fixture and rendering it gives the
//!    committed `tests/golden/*.svg` byte for byte.
//!
//! On a mismatch the actual output is written beside the build
//! (`CARGO_TARGET_TMPDIR/golden-actual/`) and the failure names the copy command.
//! An intended change is blessed by running it and committing the result; there is
//! no environment switch that rewrites goldens from inside a test.

mod common;

use common::{committed, every_kind, tic_tac_toe_kernel};
use ikigai_core::Topology;

/// Compare `actual` with the committed `tests/{relative}`, writing the actual out
/// when they differ.
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

#[test]
fn the_every_kind_fixture_is_what_core_writes_for_it() {
    let tree = every_kind();
    matches("fixtures/every-kind.ttl", &tree.to_turtle());
    let text = committed("fixtures/every-kind.ttl").expect("the fixture");
    assert_eq!(Topology::from_turtle(&text).expect("it parses"), tree);
}

#[test]
fn the_tic_tac_toe_fixture_is_a_real_kernels_topology() {
    let tree = tic_tac_toe_kernel().topology();
    matches("fixtures/tic-tac-toe.ttl", &tree.to_turtle());
    let text = committed("fixtures/tic-tac-toe.ttl").expect("the fixture");
    assert_eq!(Topology::from_turtle(&text).expect("it parses"), tree);
}

#[test]
fn the_every_kind_picture_is_stable() {
    let text = committed("fixtures/every-kind.ttl").expect("the fixture");
    let tree = Topology::from_turtle(&text).expect("it parses");
    let svg = ikigai_diagram::render(&tree);
    assert_eq!(svg, ikigai_diagram::render(&tree), "deterministic");
    matches("golden/every-kind.svg", &svg);
}

#[test]
fn the_tic_tac_toe_picture_is_stable() {
    let text = committed("fixtures/tic-tac-toe.ttl").expect("the fixture");
    let tree = Topology::from_turtle(&text).expect("it parses");
    let svg = ikigai_diagram::render(&tree);
    assert_eq!(svg, ikigai_diagram::render(&tree), "deterministic");
    matches("golden/tic-tac-toe.svg", &svg);
}
