//! Double-clickable launcher: the Magic Carpet 2 campaign
//! (`mgcarpet --campaign mc2`).

#[path = "../launcher.rs"]
mod launcher;

fn main() {
    launcher::run("mc2")
}
