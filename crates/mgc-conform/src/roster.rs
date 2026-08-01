//! The KNOWN-DEVIATION ROSTER (docs/CONFORMANCE.md): a committed
//! rule list (`conformance/known-deviations.json`) that classifies
//! `verify-deltas` diff rows into NAMED, ledger-tracked families —
//! capture-domain closure gaps (terrain, input latency), registered
//! DEVIATIONS.md behavior, and open port leads — so a triaged take's
//! headline number is the UNEXPLAINED residue, not the gross row
//! count. The goal state on a fully triaged take: unexplained = 0,
//! everything either conforming or matched to a rule.
//!
//! Rules are deliberately SCOPED (take, family, field, onset window,
//! tile rect) and the runner always prints per-rule hit counts — a
//! rule that suddenly matches an order of magnitude more rows is a
//! visible signal, not a silent mask. The FIXTURE suite is untouched:
//! signatures stay raw so drift detection keeps its full resolution;
//! the roster shapes only the verify-deltas report and its CSV.

use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RuleStatus {
    /// A closure limitation of the recording (no terrain channel,
    /// input latency, mid-frame capture) — not a port bug.
    Capture,
    /// Registered intentional port behavior (docs/DEVIATIONS.md).
    Deviation,
    /// A known, ledger-tracked port lead awaiting its fix round.
    Open,
}

impl RuleStatus {
    pub fn tag(self) -> &'static str {
        match self {
            RuleStatus::Capture => "capture",
            RuleStatus::Deviation => "deviation",
            RuleStatus::Open => "open",
        }
    }
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum RowKind {
    Field,
    Missing,
    Extra,
}

#[derive(Deserialize)]
pub struct Rule {
    /// Short kebab-case name, unique; shown in the report + CSV.
    pub id: String,
    pub status: RuleStatus,
    /// One-line provenance — cite the ledger entry it rides.
    #[allow(dead_code)]
    pub note: String,
    /// Recording stems this rule applies to (e.g. "mc2l0"); absent =
    /// every take.
    #[serde(default)]
    pub takes: Option<Vec<String>>,
    /// Row kind; absent = any.
    #[serde(default)]
    pub kind: Option<RowKind>,
    #[serde(default)]
    pub class: Option<u8>,
    #[serde(default)]
    pub model: Option<u8>,
    /// Field name (field rows only; a field-bearing rule never
    /// matches missing/extra rows).
    #[serde(default)]
    pub field: Option<String>,
    /// Pair-tick onset window, inclusive.
    #[serde(default)]
    pub t_min: Option<u64>,
    #[serde(default)]
    pub t_max: Option<u64>,
    /// Tile-space rect [x0, y0, x1, y1] inclusive (CSV coordinates —
    /// world / 256). Rows with no coordinate context never match a
    /// rect-scoped rule.
    #[serde(default)]
    pub rect: Option<[f64; 4]>,
    /// Explicit slot list.
    #[serde(default)]
    pub slots: Option<Vec<u16>>,
}

#[derive(Deserialize)]
pub struct Roster {
    pub rules: Vec<Rule>,
}

/// One diff row's matching context.
pub struct RowCtx<'a> {
    pub kind: RowKind,
    pub slot: Option<u16>,
    pub class: u8,
    pub model: u8,
    /// Field name for field rows.
    pub field: Option<&'a str>,
    /// Tile-space position, when entity context exists.
    pub pos: Option<(f64, f64)>,
}

impl Roster {
    pub fn load(path: &Path) -> Result<Option<Roster>, String> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let r: Roster =
            serde_json::from_str(&raw).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut seen = std::collections::BTreeSet::new();
        for rule in &r.rules {
            if !seen.insert(&rule.id) {
                return Err(format!("duplicate roster rule id `{}`", rule.id));
            }
        }
        Ok(Some(r))
    }

    /// First matching rule's index, or None = unexplained.
    pub fn classify(&self, take: &str, t: u64, row: &RowCtx) -> Option<usize> {
        self.rules.iter().position(|r| {
            if let Some(takes) = &r.takes
                && !takes.iter().any(|s| s == take)
            {
                return false;
            }
            if let Some(k) = r.kind
                && k != row.kind
            {
                return false;
            }
            if let Some(c) = r.class
                && c != row.class
            {
                return false;
            }
            if let Some(m) = r.model
                && m != row.model
            {
                return false;
            }
            if let Some(f) = &r.field {
                match row.field {
                    Some(rf) if rf == f => {}
                    _ => return false,
                }
            }
            if let Some(t0) = r.t_min
                && t < t0
            {
                return false;
            }
            if let Some(t1) = r.t_max
                && t > t1
            {
                return false;
            }
            if let Some(slots) = &r.slots {
                match row.slot {
                    Some(s) if slots.contains(&s) => {}
                    _ => return false,
                }
            }
            if let Some([x0, y0, x1, y1]) = r.rect {
                match row.pos {
                    Some((x, y)) if x >= x0 && x <= x1 && y >= y0 && y <= y1 => {}
                    _ => return false,
                }
            }
            true
        })
    }
}

/// One diff row's classification.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    /// Needs investigation — matched by nothing.
    Unexplained,
    /// Matched roster rule (index into `Roster::rules`).
    Rule(usize),
    /// The row is clean in the port run driven by the OTHER
    /// `--pin-pose` sample. Retail's player pose changes mid-tick at
    /// the carpet's pool slot, so handlers on the two sides of that
    /// slot saw different poses — the once-per-tick capture holds
    /// only one of them, and every pinned run is wrong for one side.
    /// Runner-built (no roster provenance), reported separately.
    PosePhase,
}

impl Tag {
    pub fn known(self) -> bool {
        self != Tag::Unexplained
    }
}

/// Per-row rule tags for one pair, index-aligned with the PairDiff's
/// missing / extra / fields vectors.
#[derive(Default)]
pub struct RuleTags {
    pub missing: Vec<Tag>,
    pub extra: Vec<Tag>,
    pub fields: Vec<Tag>,
}

impl RuleTags {
    pub fn all_known(&self) -> bool {
        self.missing.iter().all(|t| t.known())
            && self.extra.iter().all(|t| t.known())
            && self.fields.iter().all(|t| t.known())
    }
}
