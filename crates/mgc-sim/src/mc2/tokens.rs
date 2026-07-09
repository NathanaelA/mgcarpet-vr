//! MC2 class-15 spell tokens — the SPELL JARS (Phase 4.3; the
//! prerequisite entity layer for the Phase-4.2 spell system). Trace
//! bank: docs/traces/mc2-class15-spell-tokens.md (`EF:` = remc2
//! EventsFunctions.cpp, `L:` = Level.cpp).
//!
//! One creator serves all 26 spells (`AddSpellXX_XX_51120` EF:54124
//! behind the 26 thin `AddSpellNN` wrappers): class 15, model = the
//! spell index 0..25, actionIndex = 3*model, sprite 77 for EVERY
//! model, pickup box 768/768/1280. Each model owns three consecutive
//! action states: 3M = the spell EFFECT (gated on an active cast —
//! inert for a fresh token), 3M+1 = pickup, 3M+2 = self-replenishing
//! pickup (collection drops a fresh state-3M+2 token in place). The
//! authored THING's `swi_id` selects the state (`actionIndex +=
//! stageTag`, >= 3 -> junk state 253 — the shared class-12/15 spawn
//! case, EF:33209-33217).
//!
//! Phase-4.3 scope: the token entity + pickup (grant banked into
//! [`Gen::mc2_spell_tokens`] like the class-14 scrolls). The cast
//! machinery (`SetSpell_6D5E0` data wiring, `word_0x2E_46` cast
//! gate, the death scatter) is Phase 4.2.

use crate::mc1::features::Gen;

impl Gen {
    /// `AddSpellXX_XX_51120` (EF:54124) — the shared token ctor:
    /// maxLife/life 0, byte[0] &= 0xF7 (untargetable), map-linked,
    /// fixed sprite 77 (the jar), pickup half-extents 768/768/1280
    /// (SetEntityShiftRot EF:32874). No RNG. `SetSpell_6D5E0(_, 0)`
    /// (the per-spell mana/tier wiring) is Phase 4.2 — a fresh
    /// token's pickup path never reads those fields.
    pub(crate) fn mc2_spawn_spell_token(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 15;
            e.model65 = model;
            e.max_life = 0;
            e.tick70 = model.wrapping_mul(3);
            e.flags &= !0x8;
        }
        self.link(i, x, y, z);
        self.mc2_set_sprite(i, 77);
        self.extents(i, 768, 1280);
        self.refill_life(i);
        Some(i)
    }
}
