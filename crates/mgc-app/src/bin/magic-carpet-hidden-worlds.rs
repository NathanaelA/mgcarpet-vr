//! Double-clickable launcher: the Magic Carpet: Hidden Worlds campaign
//! (`mgcarpet --campaign mc1hw`).

#[path = "../launcher.rs"]
mod launcher;

fn main() {
    launcher::run("mc1hw")
}
