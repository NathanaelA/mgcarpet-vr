//! Community/mod overlay: loose replacement files under
//! `gamedata/overlay/`, consumed at bake time. The overlay may be a
//! git checkout (the "community mod" distribution), so repository
//! furniture — dotfiles, `README*`, `LICENSE*` — is expected and
//! silently ignored; everything else the bake cannot apply draws a
//! warning so it can never be silently inert. Normative spec:
//! docs/MODDING.md; user-facing contract: gamedata/README.md.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Game-tag directories, matching the baked tree's layout.
const GAME_TAGS: [&str; 3] = ["mc1", "mc1hw", "mc2"];
/// Implemented per-game categories.
const CATEGORIES: [&str; 1] = ["levels"];

/// One replacement level payload, `<overlay>/<tag>/levels/LEVnnnnn.DAT`
/// — the decompressed archive member that replaces member `index`.
#[derive(Debug)]
pub struct OverlayLevel {
    /// Archive member index, from the file name.
    pub index: u32,
    /// Absolute path, for reading the payload.
    pub path: PathBuf,
    /// Overlay-relative path (`mc1/levels/LEV00032.DAT`) — the
    /// provenance string stamped into `meta.overlay`.
    pub rel: String,
}

pub struct Overlay {
    root: PathBuf,
}

impl Overlay {
    /// The overlay root next to the game installs
    /// (`<gamedata>/overlay/`), when present. Warns about unknown
    /// top-level entries and unknown categories by name.
    pub fn locate(gamedata: &Path) -> Option<Overlay> {
        let root = gamedata.join("overlay");
        if !root.is_dir() {
            return None;
        }
        for name in dir_names(&root) {
            if furniture(&name) {
                continue;
            }
            if !GAME_TAGS.contains(&name.as_str()) {
                eprintln!("warning: overlay/{name}: not a game dir ({GAME_TAGS:?}) — ignored");
                continue;
            }
            for cat in dir_names(&root.join(&name)) {
                if !furniture(&cat) && !CATEGORIES.contains(&cat.as_str()) {
                    eprintln!(
                        "warning: overlay/{name}/{cat}: unknown category \
                         (this build supports {CATEGORIES:?}) — ignored"
                    );
                }
            }
        }
        Some(Overlay { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The replacement levels for one game tag, sorted by member
    /// index. Two files targeting the same member (case variants) are
    /// an error — the bake refuses ambiguous input rather than picking
    /// one. Non-matching files draw a warning (catches `LEV0032.DAT`
    /// near-misses).
    pub fn levels(&self, tag: &str) -> Result<Vec<OverlayLevel>, String> {
        let dir = self.root.join(tag).join("levels");
        let mut by_index: BTreeMap<u32, OverlayLevel> = BTreeMap::new();
        for name in dir_names(&dir) {
            if furniture(&name) {
                continue;
            }
            let Some(index) = level_index(&name) else {
                eprintln!(
                    "warning: overlay/{tag}/levels/{name}: not LEVnnnnn.DAT \
                     (LEV + 5-digit member index + .DAT) — ignored"
                );
                continue;
            };
            let level = OverlayLevel {
                index,
                path: dir.join(&name),
                rel: format!("{tag}/levels/{name}"),
            };
            if let Some(prev) = by_index.insert(index, level) {
                return Err(format!(
                    "overlay/{tag}/levels: {} and {name} both target member {index}",
                    prev.rel.rsplit('/').next().unwrap_or(&prev.rel),
                ));
            }
        }
        Ok(by_index.into_values().collect())
    }
}

/// `LEVnnnnn.DAT` (case-insensitive) → member index.
fn level_index(name: &str) -> Option<u32> {
    let upper = name.to_ascii_uppercase();
    let digits = upper.strip_prefix("LEV")?.strip_suffix(".DAT")?;
    if digits.len() != 5 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

/// Repository furniture, expected in a git-checkout overlay.
fn furniture(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    name.starts_with('.') || upper.starts_with("README") || upper.starts_with("LICENSE")
}

/// Entry names of `dir`, sorted; empty when absent/unreadable.
fn dir_names(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mgc-overlay-test-{}-{tag}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        dir
    }

    #[test]
    fn locates_and_enumerates() {
        let gamedata = scratch("enum");
        let levels = gamedata.join("overlay/mc1/levels");
        std::fs::create_dir_all(&levels).unwrap();
        std::fs::write(levels.join("LEV00032.DAT"), b"x").unwrap();
        std::fs::write(levels.join("lev00002.dat"), b"y").unwrap();
        std::fs::write(levels.join("README.md"), b"doc").unwrap();
        std::fs::write(levels.join("LEV032.DAT"), b"near-miss").unwrap();
        std::fs::write(gamedata.join("overlay/LICENSE"), b"MIT").unwrap();

        let overlay = Overlay::locate(&gamedata).expect("overlay dir present");
        let mc1 = overlay.levels("mc1").unwrap();
        assert_eq!(
            mc1.iter()
                .map(|l| (l.index, l.rel.as_str()))
                .collect::<Vec<_>>(),
            [
                (2, "mc1/levels/lev00002.dat"),
                (32, "mc1/levels/LEV00032.DAT")
            ]
        );
        assert!(overlay.levels("mc2").unwrap().is_empty());
        assert!(Overlay::locate(&scratch("absent")).is_none());

        std::fs::remove_dir_all(&gamedata).ok();
    }

    #[test]
    fn duplicate_member_is_an_error() {
        let gamedata = scratch("dup");
        let levels = gamedata.join("overlay/mc2/levels");
        std::fs::create_dir_all(&levels).unwrap();
        std::fs::write(levels.join("LEV00003.DAT"), b"a").unwrap();
        std::fs::write(levels.join("lev00003.dat"), b"b").unwrap();

        let overlay = Overlay::locate(&gamedata).unwrap();
        let err = overlay.levels("mc2").unwrap_err();
        assert!(err.contains("member 3"), "unexpected error: {err}");

        std::fs::remove_dir_all(&gamedata).ok();
    }

    #[test]
    fn name_parsing() {
        assert_eq!(level_index("LEV00032.DAT"), Some(32));
        assert_eq!(level_index("lev99999.dat"), Some(99999));
        assert_eq!(level_index("LEV0032.DAT"), None);
        assert_eq!(level_index("LEV000320.DAT"), None);
        assert_eq!(level_index("LEV00032.BIN"), None);
        assert_eq!(level_index("XLEV00032.DAT"), None);
    }
}
