//! The end-of-level STATS LEDGER — what the performance screen reads.
//!
//! Two layers, kept apart on purpose:
//!
//! * **Retail's counters**, verbatim: the creature census (the
//!   kills-% denominator — MC1 `str_232607.var_u32_232607`
//!   (`dword_38C9F`), MC2 `str_0x364D2.dword_0x364D2`) and the
//!   spell-offer census (MC1 `var_u8_232611`, MC2 `dword_0x364D6`).
//!   The numerators (`Gen::kills/shots/hits`, the owned book, banked
//!   mana) already live where retail keeps them.
//! * **The enhanced death tally** (player ruling 2026-09-24): EVERY
//!   scored creature death, billed to whoever holds its killer latch —
//!   the player, a rival, or the environment. Retail credits only the
//!   human's `+359`, so "Creatures Killed" can never reach 100% on a
//!   level cleared by rivals, fire or a castle.
//!
//! The ledger is pure bookkeeping: nothing in the sim reads it, it
//! rides [`Gen`](super::features::Gen) inside a `HashSilent` (so no
//! golden moves), and it is carried by the save (snapshot v26).

use crate::snapshot::{Reader, Snap, SnapshotError, Writer};

/// The owner tag of the human (a victim's `f38` when the player
/// killed it).
const PLAYER_TAG: u16 = 0xFFFF;

/// Retail's spell-offer census is a raw memory run: MC1 memsets 96
/// bytes and bumps a DWORD at byte offset `model` (so a model up to
/// 95 touches byte 98); MC2 memsets 104 (26 dwords).
pub(crate) const SPELL_CENSUS_BYTES: usize = 104;

/// Who a creature death is billed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Killer {
    /// The human — directly, or through anything that carries the
    /// player's owner tag (fire the player lit, a volcano the player
    /// cast, the player's castle, the player's own creatures).
    Player,
    /// A rival wizard, by player slot (1..=7).
    Rival(u8),
    /// Nobody: wild creatures, unowned fire, drowning, walled in.
    Environment,
}

/// The ledger itself. `deaths[0]` = player, `[1..8]` = rival slots,
/// `[8]` = environment.
#[derive(Clone, Debug)]
pub(crate) struct StatsLedger {
    pub(crate) census: i32,
    pub(crate) spell_census: [u8; SPELL_CENSUS_BYTES],
    pub(crate) deaths: [u32; 9],
    /// Killer hints for the DIRECT kills — writers that stamp
    /// `act_life = -1` themselves (castle footprint crush, doomsday
    /// kill-all, the death field). The victim's next inbox overwrites
    /// `f38` with a mail-less `f40 = 0`, so the tag would be lost by
    /// the time the prekill bills it. (slot, owner tag); consumed by
    /// the prekill.
    pub(crate) hints: Vec<(u16, u16)>,
    /// MC2: the spell census is owed at the next tick top (retail
    /// runs it at the first tick's seating, after the book carry).
    pub(crate) mc2_spell_census_due: bool,
    /// MC2 `array_0x403_1027x` — spells PICKED UP this level (the
    /// collect block `sub_68FF0` EF:56044 sets it; the derive reads
    /// it as "found"). Bit per spell.
    pub(crate) mc2_found: u32,
    /// MC2: every spell the level authors a class-15 jar for (bit per
    /// spell, read at load before any record is consumed).
    pub(crate) mc2_jars: u32,
    /// MC2: `mc2_jars` minus the spells the wizard started with — the
    /// enhanced "Spells found" denominator.
    pub(crate) mc2_offered: u32,
    /// The ENHANCED accuracy tally — retail's gated shots and hits
    /// minus the possession bolts (player ruling 2026-09-24). MC1
    /// shadows `Gen::shots/hits`; in MC2 it is the only shot count
    /// (`sub_65780`, `mc2_shot_stats`).
    pub(crate) offensive_shots: u32,
    pub(crate) offensive_hits: u32,
}

impl Default for StatsLedger {
    fn default() -> Self {
        StatsLedger {
            census: 0,
            spell_census: [0; SPELL_CENSUS_BYTES],
            deaths: [0; 9],
            hints: Vec::new(),
            mc2_spell_census_due: false,
            mc2_found: 0,
            mc2_jars: 0,
            mc2_offered: 0,
            offensive_shots: 0,
            offensive_hits: 0,
        }
    }
}

impl StatsLedger {
    /// MC1 `sub_37440` (:43954): `inc dword [census + model]` —
    /// CARPET.EXE 0x4FCE3 `ff 84 02 a3 8c 03 00`, a DWORD increment at
    /// a BYTE-scaled index. The derive (`sub_448E0` :54658, file
    /// 0x5D141 `83 bc 19 …` with the index stepping by 4) then reads
    /// dword `i` = bytes `4i..4i+3`. Kept raw so the faithful row is
    /// retail's own arithmetic.
    pub(crate) fn mc1_offer(&mut self, model: u16) {
        let o = model as usize;
        if o + 4 > SPELL_CENSUS_BYTES {
            return;
        }
        let v = u32::from_le_bytes(self.spell_census[o..o + 4].try_into().unwrap());
        self.spell_census[o..o + 4].copy_from_slice(&v.wrapping_add(1).to_le_bytes());
    }

    /// MC2 `sub_574A0` (EF:40141): `dword_0x364D6[subtype]++`.
    pub(crate) fn mc2_offer(&mut self, spell: u16) {
        let i = spell as usize;
        if i >= SPELL_CENSUS_BYTES / 4 {
            return;
        }
        let o = i * 4;
        let v = u32::from_le_bytes(self.spell_census[o..o + 4].try_into().unwrap());
        self.spell_census[o..o + 4].copy_from_slice(&v.wrapping_add(1).to_le_bytes());
    }

    /// Dword `i` of the spell census, as both derivations read it.
    pub(crate) fn offer_dword(&self, i: usize) -> u32 {
        let o = i * 4;
        if o + 4 > SPELL_CENSUS_BYTES {
            return 0;
        }
        u32::from_le_bytes(self.spell_census[o..o + 4].try_into().unwrap())
    }

    /// Byte `m` of the census — MC1's per-MODEL offer count as the
    /// census MEANT it (the enhanced spells row).
    pub(crate) fn offer_byte(&self, m: usize) -> u8 {
        self.spell_census.get(m).copied().unwrap_or(0)
    }

    pub(crate) fn clear_spell_census(&mut self) {
        self.spell_census = [0; SPELL_CENSUS_BYTES];
    }

    /// Record a direct-kill hint (replaces an older one for the slot).
    pub(crate) fn hint(&mut self, slot: usize, tag: u16) {
        let slot = slot as u16;
        if let Some(h) = self.hints.iter_mut().find(|h| h.0 == slot) {
            h.1 = tag;
        } else {
            self.hints.push((slot, tag));
        }
    }

    pub(crate) fn take_hint(&mut self, slot: usize) -> Option<u16> {
        let at = self.hints.iter().position(|h| h.0 as usize == slot)?;
        Some(self.hints.swap_remove(at).1)
    }

    pub(crate) fn bill(&mut self, k: Killer) {
        let i = match k {
            Killer::Player => 0,
            Killer::Rival(s) => (s as usize).clamp(1, 7),
            Killer::Environment => 8,
        };
        self.deaths[i] = self.deaths[i].saturating_add(1);
    }
}

impl Snap for StatsLedger {
    fn put(&self, w: &mut Writer) {
        w.put(&self.census);
        w.raw(&self.spell_census);
        w.put(&self.deaths);
        w.put(&(self.hints.len() as u32));
        for &(s, t) in &self.hints {
            w.put(&s);
            w.put(&t);
        }
        w.put(&self.mc2_spell_census_due);
        w.put(&self.mc2_found);
        w.put(&self.mc2_jars);
        w.put(&self.mc2_offered);
        w.put(&self.offensive_shots);
        w.put(&self.offensive_hits);
    }
    fn get(r: &mut Reader) -> Result<Self, SnapshotError> {
        let census = r.get()?;
        let spell_census = r.raw(SPELL_CENSUS_BYTES)?.try_into().unwrap();
        let deaths = r.get()?;
        let n: u32 = r.get()?;
        let mut hints = Vec::with_capacity(n.min(4096) as usize);
        for _ in 0..n {
            hints.push((r.get()?, r.get()?));
        }
        let mc2_spell_census_due = r.get()?;
        let mc2_found = r.get()?;
        let mc2_jars = r.get()?;
        let mc2_offered = r.get()?;
        let offensive_shots = r.get()?;
        let offensive_hits = r.get()?;
        Ok(StatsLedger {
            census,
            spell_census,
            deaths,
            hints,
            mc2_spell_census_due,
            mc2_found,
            mc2_jars,
            mc2_offered,
            offensive_shots,
            offensive_hits,
        })
    }
}

/// The creature models NEITHER game scores: retail's credit gate
/// (MC1 :21840-50, MC2 `PreKillEntity_1C890` EF:9543-51) and the MC1
/// census (:43947) both skip them. (MC2's census additionally skips
/// 22 — see [`mc2_census_counts`].)
pub(crate) fn scored_model(model: u8) -> bool {
    !matches!(model, 9 | 12 | 13 | 14 | 15)
}

/// MC2 `sub_4A1E0` (EF:33016-18): the census counts class-5 records
/// with `subtype < 0xC || (subtype > 0xF && subtype != 0x16)`.
pub(crate) fn mc2_census_counts(model: u16) -> bool {
    model < 12 || (model > 15 && model != 22)
}

impl super::features::Gen {
    /// Resolve an owner tag to a killer. `rival_ents` holds each
    /// rival's wizard entity (its own owner tag). Anything else is a
    /// record slot whose `+24` owner tag is followed, hop by hop, until
    /// it names a wizard. A live projectile's slot (the MC2 m0 dodge
    /// hook's overload) is one hop. A tree's standing fire is two or
    /// more (fire → the fire that spread to it → … → the wizard's
    /// bolt), because the tree's flame takes the BROADCASTER's `+24`
    /// (`tree_tick`, :57683). Player ruling 2026-09-24: fire with a
    /// clear owner bills that wizard. A record owning itself (the
    /// allocator's default `id24 = slot`, i.e. a wild creature or an
    /// unowned fire), a dead end, or the hop cap is the environment.
    pub(crate) fn stats_killer_of(&self, tag: u16) -> Killer {
        let wizard = |t: u16| -> Option<Killer> {
            if t == PLAYER_TAG {
                return Some(Killer::Player);
            }
            let s = self.rival_ents.iter().position(|&e| e != 0 && e == t)?;
            Some(if s == 0 {
                Killer::Player
            } else {
                Killer::Rival(s as u8)
            })
        };
        let mut t = tag;
        for _ in 0..8 {
            if t == 0 {
                break;
            }
            if let Some(k) = wizard(t) {
                return k;
            }
            let Some(e) = self.ent.get(t as usize) else {
                break;
            };
            if e.id24 == t {
                break;
            }
            t = e.id24;
        }
        Killer::Environment
    }

    /// Bill one creature death to the ledger — called at the prekill
    /// (both games) after the chain inherit, before the state change.
    /// Unscored models and the player's own creatures are not
    /// "monsters" and are not billed.
    pub(crate) fn stats_note_death(&mut self, i: usize) {
        let hint = self.stats.0.take_hint(i);
        let e = &self.ent[i];
        if !scored_model(e.model65) || e.id24 == PLAYER_TAG {
            return;
        }
        let tag = if e.f38 != 0 { e.f38 } else { hint.unwrap_or(0) };
        let k = self.stats_killer_of(tag);
        self.stats.0.bill(k);
    }

    /// Scored creatures still standing: live class-5 chain HEADS of a
    /// scored model, not the player's own, not yet dying. A chain
    /// segment is any record some class-5 record's `f54` names.
    pub(crate) fn stats_alive_creatures(&self) -> u32 {
        let n = self.ent.len();
        let mut seg = vec![false; n];
        for e in self.ent.iter() {
            if e.class64 == 5 && (e.f54 as usize) < n && e.f54 != 0 {
                seg[e.f54 as usize] = true;
            }
        }
        let mut alive = 0;
        for (i, e) in self.ent.iter().enumerate().skip(1) {
            if e.class64 == 5
                && !seg[i]
                && scored_model(e.model65)
                && e.id24 != PLAYER_TAG
                && e.act_life >= 0
            {
                alive += 1;
            }
        }
        alive
    }
}

/// Everything the performance screen shows, raw and derived. Built
/// by [`crate::engine::world::World::level_stats`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LevelStats {
    /// True for MC2 (no Overall row; different derivations).
    pub mc2: bool,
    // ---- retail's raw counters ----
    /// The human's creature kills (`+359` / `creaturesKilledPercent`
    /// before the derive).
    pub kills: u32,
    /// Retail's creature census (the kills denominator).
    pub census: i32,
    pub shots: u32,
    pub hits: u32,
    /// Spells offered / found, as RETAIL counts them.
    pub spells_offered: u32,
    pub spells_found: u32,
    /// Banked mana (houses + castle store) and the world total.
    pub banked: u32,
    pub world_mana: u32,
    /// A castle stands (MC1's mana row is 0 without one).
    pub has_castle: bool,
    // ---- retail's derived rows (0..=100) ----
    pub killed_pct: u32,
    pub accuracy_pct: u32,
    pub spells_pct: u32,
    pub mana_pct: u32,
    /// MC1 only — the mean of the applicable rows plus Mana.
    pub overall_pct: u32,
    // ---- the enhanced tally ----
    /// Every scored creature death: [player, rival 1..7, environment].
    pub deaths: [u32; 9],
    /// Scored creatures still alive at the snapshot.
    pub alive: u32,
    /// Spells offered / found, counted per spell over every jar the
    /// level authors (MC1's census bug undone; MC2's census blind spot
    /// — it counts after the load consumed the placed jars — undone).
    pub spells_offered_fixed: u32,
    pub spells_found_fixed: u32,
    /// Shots / hits without the possession bolts (MC1 model 1; MC2
    /// models 1 and 17 — the only MC2 shot count, see
    /// `mc2_shot_stats`).
    pub offensive_shots: u32,
    pub offensive_hits: u32,
    /// The mana census without its seed (MC1's intrinsic 1000, MC2's
    /// 1 — neither is ever collectable): the world total, everything
    /// credited to the player, and the dwellings' share of that.
    /// Owned minus dwellings is the castle store plus the mana still
    /// on its way there (balloons, claimed balls).
    pub mana_world: u32,
    pub mana_owned: u32,
    pub mana_houses: u32,
}

impl LevelStats {
    pub fn deaths_total(&self) -> u32 {
        self.deaths.iter().sum()
    }
    pub fn deaths_player(&self) -> u32 {
        self.deaths[0]
    }
    pub fn deaths_rivals(&self) -> u32 {
        self.deaths[1..8].iter().sum()
    }
    pub fn deaths_environment(&self) -> u32 {
        self.deaths[8]
    }
    /// Every creature that ever lived on the level (and was scored).
    pub fn creatures_total(&self) -> u32 {
        self.deaths_total() + self.alive
    }
    /// The kill breakdown in tenths of a percent — by you, by rivals,
    /// by nature, still alive — summing to exactly 100.0.
    pub fn kill_split10(&self) -> [u32; 4] {
        let v = split10(&[
            self.deaths_player(),
            self.deaths_rivals(),
            self.deaths_environment(),
            self.alive,
        ]);
        [v[0], v[1], v[2], v[3]]
    }
    /// The enhanced "Creatures Killed" (tenths): share of all creatures
    /// dead, by anyone — the sum of the breakdown's killer rows, so the
    /// headline always agrees with them. 100.0 on a level that had none.
    pub fn cleared_pct10(&self) -> u32 {
        if self.creatures_total() == 0 {
            return 1000;
        }
        1000 - self.kill_split10()[3]
    }
    /// The enhanced "Accuracy" (tenths): offensive spells only.
    pub fn offensive_accuracy_pct10(&self) -> u32 {
        pct10(self.offensive_hits, self.offensive_shots)
    }
    /// The enhanced "Spells found" (tenths).
    pub fn spells_fixed_pct10(&self) -> u32 {
        pct10(self.spells_found_fixed, self.spells_offered_fixed)
    }
    /// The mana breakdown in tenths — in the castle (with what is on
    /// its way there), in dwellings, unclaimed (rivals' included) —
    /// summing to exactly 100.0 (all zero on a level without mana).
    pub fn mana_split10(&self) -> [u32; 3] {
        let v = split10(&self.mana_rows());
        [v[0], v[1], v[2]]
    }
    /// The mana breakdown's absolutes: `[castle (with what is on its
    /// way there), dwellings, unclaimed]`; the first two sum to owned.
    pub fn mana_rows(&self) -> [u32; 3] {
        mana_parts(self.mana_world, self.mana_owned, self.mana_houses)
    }
    /// The enhanced "Mana" (tenths): owned over the seedless world
    /// total — castle plus dwellings, so it agrees with the breakdown.
    /// 100.0 on a level without mana.
    pub fn mana_pct10(&self) -> u32 {
        if self.mana_world == 0 {
            return 1000;
        }
        let [c, h, _] = self.mana_split10();
        c + h
    }
    /// The enhanced "Overall" (tenths): retail's rule (`sub_448E0`
    /// :54730-54 — the mean of the rows that APPLY, Mana always) over
    /// the enhanced rows, rounded half up.
    pub fn overall_enhanced_pct10(&self) -> u32 {
        let mut sum = self.mana_pct10();
        let mut n = 1;
        if self.creatures_total() != 0 {
            sum += self.cleared_pct10();
            n += 1;
        }
        if self.offensive_shots != 0 {
            sum += self.offensive_accuracy_pct10();
            n += 1;
        }
        if self.spells_offered_fixed != 0 {
            sum += self.spells_fixed_pct10();
            n += 1;
        }
        (2 * sum + n) / (2 * n)
    }
}

/// `[castle, dwellings, unclaimed]` from the seedless census totals.
pub fn mana_parts(world: u32, owned: u32, houses: u32) -> [u32; 3] {
    let owned = owned.min(world);
    let houses = houses.min(owned);
    [owned - houses, houses, world - owned]
}

/// `n/d` in tenths of a percent (0..=1000), rounded half up (player
/// ruling 2026-09-25), with STRICT ends: 100.0 only when `n == d`,
/// 0.0 only when `n == 0` — a near miss reads 99.9 / 0.1. 100.0 when
/// there is nothing to count.
pub fn pct10(n: u32, d: u32) -> u32 {
    if d == 0 || n >= d {
        return 1000;
    }
    if n == 0 {
        return 0;
    }
    let (n, d) = (n as u64, d as u64);
    ((2000 * n + d) / (2 * d)).clamp(1, 999) as u32
}

/// Split 100.0 % over `parts` in tenths so the rows sum to exactly
/// 1000 (player ruling 2026-09-25): every part takes its floor, the
/// leftover tenths go to the largest remainders (a tie to the smaller
/// part), and the strict ends of [`pct10`] hold — a non-zero part
/// never reads 0.0 (it borrows a tenth from the largest row), so a
/// part short of the whole never reads 100.0. All zero when the parts
/// are.
pub fn split10(parts: &[u32]) -> Vec<u32> {
    let d: u64 = parts.iter().map(|&p| p as u64).sum();
    if d == 0 {
        return vec![0; parts.len()];
    }
    let mut v: Vec<u32> = parts
        .iter()
        .map(|&p| (1000 * p as u64 / d) as u32)
        .collect();
    let rem = |i: usize| 1000 * parts[i] as u64 % d;
    let short = 1000 - v.iter().sum::<u32>() as usize;
    let mut order: Vec<usize> = (0..parts.len()).collect();
    order.sort_by(|&a, &b| rem(b).cmp(&rem(a)).then(parts[a].cmp(&parts[b])));
    for &i in order.iter().take(short) {
        v[i] += 1;
    }
    for i in 0..parts.len() {
        if parts[i] > 0 && v[i] == 0 {
            let big = (0..v.len())
                .max_by_key(|&j| (v[j], std::cmp::Reverse(j)))
                .unwrap();
            v[big] -= 1;
            v[i] = 1;
        }
    }
    v
}

/// Tenths as "70.4".
pub fn fmt10(p: u32) -> String {
    format!("{}.{}", p / 10, p % 10)
}

/// Tenths to a whole percentage for retail's integer rows (the MC2
/// save's score table), keeping the strict ends.
pub fn whole(p10: u32) -> u32 {
    match p10 {
        0 => 0,
        1000.. => 100,
        p => ((p + 5) / 10).clamp(1, 99),
    }
}

/// `100·n/d` clamped to 0..=100; 100 when there is nothing to count.
pub fn pct(n: u32, d: u32) -> u32 {
    if d == 0 {
        100
    } else {
        (100 * n as u64 / d as u64).min(100) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ THE MC1 SPELL CENSUS IS A DWORD BUMP AT A BYTE INDEX
    /// (CARPET.EXE 0x4FCE3) READ BACK A DWORD PER SPELL (0x5D141).
    /// Jar models 0..3 all land in dword 0; model 5 in dword 1.
    #[test]
    fn mc1_spell_census_is_retails_byte_indexed_dword() {
        let mut l = StatsLedger::default();
        for m in [0, 2, 3, 5] {
            l.mc1_offer(m);
        }
        assert_ne!(l.offer_dword(0), 0);
        assert_ne!(l.offer_dword(1), 0);
        assert_eq!(l.offer_dword(2), 0, "no jar of model 8..11");
        assert_eq!(
            l.offer_byte(1),
            0,
            "the per-model reading: model 1 was not offered"
        );
        assert_eq!(l.offer_byte(5), 1);
    }

    #[test]
    fn pct10_rounds_half_up_with_strict_ends() {
        assert_eq!(pct10(162, 230), 704); // 70.43
        assert_eq!(pct10(68, 230), 296); // 29.57
        assert_eq!(pct10(1, 8), 125); // 12.5 exactly
        assert_eq!(pct10(1, 16), 63); // 6.25 → 6.3
        assert_eq!(pct10(1999, 2000), 999, "99.95 is not 100.0");
        assert_eq!(pct10(1, 3000), 1, "0.03 is not 0.0");
        assert_eq!(pct10(0, 5), 0);
        assert_eq!(pct10(5, 5), 1000);
        assert_eq!(pct10(0, 0), 1000, "nothing to count");
    }

    #[test]
    fn split10_sums_to_exactly_one_hundred() {
        assert_eq!(split10(&[162, 68]), vec![704, 296]);
        assert_eq!(split10(&[1, 1, 1]), vec![334, 333, 333]);
        for parts in [
            [7u32, 13, 29, 51],
            [1, 2998, 0, 1],
            [3, 3, 3, 0],
            [123, 456, 789, 1011],
        ] {
            assert_eq!(split10(&parts).iter().sum::<u32>(), 1000, "{parts:?}");
        }
        // A near miss: the half-tenth tie would crown the big part
        // 100.0 and the small one 0.0 — the strict ends forbid both.
        assert_eq!(split10(&[1999, 1]), vec![999, 1]);
        assert_eq!(split10(&[1, 2998, 0, 1]), vec![1, 998, 0, 1]);
        assert_eq!(split10(&[0, 0]), vec![0, 0]);
        assert_eq!(split10(&[0, 9]), vec![0, 1000]);
    }

    #[test]
    fn a_cleared_level_reaches_one_hundred_percent_mana() {
        // The census seed is gone: everything collected is 100.0.
        let s = LevelStats {
            mana_world: 12_000,
            mana_owned: 12_000,
            mana_houses: 3_000,
            ..Default::default()
        };
        assert_eq!(s.mana_pct10(), 1000);
        assert_eq!(s.mana_split10(), [750, 250, 0]);
        let s = LevelStats {
            mana_owned: 11_999,
            ..s
        };
        assert_eq!(s.mana_pct10(), 999);
    }

    #[test]
    fn the_kill_headline_agrees_with_its_breakdown() {
        let mut s = LevelStats::default();
        s.deaths[0] = 162;
        s.deaths[8] = 68;
        assert_eq!(s.kill_split10(), [704, 0, 296, 0]);
        assert_eq!(s.cleared_pct10(), 1000);
        s.alive = 1;
        let k = s.kill_split10();
        assert_eq!(k.iter().sum::<u32>(), 1000);
        assert_eq!(s.cleared_pct10(), k[0] + k[1] + k[2]);
        assert!(s.cleared_pct10() < 1000);
    }

    #[test]
    fn ledger_round_trips_through_the_snapshot_codec() {
        let mut l = StatsLedger {
            census: 17,
            ..Default::default()
        };
        l.mc1_offer(4);
        l.bill(Killer::Rival(3));
        l.bill(Killer::Environment);
        l.hint(40, 0xFFFF);
        l.mc2_spell_census_due = true;
        l.mc2_found = 0b101;
        l.mc2_jars = 0b111;
        l.mc2_offered = 0b110;
        let mut w = Writer::new();
        w.put(&l);
        let buf = w.into_buf();
        let mut r = Reader::for_test(&buf);
        let back: StatsLedger = r.get().unwrap();
        assert_eq!(back.census, 17);
        assert_eq!(back.spell_census, l.spell_census);
        assert_eq!(back.deaths, l.deaths);
        assert_eq!(back.hints, l.hints);
        assert!(back.mc2_spell_census_due);
        assert_eq!(back.mc2_found, 0b101);
        assert_eq!(back.mc2_jars, 0b111);
        assert_eq!(back.mc2_offered, 0b110);
    }
}
