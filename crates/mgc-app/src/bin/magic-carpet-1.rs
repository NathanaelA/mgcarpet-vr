//! Double-clickable launcher: the Magic Carpet campaign
//! (`mgcarpet --campaign mc1`).

#[path = "../launcher.rs"]
mod launcher;

fn main() {
    launcher::run("mc1")
}
