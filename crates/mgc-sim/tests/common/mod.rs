//! Shared golden-guard for the baked-data self-skips.
//! Include from a test file with `#[path = "common/mod.rs"] mod common;`.

/// Report a baked-data skip. Under `MGC_REQUIRE_GOLDENS=1` a skip is
/// an ERROR — pre-release / fixture-equipped CI runs set it so an
/// absent `baked/` tree FAILS the suite instead of silently passing
/// every golden. The `GOLDEN-SKIP:` prefix is the grep-able skip
/// report (`cargo test 2>&1 | grep -c GOLDEN-SKIP`).
pub fn golden_skip(what: &str) {
    if std::env::var_os("MGC_REQUIRE_GOLDENS").is_some_and(|v| v != "0" && !v.is_empty()) {
        panic!("MGC_REQUIRE_GOLDENS is set, but: {what}");
    }
    eprintln!("GOLDEN-SKIP: {what}");
}
