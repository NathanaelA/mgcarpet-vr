//! Shared body of the three double-clickable campaign launchers
//! (`magic-carpet-1`, `magic-carpet-hidden-worlds`, `magic-carpet-2`):
//! find the real `mgcarpet` binary sitting next to this executable,
//! pin the working directory to that folder (all data lookup —
//! `gamedata/`, `baked/`, `saves/`, `mgcarpet.json` — is cwd-relative,
//! and a file-manager double-click often starts in `$HOME` or wherever
//! a shortcut's "Start in" points), then hand over with
//! `--campaign <id>`, forwarding any extra arguments.
//!
//! This file is NOT part of the `mgcarpet` binary — each launcher in
//! `src/bin/` imports it with `#[path]`. Std-only on purpose: the
//! launchers must stay trivial to audit and add nothing to build time.

use std::path::{Path, PathBuf};
use std::process::{Command, exit};

/// A directory that already looks like a game root keeps its claim on
/// the working directory: `gamedata/` (retail packs), `baked/` (a
/// previous bake), or `mgcarpet.json` (an explicit config, possibly
/// pointing at data elsewhere).
fn is_game_root(dir: &Path) -> bool {
    dir.join("gamedata").is_dir()
        || dir.join("baked").is_dir()
        || dir.join("mgcarpet.json").is_file()
}

pub fn run(campaign: &str) -> ! {
    let exe = std::env::current_exe().ok();
    let Some(dir) = exe.as_deref().and_then(|p| p.parent()).map(PathBuf::from) else {
        fail("cannot locate this launcher's own directory");
    };
    let game = dir.join(if cfg!(windows) {
        "mgcarpet.exe"
    } else {
        "mgcarpet"
    });
    if !game.exists() {
        fail(&format!(
            "{} not found — this launcher must sit in the same folder as the \
             mgcarpet binary it starts",
            game.display()
        ));
    }
    let mut cmd = Command::new(&game);
    cmd.arg("--campaign")
        .arg(campaign)
        .args(std::env::args_os().skip(1));
    // An intentional working directory wins: launched from a dev
    // checkout or from inside the game folder, the caller's cwd
    // already holds (or points at) the data, and yanking it to the
    // binary's folder would lose it — the dev tree keeps its binaries
    // in target/<profile>/, away from the data. Pin to the launcher's
    // own folder only when the caller's cwd shows no game root at all:
    // the file-manager double-click that starts in $HOME.
    let cwd_is_root = std::env::current_dir()
        .map(|d| is_game_root(&d))
        .unwrap_or(false);
    if !cwd_is_root {
        cmd.current_dir(&dir);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec(); // only returns on failure
        fail(&format!("failed to launch {}: {err}", game.display()));
    }
    #[cfg(not(unix))]
    {
        match cmd.status() {
            Ok(s) => exit(s.code().unwrap_or(1)),
            Err(e) => fail(&format!("failed to launch {}: {e}", game.display())),
        }
    }
}

/// Print the error and, on Windows — where the console a double-click
/// opened vanishes with the process — hold the window until Enter so
/// the message can actually be read.
fn fail(msg: &str) -> ! {
    eprintln!("mgcarpet launcher: {msg}");
    if cfg!(windows) {
        eprintln!("press Enter to close");
        let _ = std::io::stdin().read_line(&mut String::new());
    }
    exit(1)
}
