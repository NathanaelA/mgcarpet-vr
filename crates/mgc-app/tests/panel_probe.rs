//! DIG INSTRUMENT (ignored): the wizard-strip UI sprites per bundle
//! variant — dimensions and fully-transparent margins — so the mana
//! ruler recess can be measured against the 64-px fill `ui.rs::bar`
//! draws at +58. 2026-09-10: mc2-day's sub-panel (41) is 129×45 with a
//! transparent leading column/row; every other variant is 128×44.
use std::path::Path;

#[test]
#[ignore]
fn wizard_panel_recess_probe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../baked/assets");
    for variant in [
        "mc2-night",
        "mc2-day",
        "mc2-night-fog",
        "mc2-cave",
        "mc1-temperate",
    ] {
        let Ok(b) = mgc_formats::bundle::Bundle::load(&root.join(variant)) else {
            eprintln!("{variant}: no bundle");
            continue;
        };
        let Some((idx, px)) = b.ui_sprites.as_ref() else {
            continue;
        };
        let aw = idx.atlas_width as usize;
        for id in 38..=56usize {
            let Some(e) = idx.sprites.get(id) else {
                continue;
            };
            let Some(f) = e.frames.first() else {
                eprintln!("{variant} sprite {id}: no frame");
                continue;
            };
            let (w, h) = (e.width as usize, e.height as usize);
            let at = |x: usize, y: usize| px[(f.y as usize + y) * aw + f.x as usize + x];
            let col_clear = |x: usize| (0..h).all(|y| at(x, y) == 0);
            let row_clear = |y: usize| (0..w).all(|x| at(x, y) == 0);
            let left = (0..w).take_while(|&x| col_clear(x)).count();
            let right = (0..w).rev().take_while(|&x| col_clear(x)).count();
            let top = (0..h).take_while(|&y| row_clear(y)).count();
            let bottom = (0..h).rev().take_while(|&y| row_clear(y)).count();
            eprintln!(
                "{variant} sprite {id}: {w}x{h} flags {:#x} clear margins L{left} R{right} T{top} B{bottom}",
                e.flags
            );
            if id == 41 && std::env::var_os("PROBE_ROWS").is_some() {
                for y in 0..h {
                    let line: String = (0..w)
                        .map(|x| {
                            let p = at(x, y);
                            if p == 0 {
                                '.'
                            } else {
                                char::from(b'a' + (p % 26))
                            }
                        })
                        .collect();
                    eprintln!("{variant} {y:2} {line}");
                }
            }
        }
    }
}
