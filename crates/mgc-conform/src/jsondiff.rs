//! Structural JSON diff — path-labelled leaf mismatches between two
//! `serde_json::Value` trees. The conformance reports lean on this for
//! both the decoder self-check and the obs comparators.

use serde_json::Value;

/// One leaf-level difference: `path` in `a.b[3].c` form, and the two
/// sides rendered compactly (missing side = "∅").
#[derive(Debug, Clone)]
pub struct Diff {
    pub path: String,
    pub want: String,
    pub got: String,
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 60 {
        format!("{}…", &s[..60])
    } else {
        s
    }
}

/// Collect leaf differences of `got` vs `want`, depth-first, capped at
/// `cap` entries (0 = unlimited).
pub fn diff(want: &Value, got: &Value, cap: usize) -> Vec<Diff> {
    let mut out = Vec::new();
    walk(want, got, String::new(), &mut out, cap);
    out
}

fn full(out: &[Diff], cap: usize) -> bool {
    cap != 0 && out.len() >= cap
}

fn walk(want: &Value, got: &Value, path: String, out: &mut Vec<Diff>, cap: usize) {
    if full(out, cap) {
        return;
    }
    match (want, got) {
        (Value::Object(w), Value::Object(g)) => {
            for (k, wv) in w {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                match g.get(k) {
                    Some(gv) => walk(wv, gv, p, out, cap),
                    None => out.push(Diff {
                        path: p,
                        want: short(wv),
                        got: "∅".into(),
                    }),
                }
                if full(out, cap) {
                    return;
                }
            }
            for (k, gv) in g {
                if !w.contains_key(k) {
                    let p = if path.is_empty() {
                        k.clone()
                    } else {
                        format!("{path}.{k}")
                    };
                    out.push(Diff {
                        path: p,
                        want: "∅".into(),
                        got: short(gv),
                    });
                    if full(out, cap) {
                        return;
                    }
                }
            }
        }
        (Value::Array(w), Value::Array(g)) => {
            for i in 0..w.len().max(g.len()) {
                let p = format!("{path}[{i}]");
                match (w.get(i), g.get(i)) {
                    (Some(wv), Some(gv)) => walk(wv, gv, p, out, cap),
                    (Some(wv), None) => out.push(Diff {
                        path: p,
                        want: short(wv),
                        got: "∅".into(),
                    }),
                    (None, Some(gv)) => out.push(Diff {
                        path: p,
                        want: "∅".into(),
                        got: short(gv),
                    }),
                    (None, None) => unreachable!(),
                }
                if full(out, cap) {
                    return;
                }
            }
        }
        (w, g) => {
            if w != g {
                out.push(Diff {
                    path,
                    want: short(w),
                    got: short(g),
                });
            }
        }
    }
}
