//! Determinism PROVENANCE — the machine-readable trust-boundary stamp.
//!
//! Tier-B replay is "same-binary / same-machine bit-reproducible" (design §3.4).
//! That guarantee is only meaningful *relative to a fixed set of trust-boundary
//! inputs*: the compiler/toolchain that produced the codegen, the language
//! edition, the pinned RNG family that defines the byte stream, the per-tick
//! state-hash format version, and the build target. A recorded replay (or a
//! pinned golden hash) is valid ONLY under the same values of these inputs.
//!
//! This module is the SINGLE machine-readable source of truth for those inputs,
//! so a replay header (plan-3) can embed the exact environment its hashes were
//! captured under, and so a later toolchain/dep/edition bump is an attributable,
//! reviewed REBASELINE event — never a silent re-capture. It mirrors the golden
//! discipline already in `hash.rs` (`golden_zero_state_hash`) and `rng.rs`
//! (`golden_first_draws_are_pinned`): a pinned value plus a loud drift test.
//!
//! ## What is pinned vs. observed
//!
//! - **Pinned** (`PINNED_RUSTC_CHANNEL`, `EDITION`, `PINNED_RAND_CHACHA`,
//!   `PINNED_RAND_CORE`): declared here AND in the build config files
//!   (`rust-toolchain.toml`, workspace `Cargo.toml`). The tests below read those
//!   files and FAIL if the two ever disagree — that failure is the tripwire that
//!   forces "bumping a trust-boundary input is a deliberate, reviewed rebaseline
//!   that invalidates every replay/golden recorded under the old value."
//! - **Observed** (`TARGET_ARCH`, `TARGET_OS`): compile-time facts about the
//!   build target. They legitimately differ per machine (Tier B is same-machine,
//!   not cross-platform — Tier C is out of scope), so they are recorded in the
//!   stamp but have no "must match" test.
//! - **Live** (`hash_format_version`): sourced directly from
//!   [`crate::hash::HASH_FORMAT_VERSION`], so appending a hashed field (which
//!   bumps that version) flows into the stamp automatically.
//!
//! Determinism floor: every value here is a compile-time `const` (`env::consts`
//! is a const, NOT the banned `env::var` function); the drift tests read the
//! committed config files via `std::fs`, never the process environment. Nothing
//! in this module touches the hashed per-tick path.

/// Pinned rustc channel. MUST equal `rust-toolchain.toml` `[toolchain] channel`.
/// A rustc bump can change FP/codegen and therefore the `to_bits()` hash surface,
/// so it invalidates every replay/golden captured under the old compiler — a
/// reviewed rebaseline, not a silent re-record. (Verified by
/// `pinned_rustc_matches_toolchain_file`.)
pub const PINNED_RUSTC_CHANNEL: &str = "1.95.0";

/// Language edition. MUST equal `Cargo.toml` `[workspace.package] edition`.
/// Editions are syntactic/lint-level (they do not by themselves perturb
/// `to_bits()`), but they are still recorded so a replay's exact build is fully
/// attributable. (Verified by `edition_matches_workspace_manifest`.)
pub const EDITION: &str = "2024";

/// Pinned `rand_chacha`. MUST equal the exact (`=`) pin in the workspace
/// `[workspace.dependencies]`. `ChaCha8Rng`'s byte stream is version-stable only
/// within a pinned family; a bump re-keys every recorded sequence. (Verified by
/// `pinned_rand_versions_match_workspace_manifest`.)
pub const PINNED_RAND_CHACHA: &str = "0.10.0";

/// Pinned `rand_core`. MUST equal the exact (`=`) pin in the workspace
/// `[workspace.dependencies]`. `seed_from_u64`'s u64→seed expansion lives here.
/// (Verified by `pinned_rand_versions_match_workspace_manifest`.)
pub const PINNED_RAND_CORE: &str = "0.10.1";

/// Build target architecture (e.g. `x86_64`). Observed, not pinned: Tier B is
/// same-machine, so this is recorded for attribution, not asserted.
pub const TARGET_ARCH: &str = std::env::consts::ARCH;

/// Build target OS (e.g. `linux`). Observed, not pinned (see [`TARGET_ARCH`]).
pub const TARGET_OS: &str = std::env::consts::OS;

/// The full machine-readable determinism trust-boundary stamp. A replay header
/// (plan-3) embeds one of these so a tier/version mismatch is *detectable* even
/// though Tier B does not *prevent* it (design §3.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provenance {
    /// rustc channel the binary was built with (pinned via `rust-toolchain.toml`).
    pub rustc_channel: &'static str,
    /// Language edition (pinned via workspace `Cargo.toml`).
    pub edition: &'static str,
    /// Pinned `rand_chacha` version (defines the ChaCha8 byte stream).
    pub rand_chacha: &'static str,
    /// Pinned `rand_core` version (defines `seed_from_u64`).
    pub rand_core: &'static str,
    /// Build target architecture (observed).
    pub target_arch: &'static str,
    /// Build target OS (observed).
    pub target_os: &'static str,
    /// Live per-tick state-hash format version (from [`crate::hash`]).
    pub hash_format_version: u32,
}

/// The canonical stamp for THIS build. Embed in replay headers and golden
/// baselines so every recorded hash is attributable to its environment.
pub const PROVENANCE: Provenance = Provenance {
    rustc_channel: PINNED_RUSTC_CHANNEL,
    edition: EDITION,
    rand_chacha: PINNED_RAND_CHACHA,
    rand_core: PINNED_RAND_CORE,
    target_arch: TARGET_ARCH,
    target_os: TARGET_OS,
    hash_format_version: crate::hash::HASH_FORMAT_VERSION,
};

impl Provenance {
    /// The provenance of the current build. Const-folds to [`PROVENANCE`].
    pub fn current() -> Self {
        PROVENANCE
    }

    /// A stable single-line rendering for the replay header / logs. Field order
    /// is fixed; do not reorder (recorded headers parse positionally-by-key).
    pub fn stamp(&self) -> String {
        format!(
            "rustc={} edition={} rand_chacha={} rand_core={} target={}-{} hash_fmt_v={}",
            self.rustc_channel,
            self.edition,
            self.rand_chacha,
            self.rand_core,
            self.target_arch,
            self.target_os,
            self.hash_format_version,
        )
    }
}

impl Default for Provenance {
    fn default() -> Self {
        PROVENANCE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Extract the quoted value of `key = "..."` from TOML-ish `contents`,
    /// matching on the key as the whole left-hand side of an assignment. Returns
    /// the string between the first pair of double quotes on that line.
    fn quoted_value(contents: &str, key: &str) -> Option<String> {
        for line in contents.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('#') {
                continue;
            }
            let (lhs, rhs) = match trimmed.split_once('=') {
                Some(parts) => parts,
                None => continue,
            };
            if lhs.trim() != key {
                continue;
            }
            let start = rhs.find('"')? + 1;
            let end = rhs[start..].find('"')? + start;
            return Some(rhs[start..end].to_string());
        }
        None
    }

    fn read_repo_file(rel: &str) -> String {
        // CARGO_MANIFEST_DIR = <repo>/crates/jumpgate-core ; the config files
        // live at the workspace root two levels up.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
    }

    #[test]
    fn pinned_rustc_matches_toolchain_file() {
        // TRUST-BOUNDARY TRIPWIRE. If rust-toolchain.toml is bumped without
        // updating PINNED_RUSTC_CHANNEL (and consciously rebaselining every
        // golden/replay captured under the old compiler), this fails loudly.
        let toolchain = read_repo_file("rust-toolchain.toml");
        let channel = quoted_value(&toolchain, "channel")
            .expect("rust-toolchain.toml must declare a `channel`");
        assert_eq!(
            channel, PINNED_RUSTC_CHANNEL,
            "rustc trust boundary drifted: rust-toolchain.toml channel = {channel:?} but \
             provenance PINNED_RUSTC_CHANNEL = {PINNED_RUSTC_CHANNEL:?}. A rustc bump can \
             change codegen/FP and invalidates recorded replays — update the const ONLY as \
             part of a deliberate, reviewed rebaseline."
        );
    }

    #[test]
    fn edition_matches_workspace_manifest() {
        let manifest = read_repo_file("Cargo.toml");
        let edition = quoted_value(&manifest, "edition")
            .expect("workspace Cargo.toml must declare an `edition`");
        assert_eq!(
            edition, EDITION,
            "edition trust boundary drifted: Cargo.toml edition = {edition:?} but provenance \
             EDITION = {EDITION:?}."
        );
    }

    #[test]
    fn pinned_rand_versions_match_workspace_manifest() {
        // The workspace pins carry a leading `=` (exact-version requirement);
        // strip it before comparing to the bare provenance version string.
        let manifest = read_repo_file("Cargo.toml");
        let chacha = quoted_value(&manifest, "rand_chacha")
            .expect("workspace Cargo.toml must pin `rand_chacha`");
        let core = quoted_value(&manifest, "rand_core")
            .expect("workspace Cargo.toml must pin `rand_core`");
        assert_eq!(
            chacha.trim_start_matches('='),
            PINNED_RAND_CHACHA,
            "rand_chacha trust boundary drifted: Cargo.toml = {chacha:?} vs provenance \
             {PINNED_RAND_CHACHA:?}. A bump re-keys every recorded ChaCha8 sequence."
        );
        assert_eq!(
            core.trim_start_matches('='),
            PINNED_RAND_CORE,
            "rand_core trust boundary drifted: Cargo.toml = {core:?} vs provenance \
             {PINNED_RAND_CORE:?}."
        );
    }

    #[test]
    fn provenance_carries_live_hash_format_version() {
        // Sourced from hash.rs, so appending a hashed field (which bumps
        // HASH_FORMAT_VERSION) automatically updates the recorded stamp.
        assert_eq!(
            PROVENANCE.hash_format_version,
            crate::hash::HASH_FORMAT_VERSION,
            "provenance must carry the live state-hash format version"
        );
        assert_eq!(Provenance::current(), PROVENANCE);
    }

    #[test]
    fn stamp_is_stable_and_complete() {
        let s = PROVENANCE.stamp();
        for needle in [
            "rustc=1.95.0",
            "edition=2024",
            "rand_chacha=0.10.0",
            "rand_core=0.10.1",
            "hash_fmt_v=6",
        ] {
            assert!(s.contains(needle), "stamp {s:?} missing {needle:?}");
        }
        // Observed target fields are present (values vary per machine).
        assert!(s.contains(&format!("target={TARGET_ARCH}-{TARGET_OS}")));
    }
}
