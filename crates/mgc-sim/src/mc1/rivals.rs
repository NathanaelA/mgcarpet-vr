//! Hostile (AI) wizards — the class-3 model-1 rival carpets: spawn,
//! per-tick brain, casting arm, mortality and respawn. Direct port of
//! the remc1 AI wizard machinery; all citations remc1 sub_main.cpp.
//! The full trace record lives in docs/ROADMAP.md "HOSTILE WIZARDS
//! (RIVAL AI) — TRACE BANK".
//!
//! Architecture mirrors the original: the AI is a DECISION LAYER over
//! the shared engines — rivals cast through the same class-12
//! manifestation entities, spawn the same class-9/10 projectiles and
//! effects (owner = the rival's wizard entity slot, so the generic
//! combat plumbing serves them unchanged), obey the same mana economy
//! (census ceiling, spell costs, castle-stored thresholds, the debit
//! riding the regen delta), and die into the same jar-scatter + grave
//! sequence as the human.
//!
//! Retail AI-vs-human asymmetries, ported faithfully: AI heals 4x the
//! human's afield rate (:18013 vs :55418); AI at its castle DISCARDS
//! damage instead of forwarding it to the castle (:17975-79); the
//! AI's first castle is spawned directly, free and instant
//! (:19200-08); the AI carpet ignores walls, drag and knockback
//! (sub_14EB0 runs neither the wall gate nor the knock fields); AI
//! target scans are omniscient; the AI learns spells on a 200-tick
//! timer from any jar existing in the world instead of picking jars
//! up (:64805-14, :19381-443).
//!
//! Interim deviations (ours, flagged inline): the hate feed runs at
//! damage-intake and homing-acquisition time instead of the
//! original's per-projectile one-shot ledger scan (sub_16540 —
//! equivalent inputs, slightly later timing); creatures still target
//! only the human (OPEN: widen the mob scans to the full wizard
//! list); the duel pull on the CASTER is applied through the knock
//! channel (magnitude from the traced formula).

use crate::mc1::behavior::BEHAVIOR;
use crate::mc1::features::Gen;
use crate::mc1::mobs::PLAYER_TARGET;
use crate::mc1::spells::{SPELL_COUNT, SPELLS};
use crate::mc1::world::{LifeState, World};

/// Per-slot config from the level record (wizards.json), resolved by
/// the app: personality params, starting castle, and the two spell
/// masks (str_230867_37072[slot], :49222/:54965-67).
#[derive(Debug, Clone)]
pub struct RivalConfig {
    /// u16_522: hate rise rate, war thresholds, opportunism margins.
    pub aggression: u8,
    /// u16_524: commit aim cone, rebound-notice probability.
    pub accuracy: u8,
    /// u16_526: decision period, turn agility, burst pause, respawn.
    pub tempo: u8,
    /// Starting castle level: 0 = none, N = a castle at level N-1
    /// spawns with the wizard (level tail @38804+slot).
    pub castle_level: u8,
    /// Level-start book: pregrant && allowed (:49222).
    pub book: [bool; SPELL_COUNT],
    /// var_230983 — what the AI may LEARN mid-level (Type_160+796).
    pub allowed: [bool; SPELL_COUNT],
}

/// The rival wizard names, by player slot (off_99B68 :5741; slot 0 =
/// the human's default name).
pub const RIVAL_NAMES: [&str; 8] = [
    "Zanzamar",
    "Vodor",
    "Gryshnak",
    "Mahmoud",
    "Syed",
    "Raschid",
    "Alhabbal",
    "Scheherazade",
];

/// The hate ledger's neutral baseline (0x601F, :17946-67).
const HATE_NEUTRAL: u16 = 24607;
/// Hate toward a freshly (re)spawned wizard — elevated but decaying
/// (the post-respawn truce, -24609 as unsigned :55037-41).
const HATE_RESPAWN: u16 = 40927;

/// AI per-spell re-attempt cooldowns, ticks (word_90034 :2163).
const AI_RECAST: [u16; SPELL_COUNT] = [
    2, 1, 32, 10, 1, 0, 0, 4, 400, 0, 1, 0, 1, 0, 1, 1, 40, 600, 0, 1, 4, 2, 3, 4,
];

/// The AI brain state (Type_160+415). States 2/4/5/10 exist in the
/// original's table but no selector ever sets them (cut content).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub(crate) enum AiState {
    /// Fresh spawn: decide immediately (cascade runs twice, :17850).
    #[default]
    Fresh,
    /// Fly home, cast 0x10 = upgrade (sub_13800 :18106).
    Upgrade,
    /// Fly to the scouted site, plant the castle (sub_138F0 :18142).
    Build,
    /// Claim a mana ball with spell 3 (sub_13BA0 :18236).
    Possess,
    /// Raid an enemy castle (sub_13CA0 :18271).
    RaidCastle,
    /// Attack an enemy wizard (sub_13DC0 :18314).
    AttackWizard,
    /// Intercept an enemy balloon (same handler, state 9).
    RaidBalloon,
    /// Hunt any mana-holding creature (state 0xD).
    HuntMana,
    /// Return home (heal / regroup; sub_13A70 :18204).
    Home,
    /// Cruise (full life, nothing to do; sub_13A10 :18188).
    Cruise,
}

/// One live rival: the Type_160 subset the AI machinery needs. The
/// wizard's position/yaw/life/speed live on its pool entity (class 3
/// model 1); carried mana rides the entity's f140 mirror for the
/// census.
#[derive(Hash)]
pub(crate) struct Rival {
    /// Player slot (1..=7); slot 0 = the human, never a Rival.
    pub slot: u8,
    /// Wizard entity pool index. Also the rival's OWNER TAG: its
    /// projectiles' id24 and its claims' f144 (the original's +24 =
    /// own entity index, :44219).
    pub ent: u16,
    /// Manifestation pool slots by spell id (var_676; 0 = not owned).
    pub owned: [u16; SPELL_COUNT],
    /// Spells known across deaths — respawn re-mints manifestations
    /// (:54884-923); the scattered jars decay independently.
    pub known: [bool; SPELL_COUNT],
    /// Learn eligibility (Type_160+796).
    allowed: [bool; SPELL_COUNT],
    /// Spell-learning countdowns (+628): armed to 200 by a matching
    /// jar existing anywhere; conjures an own copy at expiry.
    learn: [u16; SPELL_COUNT],
    /// AI re-attempt cooldowns (+724, from [`AI_RECAST`]). Slot 16 is
    /// initialized to 4*slot — the per-player castle-build stagger
    /// the decompile shows as "var_756" (:55049).
    cooldown: [u16; SPELL_COUNT],
    /// Carried mana (+140) / ceiling (+136, census-owned) / regen
    /// delta (+132 — cast debits ride it negative).
    pub mana: u32,
    pub mana_max: u32,
    mana_delta: i32,
    /// Personality (u16_522/524/526).
    agg: u16,
    acc: u16,
    tempo: u16,
    pub state: AiState,
    /// Hate ledger + war flags per player slot (str_456; baseline
    /// [`HATE_NEUTRAL`]).
    pub(crate) hate: [u16; 8],
    pub(crate) war: [bool; 8],
    /// Fireball/lightning burst counter (+404): 8 shots then a
    /// negative lockout of (255-tempo)/8+1 ticks (:19129-36).
    burst: i16,
    /// Poverty latch (+406): mana < max/4 stops attack casting until
    /// it recovers past max/4+6000 (or max/2) (:19468-91).
    poverty: bool,
    /// Current target: entity slot or [`PLAYER_TARGET`]; 0 = none.
    /// Signature = team + model + (class<<7) (sub_15420 :19039).
    target: u16,
    target_sig: u16,
    /// Scouted castle site (+150).
    site: (u16, u16),
    /// Lateral dodge velocity (v_16; impulse 80, decay 4/tick).
    jink: i16,
    /// Desired speed (v_12) toward which f126 accelerates 16/tick.
    vdes: i16,
    /// Spawn grace (u16_331): mailbox discarded while > 0.
    pub(crate) grace: u16,
    /// Post-hit regen stall (u32_383).
    regen_stall: u16,
    /// Dead and castle-less: permanently out (byte_13329_6 = 0,
    /// :55622). Property is NOT torn down.
    pub eliminated: bool,
    /// Buff flags derived from the manifestations' bursts.
    pub shield: bool,
    pub invisible: bool,
    pub rebound: bool,
}

impl Rival {
    fn new(slot: u8, ent: u16, cfg: &RivalConfig) -> Self {
        let mut cooldown = [0u16; SPELL_COUNT];
        // The castle-build stagger (:55049): 4 ticks per player slot.
        cooldown[16] = 4 * slot as u16;
        Rival {
            slot,
            ent,
            owned: [0; SPELL_COUNT],
            known: [false; SPELL_COUNT],
            allowed: cfg.allowed,
            learn: [0; SPELL_COUNT],
            cooldown,
            mana: 1000,
            mana_max: 1000,
            mana_delta: 0,
            agg: cfg.aggression as u16,
            acc: cfg.accuracy as u16,
            tempo: cfg.tempo as u16,
            state: AiState::Fresh,
            hate: [HATE_NEUTRAL; 8],
            war: [false; 8],
            burst: 0,
            poverty: false,
            target: 0,
            target_sig: 0,
            site: (0, 0),
            jink: 0,
            vdes: 0,
            grace: 100,
            regen_stall: 0,
            eliminated: false,
            shield: false,
            invisible: false,
            rebound: false,
        }
    }

    /// Decision-tick gate: every `64 - tempo/4` ticks keyed on the
    /// entity age byte (:18024/:18065).
    fn think_period(&self) -> u8 {
        (64 - (self.tempo / 4) as i32).max(1) as u8
    }
}

impl World {
    /// Wire the level's wizards from the baked config: one rival per
    /// active AI slot (1..player_count), spawned at its start marker
    /// (class-3 model 4+slot placement, str_9177 :44068-107) with its
    /// level-start book and starting castle (sub_44D30 :54802-55005).
    /// Slot 0 (the human) is ignored here — the human's book comes
    /// from the campaign/jar machinery.
    pub fn set_wizards(&mut self, configs: &[Option<RivalConfig>; 8], player_count: u16) {
        for slot in 1..player_count.min(8) as u8 {
            let Some(cfg) = &configs[slot as usize] else {
                continue;
            };
            self.spawn_rival(slot, cfg.clone());
        }
    }

    fn spawn_rival(&mut self, slot: u8, cfg: RivalConfig) {
        // Start marker: class 3, model 4+slot (tile-center position,
        // :44003); fall back to the human's start marker cell.
        let marker = self.start_markers[slot as usize].or(self.start_markers[0]);
        let Some((mx, my)) = marker else { return };
        let x = (mx << 8).wrapping_add(128);
        let y = (my << 8).wrapping_add(128);
        let z = (self.g.ground_z(x, y) as i16).wrapping_add(256);
        let Some(i) = self.g.spawn_class3(1, x, y, z) else {
            return;
        };
        // Per-slot rival art (:54927-55): sprite-stats rows 273-279
        // (slot 0 keeps 44). Draw type 0x11: 16 views by mirror.
        self.g.set_sprite(i, 272 + slot as u16);
        self.g.refill_life(i);
        let mut r = Rival::new(slot, i as u16, &cfg);
        // Level-start book (:49222): pregrant && allowed, as resolved
        // by the app into cfg.book.
        for s in 0..SPELL_COUNT {
            if cfg.book[s] {
                r.known[s] = true;
                if let Some(m) = self.mint_manifestation(s, i as u16) {
                    r.owned[s] = m as u16;
                }
            }
        }
        // Starting castle (:54963-55005): AI-only, needs the Castle
        // spell known and a nonzero tail level. Spawned at the wizard
        // (even-parity tile snap in the spawn handler), pre-leveled,
        // FULL of mana, terrain stamped to match.
        if cfg.castle_level > 0 && r.known[16] {
            self.spawn_starting_castle(&mut r, cfg.castle_level);
        }
        // Everyone hates a newcomer a little (:55037-41).
        for other in &mut self.rivals {
            other.hate[slot as usize] = HATE_RESPAWN;
        }
        // The team resolver for owner recolors (balls/balloons/flags).
        self.g.rival_ents[slot as usize] = i as u16;
        self.rivals.push(r);
        self.entities_dirty = true;
    }

    /// A rival-owned class-12 manifestation (the shared sub_3BF70
    /// slot economy; f144 = the owner tag — 0 on the human's and on
    /// ground jars).
    fn mint_manifestation(&mut self, spell: usize, owner: u16) -> Option<usize> {
        let m = self.g.new_event()?;
        let f44 = self.spells()[spell].damage.min(u16::MAX as u32) as u16;
        {
            let e = &mut self.g.ent[m];
            e.class64 = 12;
            e.model65 = spell as u8;
            e.tick70 = crate::mc1::world::MANIFEST_BASE + spell as u8;
            e.flags &= !8;
            e.f26 = 0;
            e.f44 = f44;
            e.f144 = owner;
        }
        self.g.set_sprite(m, 77);
        let (h4, v4) = {
            let e = &self.g.ent[m];
            (e.f80 * 4, e.f84 * 4)
        };
        self.g.extents(m, h4, v4);
        Some(m)
    }

    /// Starting castle: (3,2) at the wizard, level = castle_level-1,
    /// footprint terrain replayed per stage (the sub_279D0 loop
    /// :54982-93), capacity ladder, spawns FULL (:54996-55002),
    /// sound 30 (:54981).
    fn spawn_starting_castle(&mut self, r: &mut Rival, castle_level: u8) {
        let (wx, wy) = {
            let e = &self.g.ent[r.ent as usize];
            (e.x, e.y)
        };
        let gz = self.g.ground_z(wx, wy) as i16;
        let Some(c) = self.g.spawn_class3(2, wx, wy, gz) else {
            return;
        };
        let lvl = (castle_level - 1).min(7);
        {
            let e = &mut self.g.ent[c];
            e.id24 = r.ent;
            e.f26 = lvl as i16;
            e.tick70 = 4; // standing (the buildable state the AI
            // upgrade gate checks, :18432)
        }
        // The castle flag wears the owner's team colors (sprite
        // 177 + team, the :30809-10 family).
        self.g.set_sprite(c, 177 + r.slot as u16);
        // Terrain: replay the build painter per stage (instant, the
        // divisor-1 flatten + paint of rows 1..=lvl+1).
        let (cx, cy, cz) = {
            let e = &self.g.ent[c];
            (
                ((e.x as u32 + 128) >> 8) as u8,
                ((e.y as u32 + 128) >> 8) as u8,
                (e.z >> 5) as i32,
            )
        };
        self.g.stamp_castle_terrain(lvl as usize + 1, cx, cy, cz);
        self.terrain_dirty = true;
        // Extents + capacity ladder + full stored mana (cap 320000).
        self.g.castle_extents(c, lvl);
        let cap = Gen::CASTLE_CAP[lvl as usize];
        self.g.ent[c].f136 = cap;
        self.g.ent[c].f140 = cap.clamp(0, 320_000);
        self.g.snd(30, c);
        let _ = r;
    }

    /// The rival's castle: (3,2) with id24 = the rival's entity (the
    /// original's Type_160.var_50, resolved by scan like
    /// [`World::player_castle`]).
    pub(crate) fn rival_castle(&self, ent: u16) -> Option<usize> {
        (1..self.g.ent.len()).find(|&j| {
            let e = &self.g.ent[j];
            e.class64 == 3 && e.model65 == 2 && e.flags & 0x400 == 0 && e.id24 == ent
        })
    }

    /// Resolve an owner tag (projectile id24 / claim f144) to a
    /// player slot: PLAYER_TARGET = 0 (the human), a live rival's
    /// entity slot = its player slot. Consults both rival columns.
    pub(crate) fn owner_slot(&self, owner: u16) -> Option<u8> {
        if owner == PLAYER_TARGET {
            return Some(0);
        }
        self.rivals
            .iter()
            .find(|r| r.ent == owner)
            .map(|r| r.slot)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.ent == owner)
                    .map(|r| r.slot)
            })
    }

    // ---- the per-tick brain (sub_13170 :17842) ---------------------------

    /// Class-3 model-1 pool dispatch: resolve the rival record; a
    /// level-authored husk with no record stands and renders (the
    /// pre-rivals behavior).
    pub(crate) fn rival_entity_tick(&mut self, i: usize) {
        let Some(ri) = self.rivals.iter().position(|r| r.ent as usize == i) else {
            return;
        };
        if self.rivals[ri].eliminated {
            return;
        }
        match self.g.ent[i].tick70 {
            // Death fall (state 2, sub_45FC0 :55434).
            2 => self.rival_death_fall(ri, i),
            // Dead on the ground (state 3, sub_46480 :55594).
            3 => self.rival_dead_wait(ri, i),
            // Alive (state 1).
            _ => self.rival_alive_tick(ri, i),
        }
    }

    fn rival_alive_tick(&mut self, ri: usize, i: usize) {
        // ---- housekeeping (sub_132B0 :17903) ----
        // Burst lockout recovery (:17936-38).
        if self.rivals[ri].burst < 0 {
            self.rivals[ri].burst += 1;
        }
        // AI recast cooldowns (:17939-45).
        for c in self.rivals[ri].cooldown.iter_mut() {
            *c = c.saturating_sub(1);
        }
        self.rival_hate_decay(ri);

        // At own castle: grace 2 + the mailbox is DISCARDED — the
        // AI's damage does NOT forward into the castle (:17971-79;
        // asymmetry vs the human's redirect, retail-check owed).
        let castle = self.rival_castle(self.rivals[ri].ent);
        let at_castle = castle.is_some_and(|c| {
            let (ex, ey) = (self.g.ent[i].x, self.g.ent[i].y);
            let e = &self.g.ent[c];
            ((ex.wrapping_sub(e.x) as i16).unsigned_abs()) <= e.f80
                && ((ey.wrapping_sub(e.y) as i16).unsigned_abs()) <= e.f82
        });
        if at_castle {
            self.rivals[ri].grace = self.rivals[ri].grace.max(2);
        }
        if self.rivals[ri].grace > 0 {
            self.rivals[ri].grace -= 1;
            self.g.ent[i].mail = [(0, 0); 6];
        } else {
            self.rival_damage_intake(ri, i);
            if self.g.ent[i].act_life < 0 {
                // Death (:17980-83): state 2, the fall.
                self.g.ent[i].tick70 = 2;
                self.g.ent[i].f46 = 0;
                self.g.snd(16, i); // the death scream (:55424-30)
                return;
            }
        }

        // Movement (sub_14EB0 :18780).
        self.rival_movement(ri, i);

        // Regen (:17990-18021): mana += delta then recompute; life
        // regen at the AI's own (faster) rates.
        {
            let r = &mut self.rivals[ri];
            let stepped = r.mana as i64 + r.mana_delta as i64;
            r.mana = stepped.clamp(0, r.mana_max as i64) as u32;
            r.mana_delta = if at_castle {
                ((r.mana_max / 200) as i32).max(1000)
            } else {
                ((r.mana_max / 2000) as i32).max(100)
            };
        }
        if self.rivals[ri].regen_stall > 0 {
            self.rivals[ri].regen_stall -= 1;
        } else {
            let max = self.g.ent[i].max_life as i32;
            let heal = if at_castle { max / 200 } else { max / 500 };
            self.g.ent[i].act_life = (self.g.ent[i].act_life + heal).min(max);
        }

        // Spell learning (sub_15EC0 :19381-443).
        self.rival_learn_tick(ri);

        // Buff flags from the manifestations' bursts.
        self.rival_refresh_buffs(ri);

        // Decision-tick work (:18024): incoming-projectile defense +
        // heal.
        let think = self.g.ent[i].f63 % self.rivals[ri].think_period() == 0;
        if think {
            self.rival_defense(ri, i);
            if self.g.ent[i].act_life < self.g.ent[i].max_life as i32 {
                self.rival_cast(ri, i, 1);
            }
        }

        // Altitude hard clamp (:18035-41).
        {
            let row = &BEHAVIOR[self.g.ent[i].row156 as usize];
            let ground = self.g.ground_z(self.g.ent[i].x, self.g.ent[i].y) as i16;
            let z = &mut self.g.ent[i].z;
            *z = (*z).clamp(
                ground.saturating_add(row.v_12),
                ground.saturating_add(row.v_10),
            );
        }

        // State handler + the decision cascade (fresh runs the
        // cascade twice, :17850-51).
        let fresh = self.rivals[ri].state == AiState::Fresh;
        self.rival_state_tick(ri, i, think);
        self.rival_selector(ri, i, think);
        if fresh {
            self.rival_selector(ri, i, think);
        }
        self.entities_dirty = true;
    }

    /// Hate regression toward the baseline (:17946-67): below rises
    /// by agg+1, above decays by 256-agg — but a war flag pins it.
    fn rival_hate_decay(&mut self, ri: usize) {
        let (agg, war) = (self.rivals[ri].agg, self.rivals[ri].war);
        for (p, h) in self.rivals[ri].hate.iter_mut().enumerate() {
            if *h < HATE_NEUTRAL {
                *h = (*h + agg + 1).min(HATE_NEUTRAL);
            } else if *h > HATE_NEUTRAL && !war[p] {
                *h = h.saturating_sub(256 - agg).max(HATE_NEUTRAL);
            }
        }
    }

    /// The shared wizard damage intake (sub_46540 :55641) on the
    /// rival's mailbox: shield quarter + mana pays it, steal/grip
    /// channels, regen stall, kill-credit latch. Also our hate-feed
    /// point (APPROX: the original feeds the ledger from the
    /// projectile scan sub_16540 — same inputs, slightly earlier).
    fn rival_damage_intake(&mut self, ri: usize, i: usize) {
        // ch3 mana steal (:55689-91): the attacker banks it.
        let (steal_amt, steal_src) = self.g.ent[i].mail[3];
        if steal_src != 0 {
            let take = (steal_amt as u32).min(self.rivals[ri].mana);
            self.rivals[ri].mana -= take;
            self.credit_wizard_mana(steal_src, take);
            self.g.ent[i].mail[3] = (0, 0);
        }
        // ch4 duel grip (:55663-82): the CASTER gets pulled toward
        // this victim; the victim only takes the side effects
        // (regen stall — the pull state lives on the ATTACKER).
        let (_, grip_src) = self.g.ent[i].mail[4];
        if grip_src != 0 {
            self.rivals[ri].regen_stall = 16;
            self.g.ent[i].mail[4] = (0, 0);
            if self.owner_slot_of_source(grip_src) == Some(0) {
                // u16_314/316/318 on the human: victim, counter,
                // clamp(dist, 1024, 3072) (:55671-77).
                let (vx, vy) = (self.g.ent[i].x, self.g.ent[i].y);
                let dist =
                    Gen::isqrt(Gen::dist2_sq(self.human_pose.0, self.human_pose.1, vx, vy) as u32);
                let hold = dist.clamp(1024, 3072);
                self.set_duel_latch(self.rivals[ri].ent, hold);
            }
        }
        // ch0 damage.
        let (amt, src) = self.g.ent[i].mail[0];
        if src == 0 && amt == 0 {
            return;
        }
        self.g.ent[i].mail[0] = (0, 0);
        let mut dmg = amt.min(i32::MAX as u32) as i32;
        if dmg <= 0 {
            return;
        }
        // Shield quarter (:55700-07): mana pays the reduced hit.
        if self.rivals[ri].shield && self.rivals[ri].mana > 0 {
            dmg /= 4;
            let pay = (dmg as u32).min(self.rivals[ri].mana);
            self.rivals[ri].mana -= pay;
        }
        self.g.ent[i].act_life -= dmg;
        self.g.ent[i].f38 = src; // killer latch (+38)
        self.rivals[ri].regen_stall = 16;
        self.g.snd(17, i);
        // Hate feed (sub_16540 :19686-709): +3000 for the heavy
        // models, +500 base — we key on the source owner only
        // (model split folded to the base rate; APPROX).
        if let Some(shooter) = self.owner_slot_of_source(src) {
            self.rival_add_hate(ri, shooter, 3000);
        }
    }

    /// Resolve a mailbox source id to the attacking wizard's slot:
    /// sources carry the attacker's owner tag directly (our writers
    /// pass id24 through).
    pub(crate) fn owner_slot_of_source(&self, src: u16) -> Option<u8> {
        if src == PLAYER_TARGET {
            return Some(0);
        }
        // A pool slot: use its owner tag; a wizard entity is its own.
        let e = self.g.ent.get(src as usize)?;
        if e.class64 == 3 && e.model65 <= 1 {
            return self.owner_slot(src);
        }
        self.owner_slot(e.id24)
    }

    /// Bump the ledger and raise the war flag past the wealth-scaled
    /// threshold (:19733-39): hate > 50000 - maxmana/10 * agg/255.
    fn rival_add_hate(&mut self, ri: usize, shooter: u8, amount: u16) {
        if shooter as usize >= 8 || self.rivals[ri].slot == shooter {
            return;
        }
        let r = &mut self.rivals[ri];
        r.hate[shooter as usize] = r.hate[shooter as usize].saturating_add(amount);
        let wealth = r.mana_max / 10 * r.agg as u32 / 255;
        let threshold = 50_000u32.saturating_sub(wealth);
        if r.hate[shooter as usize] as u32 > threshold {
            r.war[shooter as usize] = true;
        }
    }

    /// Credit stolen mana to a wizard by owner tag.
    pub(crate) fn credit_wizard_mana(&mut self, owner: u16, amount: u32) {
        if owner == PLAYER_TARGET {
            self.player.mana = (self.player.mana + amount).min(self.player.mana_max);
            return;
        }
        if let Some(r) = self.rivals.iter_mut().find(|r| r.ent == owner) {
            r.mana = (r.mana + amount).min(r.mana_max);
            return;
        }
        if let Some(r) = self.mc2_rivals.iter_mut().find(|r| r.ent == owner) {
            r.mana = (r.mana + amount).min(r.mana_max);
        }
    }

    /// AI carpet movement (sub_14EB0 :18780-859): band-settle
    /// altitude, always-level forward step, lateral dodge, 16/tick
    /// accel toward the desired speed, tempo-scaled turn toward the
    /// desired heading. No wall gate, no drag/knock.
    fn rival_movement(&mut self, ri: usize, i: usize) {
        let row = &BEHAVIOR[self.g.ent[i].row156 as usize];
        let (v10, v12, v14) = (row.v_10, row.v_12, row.v_14);
        let (v2, v4) = (row.v_2, row.v_4);
        let ground = self.g.ground_z(self.g.ent[i].x, self.g.ent[i].y) as i16;
        {
            let e = &mut self.g.ent[i];
            // sub_42000 (:52576): the band settle.
            if e.z > ground.saturating_add(v10) {
                e.z = e.z.saturating_add(v14);
            } else if e.z > ground.saturating_add(v12) {
                e.z = e.z.saturating_add((v14 as i32 * 25 / 100) as i16);
            }
            if e.z < ground.saturating_add(v12) {
                e.z = ground.saturating_add(v12);
            }
        }
        // Forward (always level) + lateral dodge, then commit.
        let (yaw, speed, jink) = {
            let e = &self.g.ent[i];
            (e.f30, e.f126, self.rivals[ri].jink)
        };
        let mut pos = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        Gen::polar_step(&mut pos, yaw, 0, speed);
        if jink != 0 {
            Gen::polar_step(&mut pos, yaw.wrapping_add(0x200) & 0x7FF, 0, jink);
            self.rivals[ri].jink -= 4 * jink.signum();
        }
        self.g.move_relink(i, pos.0, pos.1, pos.2);
        // Accel 16/tick toward the desired speed (:18828-31).
        {
            let vdes = self.rivals[ri].vdes;
            let e = &mut self.g.ent[i];
            e.f126 += 16 * (vdes - e.f126).signum();
            // Turn toward the desired heading (:18835-57): rate =
            // err / (8 + (255-tempo)/16), clamped to the row's caps.
            let err = Gen::angdist(e.f30, e.f34) as i32;
            let div = 8 + ((255 - self.rivals[ri].tempo as i32) / 16);
            let step = (err / div).clamp(v4 as i32, v2 as i32) as i16;
            let t = Gen::turn_step(e.f30, e.f34, step);
            e.f30 = (e.f30 as i32 + t as i32) as u16 & 0x7FF;
        }
    }

    /// Spell learning (:64805-14 arm + sub_15EC0 :19381-443 expiry):
    /// any ground jar of an unowned, allowed spell arms a 200-tick
    /// countdown; at expiry the rival conjures its own manifestation.
    /// (Arming is folded here into the countdown scan — one pool walk
    /// per tick instead of the jar tick writing into every rival.)
    fn rival_learn_tick(&mut self, ri: usize) {
        for s in 0..SPELL_COUNT {
            if self.rivals[ri].known[s] || !self.rivals[ri].allowed[s] {
                continue;
            }
            if self.rivals[ri].learn[s] > 1 {
                self.rivals[ri].learn[s] -= 1;
                continue;
            }
            if self.rivals[ri].learn[s] == 1 {
                // Conjure the copy (off_987DE[s] :19415-31).
                let ent = self.rivals[ri].ent;
                self.rivals[ri].learn[s] = 0;
                self.rivals[ri].known[s] = true;
                if let Some(m) = self.mint_manifestation(s, ent) {
                    self.rivals[ri].owned[s] = m as u16;
                }
                continue;
            }
            // Arm on a matching ground jar existing anywhere
            // (:64805-14; jars have tick70 < MANIFEST_BASE).
            let exists = (1..self.g.ent.len()).any(|j| {
                let e = &self.g.ent[j];
                e.class64 == 12
                    && e.model65 as usize == s
                    && e.tick70 < crate::mc1::world::MANIFEST_BASE
                    && e.flags & 0x400 == 0
            });
            if exists {
                self.rivals[ri].learn[s] = 200;
            }
        }
    }

    /// Buff flags derive from the manifestations' burst counters
    /// (the human's manifestation_tick equivalents; the rival's
    /// bursts are armed by [`World::rival_cast`] and decremented
    /// here).
    fn rival_refresh_buffs(&mut self, ri: usize) {
        let mut get = |spell: usize| -> bool {
            let m = self.rivals[ri].owned[spell] as usize;
            if m == 0 {
                return false;
            }
            if self.g.ent[m].f26 > 0 {
                self.g.ent[m].f26 -= 1;
            }
            self.g.ent[m].f26 > 0
        };
        let shield = get(4);
        let invisible = get(12);
        let rebound = get(14);
        // Heal channel (1): 5% per tick while live, paid per tick.
        let heal_m = self.rivals[ri].owned[1] as usize;
        let healing = heal_m != 0 && self.g.ent[heal_m].f26 > 0;
        if healing {
            let def = &SPELLS[1];
            if self.rivals[ri].mana >= def.possess_mana / def.count as u32 {
                self.rivals[ri].mana -= def.possess_mana / def.count as u32;
                let i = self.rivals[ri].ent as usize;
                let max = self.g.ent[i].max_life as i32;
                self.g.ent[i].act_life = (self.g.ent[i].act_life + max / 20).min(max);
            }
            self.g.ent[heal_m].f26 -= 1;
        }
        // Speed-up (2) burst rides f26 too; consumed by the approach
        // helper's boost checks.
        let m2 = self.rivals[ri].owned[2] as usize;
        if m2 != 0 && self.g.ent[m2].f26 > 0 {
            self.g.ent[m2].f26 -= 1;
        }
        {
            let r = &mut self.rivals[ri];
            r.shield = shield;
            r.invisible = invisible;
            r.rebound = rebound;
        }
        // Mirror the cloak onto the entity's 0x20 bit — the shared
        // draw/targeting suppressor (:65689-90); only while alive
        // (death owns the bit in states 2/3).
        let i = self.rivals[ri].ent as usize;
        if self.g.ent[i].tick70 == 1 {
            if invisible {
                self.g.ent[i].flags |= 0x20;
            } else {
                self.g.ent[i].flags &= !0x20;
            }
        }
    }

    /// Incoming-projectile defense (sub_16800 :19769 + sub_16870/90):
    /// the nearest class-9 homing on me within 5120 → lateral jink 80 +
    /// a reactive cast (models {0,3,16} → 14 Rebound, {4,9} → 4
    /// Shield).
    fn rival_defense(&mut self, ri: usize, i: usize) {
        let me = self.rivals[ri].ent;
        let (px, py, pz) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        let mut best: Option<(usize, i32)> = None;
        for j in 1..self.g.ent.len() {
            let e = &self.g.ent[j];
            if e.class64 != 9 || e.flags & 0x400 != 0 || e.f146 != me {
                continue;
            }
            let d2 = Gen::dist2_sq(px, py, e.x, e.y);
            let dz = e.z.wrapping_sub(pz) as i32;
            let d3 = d2.wrapping_add(dz.wrapping_mul(dz));
            if d3 <= 5120 * 5120 && best.is_none_or(|(_, bd)| d3 < bd) {
                best = Some((j, d3));
            }
        }
        let Some((threat, d3)) = best else { return };
        self.rivals[ri].jink = 80;
        if d3 <= 1024 * 1024 {
            let spell = match self.g.ent[threat].model65 {
                0 | 3 | 16 => 14,
                4 | 9 => 4,
                _ => 4,
            };
            self.rival_cast(ri, i, spell);
        }
    }

    // ---- the decision cascade (sub_136C0 :18048) --------------------------

    fn rival_selector(&mut self, ri: usize, i: usize, think: bool) {
        // 1. Need a castle (sub_13F00 :18345).
        let castle = self.rival_castle(self.rivals[ri].ent);
        if castle.is_none()
            && self.rivals[ri].known[16]
            && self.rivals[ri].mana_max >= SPELLS[16].possess_mana
        {
            if self.rival_scout_site(ri, i) {
                self.rivals[ri].state = AiState::Build;
                return;
            }
        }
        // 2. Flee home hurt (sub_14310 :18480).
        if castle.is_some() && self.g.ent[i].act_life < (self.g.ent[i].max_life / 2) as i32 {
            self.set_rival_state(ri, AiState::Home, 0);
            return;
        }
        if !think {
            return;
        }
        // 3. Upgrade the castle (sub_14120 :18408).
        if let Some(c) = castle {
            let m16 = self.rivals[ri].owned[16] as usize;
            if m16 != 0
                && self.g.ent[m16].f26 == 0
                && self.rivals[ri].cooldown[16] == 0
                && self.g.ent[c].tick70 == 4
                && self.rivals[ri].mana_max
                    >= Gen::CASTLE_CAP[self.g.ent[c].f26.clamp(0, 7) as usize] as u32
                && self.g.castle_upgrade_space_ok(c)
            {
                self.set_rival_state(ri, AiState::Upgrade, 0);
                return;
            }
        }
        // 4. Raid an enemy castle (sub_143A0 :18496).
        if self.rival_has_offense(ri) && self.rival_pick_castle_target(ri, i) {
            self.rivals[ri].state = AiState::RaidCastle;
            return;
        }
        // 5. Attack an enemy wizard (sub_145B0 :18541).
        if self.rival_has_offense(ri) && self.rival_pick_wizard_target(ri, i) {
            self.rivals[ri].state = AiState::AttackWizard;
            return;
        }
        // 6. Intercept a fat enemy balloon (sub_147E0 :18596).
        if self.rival_pick_balloon_target(ri, i) {
            self.rivals[ri].state = AiState::RaidBalloon;
            return;
        }
        // 7. Claim mana balls (sub_14230 :18439-52): needs spell 3;
        // with the castle spell known, only while the ceiling is at
        // or under the castle spell's CURRENT cost — which
        // sub_47DD0 rewrites to the capacity ladder at the standing
        // castle's level, so claiming re-opens after every upgrade
        // (the original's economy loop).
        let castle_cost = castle
            .map(|c| Gen::CASTLE_CAP[self.g.ent[c].f26.clamp(0, 7) as usize] as u32)
            .unwrap_or(SPELLS[16].possess_mana);
        if self.rivals[ri].known[3]
            && (!self.rivals[ri].known[16] || self.rivals[ri].mana_max <= castle_cost)
            && self.rival_pick_ball_target(ri, i)
        {
            self.rivals[ri].state = AiState::Possess;
            return;
        }
        // 8. Hunt any mana holder (sub_14B10 :18650).
        if self.rival_pick_mana_target(ri, i) {
            self.rivals[ri].state = AiState::HuntMana;
            return;
        }
        // 9. Idle (sub_14DC0 :18749).
        if castle.is_some() && self.g.ent[i].act_life < self.g.ent[i].max_life as i32 {
            self.set_rival_state(ri, AiState::Home, 0);
        } else {
            self.rivals[ri].state = AiState::Cruise;
        }
    }

    fn set_rival_state(&mut self, ri: usize, s: AiState, target: u16) {
        self.rivals[ri].state = s;
        self.rivals[ri].target = target;
        self.rivals[ri].target_sig = self.target_sig(target);
    }

    /// Target signature (sub_15420 :19039): team + model + class<<7.
    fn target_sig(&self, target: u16) -> u16 {
        if target == 0 {
            return 0;
        }
        if target == PLAYER_TARGET {
            return PLAYER_TARGET;
        }
        let e = &self.g.ent[target as usize];
        e.id24
            .wrapping_add(e.model65 as u16)
            .wrapping_add((e.class64 as u16) << 7)
    }

    /// Target staleness (sub_15440 :19044).
    fn target_alive(&self, target: u16, sig: u16) -> bool {
        if target == 0 {
            return false;
        }
        if target == PLAYER_TARGET {
            return self.player.state == LifeState::Alive;
        }
        let e = &self.g.ent[target as usize];
        e.flags & 0x400 == 0 && e.act_life >= 0 && self.target_sig(target) == sig
    }

    /// Castle-site scout (sub_13F00 :18358-402): 4x4-supercell sweep,
    /// site OK when the nearest foreign castle is > 12288 away.
    /// (Original walks 2 candidates per supercell from its own cell;
    /// ours walks the same grid exhaustively — same acceptance rule.)
    fn rival_scout_site(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (sx, sy) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best: Option<((u16, u16), i32)> = None;
        for cell in 0..16u16 {
            let bx = ((sx >> 14).wrapping_add(cell & 3) & 3) << 14;
            let by = ((sy >> 14).wrapping_add(cell >> 2) & 3) << 14;
            for (ox, oy) in [(0u16, 0u16), (0x1F00, 0x1F00)] {
                let (tx, ty) = (
                    bx.wrapping_add(ox).wrapping_add(128),
                    by.wrapping_add(oy).wrapping_add(128),
                );
                // Nearest foreign castle.
                let mut near = i32::MAX;
                for j in 1..self.g.ent.len() {
                    let e = &self.g.ent[j];
                    if e.class64 == 3 && e.model65 == 2 && e.flags & 0x400 == 0 && e.id24 != me {
                        near = near.min(Gen::dist2_sq(tx, ty, e.x, e.y));
                    }
                }
                if near > 12288 * 12288 {
                    let d = Gen::dist2_sq(sx, sy, tx, ty);
                    if best.is_none_or(|(_, bd)| d < bd) {
                        best = Some(((tx, ty), d));
                    }
                }
            }
        }
        if let Some(((tx, ty), _)) = best {
            self.rivals[ri].site = (tx, ty);
            true
        } else {
            false
        }
    }

    /// Any offense spell owned (sub_16920 :19856: {0,15,8,17,20,7}).
    fn rival_has_offense(&self, ri: usize) -> bool {
        [0usize, 15, 8, 17, 20, 7]
            .iter()
            .any(|&s| self.rivals[ri].owned[s] != 0)
    }

    /// The hate gate (:18514 etc.): hate[owner] over the wealth-
    /// scaled threshold.
    fn hate_over(&self, ri: usize, slot: u8, wealth: u32) -> bool {
        let r = &self.rivals[ri];
        let threshold = 50_000u32.saturating_sub(wealth / 10 * r.agg as u32 / 255);
        r.hate[slot as usize] as u32 > threshold
    }

    /// Enemy-castle pick (sub_143A0 :18496-536): hated-and-undefended
    /// or plain poorer, nearest in range.
    fn rival_pick_castle_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let my_castle = self.rival_castle(me);
        let my_stored = my_castle.map_or(0, |c| self.g.ent[c].f140.max(0) as u32);
        // Skip while castle-less but castle-capable (:18507).
        if my_castle.is_none() && self.rivals[ri].known[16] {
            return false;
        }
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32;
        let mut best: Option<(u16, i32)> = None;
        for j in 1..self.g.ent.len() {
            let e = &self.g.ent[j];
            if e.class64 != 3 || e.model65 != 2 || e.flags & 0x400 != 0 || e.id24 == me {
                continue;
            }
            let Some(owner) = self.owner_slot(e.id24) else {
                continue;
            };
            let owner_wealth = self.wizard_wealth(owner);
            let hated = self.hate_over(ri, owner, owner_wealth);
            // Undefended: the owner is over 7680 away (:18517-22).
            let undefended = self
                .wizard_pos(owner)
                .is_none_or(|(wx, wy, _)| Gen::dist2_sq(e.x, e.y, wx, wy) > 7680 * 7680);
            let poorer = (e.f140.max(0) as u32)
                .saturating_add(640 * (255 - self.rivals[ri].agg as u32))
                < my_stored;
            if !(hated && undefended) && !poorer {
                continue;
            }
            let d = Gen::dist2_sq(px, py, e.x, e.y);
            if d <= range.saturating_mul(range) && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::RaidCastle, t);
            true
        } else {
            false
        }
    }

    /// Enemy-wizard pick (sub_145B0 :18541-91).
    fn rival_pick_wizard_target(&mut self, ri: usize, i: usize) -> bool {
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32 + 10;
        let my_mana = self.rivals[ri].mana;
        let mut best: Option<(u16, i32)> = None;
        let consider = |slot: u8,
                        tgt: u16,
                        x: u16,
                        y: u16,
                        invisible: bool,
                        castle_less: bool,
                        wealth: u32,
                        mana: u32,
                        best: &mut Option<(u16, i32)>| {
            if invisible {
                return; // spell-12 targets are skipped (:18558)
            }
            let war = self.rivals[ri].war[slot as usize];
            let hated = self.hate_over(ri, slot, wealth);
            // Bully the homeless rich (:18570-77).
            let bully = castle_less
                && mana.saturating_add(32 * (255 - self.rivals[ri].agg as u32)) < my_mana;
            if !war && !hated && !bully {
                return;
            }
            let d = Gen::dist2_sq(px, py, x, y);
            if d <= range.saturating_mul(range) && best.is_none_or(|(_, bd)| d < bd) {
                *best = Some((tgt, d));
            }
        };
        // The human.
        if self.player.state == LifeState::Alive {
            let (hx, hy) = (self.human_pose.0, self.human_pose.1);
            consider(
                0,
                PLAYER_TARGET,
                hx,
                hy,
                self.player.invisible,
                self.player_castle().is_none(),
                self.player.mana_max,
                self.player.mana,
                &mut best,
            );
        }
        // Other rivals.
        for oj in 0..self.rivals.len() {
            if oj == ri || self.rivals[oj].eliminated {
                continue;
            }
            let o = &self.rivals[oj];
            let e = &self.g.ent[o.ent as usize];
            if e.tick70 != 1 {
                continue; // dead/falling wizards aren't targets
            }
            consider(
                o.slot,
                o.ent,
                e.x,
                e.y,
                o.invisible,
                self.rival_castle(o.ent).is_none(),
                o.mana_max,
                o.mana,
                &mut best,
            );
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::AttackWizard, t);
            true
        } else {
            false
        }
    }

    /// Enemy-balloon pick (sub_147E0 :18596-645): hated owner, cargo
    /// over 10*(275-agg), away from its castle.
    fn rival_pick_balloon_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let range = BEHAVIOR[self.g.ent[i].row156 as usize].v_28 as i32;
        let cargo_gate = 10 * (275 - self.rivals[ri].agg as u32);
        let mut best: Option<(u16, i32)> = None;
        for j in 1..self.g.ent.len() {
            let e = &self.g.ent[j];
            if e.class64 != 3 || e.model65 != 3 || e.flags & 0x400 != 0 || e.id24 == me {
                continue;
            }
            let Some(owner) = self.owner_slot(e.id24) else {
                continue;
            };
            if !self.hate_over(ri, owner, self.wizard_wealth(owner)) {
                continue;
            }
            if (e.f140.max(0) as u32) <= cargo_gate {
                continue;
            }
            // Not sitting at its own castle (:18628-33).
            let home = self
                .rival_castle(e.id24)
                .or_else(|| (owner == 0).then(|| self.player_castle()).flatten());
            if home.is_some_and(|c| {
                Gen::dist2_sq(e.x, e.y, self.g.ent[c].x, self.g.ent[c].y) < 2048 * 2048
            }) {
                continue;
            }
            let d = Gen::dist2_sq(px, py, e.x, e.y);
            if d <= range.saturating_mul(range) && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::RaidBalloon, t);
            true
        } else {
            false
        }
    }

    /// Mana-ball pick (sub_15080 :18862): wild balls by distance;
    /// at-war owners' balls; neutral-owned only if unguarded.
    fn rival_pick_ball_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let mut best: Option<(u16, i32)> = None;
        for j in 1..self.g.ent.len() {
            let e = &self.g.ent[j];
            if e.class64 != 10 || e.model65 != 39 || e.flags & 0x400 != 0 {
                continue;
            }
            if e.f144 == me {
                continue; // already mine
            }
            let owner = self.owner_slot(e.f144);
            match owner {
                None => {} // wild: always eligible
                Some(o) => {
                    let at_war = self.rivals[ri].war[o as usize]
                        || self.hate_over(ri, o, self.wizard_wealth(o));
                    if !at_war {
                        // Neutral-owned: only if unguarded — no owner
                        // wizard within 5120 (:18905-16).
                        let guarded = self.wizard_pos(o).is_some_and(|(wx, wy, _)| {
                            Gen::dist2_sq(e.x, e.y, wx, wy) < 5120 * 5120
                        });
                        if guarded {
                            continue;
                        }
                    }
                }
            }
            let d = Gen::dist2_sq(px, py, e.x, e.y);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::Possess, t);
            true
        } else {
            false
        }
    }

    /// Mana-holder hunt (sub_14B10 :18650): any other-team creature
    /// with mana, nearest to the own castle (or self), no range cap.
    fn rival_pick_mana_target(&mut self, ri: usize, i: usize) -> bool {
        let me = self.rivals[ri].ent;
        let anchor = self
            .rival_castle(me)
            .map(|c| (self.g.ent[c].x, self.g.ent[c].y))
            .unwrap_or((self.g.ent[i].x, self.g.ent[i].y));
        let mut best: Option<(u16, i32)> = None;
        for j in 1..self.g.ent.len() {
            let e = &self.g.ent[j];
            if e.class64 != 5 || e.flags & 0x400 != 0 || e.act_life < 0 || e.tick70 == 120 {
                continue;
            }
            if e.id24 == me || e.f140 <= 0 {
                continue;
            }
            let d = Gen::dist2_sq(anchor.0, anchor.1, e.x, e.y);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j as u16, d));
            }
        }
        if let Some((t, _)) = best {
            self.set_rival_state(ri, AiState::HuntMana, t);
            true
        } else {
            false
        }
    }

    /// A wizard's live position by slot (0 = the human).
    pub(crate) fn wizard_pos(&self, slot: u8) -> Option<(u16, u16, i16)> {
        if slot == 0 {
            return (self.player.state == LifeState::Alive).then_some(self.human_pose);
        }
        let ent = self
            .rivals
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.ent)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.slot == slot)
                    .map(|r| r.ent)
            })?;
        let e = &self.g.ent[ent as usize];
        (e.tick70 == 1).then_some((e.x, e.y, e.z))
    }

    /// A wizard's mana ceiling (the wealth term in the hate gates).
    pub(crate) fn wizard_wealth(&self, slot: u8) -> u32 {
        if slot == 0 {
            return self.player.mana_max;
        }
        self.rivals
            .iter()
            .find(|r| r.slot == slot)
            .map(|r| r.mana_max)
            .or_else(|| {
                self.mc2_rivals
                    .iter()
                    .find(|r| r.slot == slot)
                    .map(|r| r.mana_max)
            })
            .unwrap_or(0)
    }

    // ---- state handlers -----------------------------------------------

    fn rival_state_tick(&mut self, ri: usize, i: usize, think: bool) {
        // Combat states drop stale targets back to Fresh.
        let needs_target = matches!(
            self.rivals[ri].state,
            AiState::Possess
                | AiState::RaidCastle
                | AiState::AttackWizard
                | AiState::RaidBalloon
                | AiState::HuntMana
        );
        if needs_target && !self.target_alive(self.rivals[ri].target, self.rivals[ri].target_sig) {
            self.rivals[ri].state = AiState::Fresh;
            self.rivals[ri].target = 0;
            return;
        }
        match self.rivals[ri].state {
            AiState::Fresh => {}
            // Fly home; cast 0x10 on arrival = the upgrade chain
            // (sub_13800 :18106-32).
            AiState::Upgrade => {
                let Some(c) = self.rival_castle(self.rivals[ri].ent) else {
                    self.rivals[ri].state = AiState::Fresh;
                    return;
                };
                let (cx, cy, cz) = {
                    let e = &self.g.ent[c];
                    (e.x, e.y, e.z)
                };
                if self.rival_approach(ri, i, cx, cy, 512, 2048) {
                    self.rival_hover_toward(i, cz.saturating_add(512));
                    self.rival_cast(ri, i, 16);
                }
            }
            // Fly to the scouted site; plant (sub_138F0 :18142-68).
            AiState::Build => {
                let (sx, sy) = self.rivals[ri].site;
                if self.rival_approach(ri, i, sx, sy, 2048, 3072) {
                    self.rival_cast(ri, i, 16);
                    if self.rival_castle(self.rivals[ri].ent).is_some() {
                        self.rivals[ri].state = AiState::Fresh;
                    }
                }
            }
            // Claim the ball (sub_13BA0 :18236-57): approach, cast 3,
            // and inside ~5 degrees write the claim directly.
            AiState::Possess => {
                let t = self.rivals[ri].target as usize;
                let (tx, ty) = (self.g.ent[t].x, self.g.ent[t].y);
                if self.rival_approach(ri, i, tx, ty, 1024, 3072) {
                    let cast = self.rival_cast(ri, i, 3);
                    let facing = Gen::angdist(
                        self.g.ent[i].f30,
                        Gen::angle_between(self.g.ent[i].x, self.g.ent[i].y, tx, ty),
                    );
                    if cast && facing <= 28 {
                        self.g.ent[t].f144 = self.rivals[ri].ent;
                        self.g.snd(4, t); // the claim chime (:29444)
                        self.rivals[ri].state = AiState::Fresh;
                    }
                }
            }
            // Castle raid (sub_13CA0 :18271-92).
            AiState::RaidCastle => {
                let t = self.rivals[ri].target as usize;
                let (tx, ty, tz) = {
                    let e = &self.g.ent[t];
                    (e.x, e.y, e.z)
                };
                self.rival_face_target(i, tx, ty, tz);
                self.rival_approach(ri, i, tx, ty, 2048, 3584);
                if think {
                    if let Some(s) = self.rival_attack_pick(ri, false) {
                        self.rival_cast(ri, i, s);
                    }
                }
            }
            // Wizard / balloon / mana-holder attack (sub_13DC0
            // :18314-40): burst-gated.
            AiState::AttackWizard | AiState::RaidBalloon | AiState::HuntMana => {
                let (tx, ty, tz) = match self.rivals[ri].target {
                    PLAYER_TARGET => self.human_pose,
                    t => {
                        let e = &self.g.ent[t as usize];
                        (e.x, e.y, e.z)
                    }
                };
                self.rival_face_target(i, tx, ty, tz);
                self.rival_approach(ri, i, tx, ty, 3072, 4096);
                self.rival_hover_toward(i, tz.saturating_add(512));
                if self.rivals[ri].burst >= 0 {
                    if let Some(s) = self.rival_attack_pick(ri, true) {
                        if self.rival_cast(ri, i, s) && self.rivals[ri].target == PLAYER_TARGET {
                            // Landing a cast clears the war flag
                            // toward that wizard (:18338-39).
                            self.rivals[ri].war[0] = false;
                        }
                    }
                }
            }
            // Home (sub_13A70 :18204-27): cloak while fleeing; the
            // teleport-home attempt is authentically dead code.
            AiState::Home => {
                let Some(c) = self.rival_castle(self.rivals[ri].ent) else {
                    self.rival_cast(ri, i, 12);
                    self.rivals[ri].state = AiState::Cruise;
                    return;
                };
                let (cx, cy) = (self.g.ent[c].x, self.g.ent[c].y);
                self.rival_cast(ri, i, 12);
                self.rival_approach(ri, i, cx, cy, 256, 2048);
                if self.g.ent[i].act_life >= self.g.ent[i].max_life as i32 {
                    self.rivals[ri].state = AiState::Fresh;
                }
            }
            // Cruise (sub_13A10 :18188): full throttle, heading held.
            AiState::Cruise => {
                self.rivals[ri].vdes = self.g.ent[i].f128;
            }
        }
    }

    /// Shared travel helper (sub_15470 :19050-94): inside arriveR →
    /// stop, done; else full speed, and beyond boostR cast the
    /// speed-up. Returns "arrived".
    fn rival_approach(
        &mut self,
        ri: usize,
        i: usize,
        tx: u16,
        ty: u16,
        arrive: i32,
        boost: i32,
    ) -> bool {
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        let d2 = Gen::dist2_sq(px, py, tx, ty);
        self.g.ent[i].f34 = Gen::angle_between(px, py, tx, ty);
        if d2 <= arrive.saturating_mul(arrive) {
            self.rivals[ri].vdes = 0;
            return true;
        }
        self.rivals[ri].vdes = self.g.ent[i].f128;
        if d2 > boost.saturating_mul(boost) {
            self.rival_cast(ri, i, 2);
        }
        false
    }

    /// Aim the body at the target (desired yaw; the commit pitch is
    /// set at cast time, :19125-27).
    fn rival_face_target(&mut self, i: usize, tx: u16, ty: u16, _tz: i16) {
        let (px, py) = (self.g.ent[i].x, self.g.ent[i].y);
        self.g.ent[i].f34 = Gen::angle_between(px, py, tx, ty);
    }

    /// Per-state altitude nudge toward target z + 512 (:18122-27).
    fn rival_hover_toward(&mut self, i: usize, tz: i16) {
        let row = &BEHAVIOR[self.g.ent[i].row156 as usize];
        let step = row.v_14.abs().max(1);
        let e = &mut self.g.ent[i];
        if e.z < tz {
            e.z = e.z.saturating_add(step);
        } else if e.z > tz {
            e.z = e.z.saturating_sub(step);
        }
    }

    /// The attack-spell picker (sub_16030 :19459 / castle variant
    /// sub_16310 :19559): poverty latch, then the priority walk
    /// 17 → 8 → (anti-rebound 15) → 7 → 20 → 0 → 15. Returns the
    /// spell to cast now; None = hold (save up or poor).
    fn rival_attack_pick(&mut self, ri: usize, vs_wizard: bool) -> Option<usize> {
        // Poverty latch (:19468-91).
        {
            let r = &mut self.rivals[ri];
            if r.mana < r.mana_max / 4 {
                r.poverty = true;
            } else if r.poverty {
                let release = (r.mana_max / 4 + 6000).min(r.mana_max / 2);
                if r.mana > release {
                    r.poverty = false;
                }
            }
            if r.poverty {
                return None;
            }
        }
        // Anti-rebound notice (:19507-16): the target visibly
        // rebounding switches the plan to lightning, acc% of the
        // time.
        let target_rebounds = vs_wizard
            && match self.rivals[ri].target {
                PLAYER_TARGET => self.player.rebound,
                t => self
                    .rivals
                    .iter()
                    .find(|r| r.ent == t)
                    .is_some_and(|r| r.rebound),
            };
        let mut order: Vec<usize> = vec![17, 8];
        if target_rebounds {
            let roll = (self.g.ent_rand(self.rivals[ri].ent as usize) % 255) as u16;
            if roll < self.rivals[ri].acc {
                order.push(15);
            }
        }
        order.extend([7, 20, 0, 15]);
        for s in order {
            if self.rivals[ri].owned[s] == 0 {
                continue;
            }
            if self.rival_cast_ready(ri, s) {
                return Some(s);
            }
            // Affordable by ceiling but cooling/poor → save up and
            // WAIT (:19527-40).
            if self.rivals[ri].mana_max >= self.spells()[s].possess_mana {
                return None;
            }
        }
        None
    }

    // ---- the cast arm (readiness sub_15A00 :19219 + executor
    // ---- sub_155F0 :19096) ------------------------------------------

    /// Readiness: owned + cooldown clear + mana covers the cost +
    /// castle-stored threshold + (aimed groups) the accuracy-scaled
    /// aim cone.
    fn rival_cast_ready(&self, ri: usize, s: usize) -> bool {
        let r = &self.rivals[ri];
        let m = r.owned[s] as usize;
        if m == 0 || r.cooldown[s] != 0 {
            return false;
        }
        let def = &self.spells()[s];
        if r.mana < def.possess_mana {
            return false;
        }
        // The castle-stored unlock ladder (sub_55DD0 :64917-19) —
        // shared with the human.
        if def.castle_req > 0
            && !self
                .rival_castle(r.ent)
                .is_some_and(|c| self.g.ent[c].f140.max(0) as u32 >= def.castle_req)
        {
            return false;
        }
        // Aimed groups: the readiness pre-gate cone
        // ((255-acc)/4+20 degrees, :19252-57).
        if matches!(s, 0 | 3 | 7 | 8 | 11 | 13 | 15 | 17 | 20) && r.target != 0 {
            let cone = ((255 - r.acc as u32) / 4 + 20) * 2048 / 360;
            let e = &self.g.ent[r.ent as usize];
            let (tx, ty) = match r.target {
                PLAYER_TARGET => (self.human_pose.0, self.human_pose.1),
                t => (self.g.ent[t as usize].x, self.g.ent[t as usize].y),
            };
            let want = Gen::angle_between(e.x, e.y, tx, ty);
            if Gen::angdist(e.f30, want) as u32 > cone {
                return false;
            }
        }
        true
    }

    /// The commit (sub_155F0 :19096-215): arm the cooldown, aim the
    /// pitch at the target, run the burst counter, debit the mana
    /// through the delta, and emit through the shared spawners.
    /// Returns true when the cast fired. Spells 18/19/21/22/23 hit
    /// the original's default case — the AI can never cast them.
    fn rival_cast(&mut self, ri: usize, i: usize, s: usize) -> bool {
        if s >= SPELL_COUNT || matches!(s, 18 | 19 | 21 | 22 | 23) {
            return false;
        }
        if !self.rival_cast_ready(ri, s) {
            return false;
        }
        let def = &self.spells()[s];
        // Group gates beyond readiness (:19113-19209).
        let (tx, ty, tz) = match self.rivals[ri].target {
            0 => {
                let e = &self.g.ent[i];
                let mut fwd = (e.x, e.y, e.z);
                Gen::polar_step(&mut fwd, e.f30, 0, 4096);
                fwd
            }
            PLAYER_TARGET => self.human_pose,
            t => {
                let e = &self.g.ent[t as usize];
                (e.x, e.y, e.z)
            }
        };
        let (ex, ey, ez, yaw) = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z, e.f30)
        };
        let want = Gen::angle_between(ex, ey, tx, ty);
        match s {
            // Precision-aimed burst pair (:19113-37).
            0 | 15 => {
                if self.rivals[ri].burst < 0 || Gen::angdist(yaw, want) > 0xAA {
                    return false;
                }
                self.rivals[ri].burst += 1;
                if self.rivals[ri].burst >= 8 {
                    // Negative lockout (:19129-36).
                    self.rivals[ri].burst = ((self.rivals[ri].tempo as i32 - 255) / 8 - 1) as i16;
                }
            }
            // Aimed group (:19158-77): the wider cone.
            3 | 7 | 8 | 11 | 13 | 17 | 20 => {
                if Gen::angdist(yaw, want) > 0xE3 {
                    return false;
                }
            }
            _ => {}
        }
        // Castle (0x10): with a castle → the upgrade chain; without →
        // the free direct plant at the site (:19190-209).
        if s == 16 {
            return self.rival_cast_castle(ri, i);
        }
        // Arm the re-attempt cooldown + the manifestation burst.
        self.rivals[ri].cooldown[s] = AI_RECAST[s];
        let m = self.rivals[ri].owned[s] as usize;
        self.g.ent[m].f26 = def.count as i16;
        // The debit rides the regen delta (sub_55E80 :64936 — the
        // authored behavior; remc1 ships it commented out).
        {
            let r = &mut self.rivals[ri];
            let c = def.possess_mana.min(i32::MAX as u32) as i32;
            r.mana_delta = if r.mana_delta >= 0 {
                -c
            } else {
                r.mana_delta - c
            };
        }
        // Absolute aim pitch to the target (:19125-27).
        let dh = Gen::isqrt(Gen::dist2_sq(ex, ey, tx, ty) as u32) as i32;
        let pitch = Gen::pitch_toward(ez, tz, dh);
        // Emission — the AI launches dead-center (no hand offset,
        // :64963: neither hand bit set) at carpet height + half
        // extent.
        let mz = ez.wrapping_add(self.g.ent[i].f78 as i16);
        self.rival_emit(ri, i, s, ex, ey, mz, yaw, pitch);
        true
    }

    /// Rival castle cast (:19190-209).
    fn rival_cast_castle(&mut self, ri: usize, i: usize) -> bool {
        self.rivals[ri].cooldown[16] = AI_RECAST[16];
        if let Some(c) = self.rival_castle(self.rivals[ri].ent) {
            // The upgrade: full next-stage cost through the shared
            // ladder, token chain approximated by the direct level-up
            // (the (9,10)→(10,43) ball ride is cosmetic here; the
            // painter is the same).
            let lvl = self.g.ent[c].f26.clamp(0, 7) as usize;
            let cost = Gen::CASTLE_CAP[lvl] as u32;
            if self.rivals[ri].mana < cost || !self.g.castle_upgrade_space_ok(c) {
                return false;
            }
            {
                let r = &mut self.rivals[ri];
                let ci = cost.min(i32::MAX as u32) as i32;
                r.mana_delta = if r.mana_delta >= 0 {
                    -ci
                } else {
                    r.mana_delta - ci
                };
            }
            // The token delivery (sub_293D0 :31033-34): ch5 mail
            // {10, owner} — the castle tick's case-0 runs the whole
            // level-up (preclear, ladder, painter). The (9,10) ball
            // ride is skipped (cosmetic; APPROX).
            self.g.ent[c].mail[5] = (10, self.rivals[ri].ent);
            return true;
        }
        // Castle-less: the FREE direct plant at the scouted site
        // (:19200-08) — no debit, no projectile.
        let (sx, sy) = self.rivals[ri].site;
        if sx == 0 && sy == 0 {
            return false;
        }
        let gz = self.g.ground_z(sx, sy) as i16;
        let Some(c) = self.g.spawn_class3(2, sx, sy, gz) else {
            return false;
        };
        {
            let e = &mut self.g.ent[c];
            e.id24 = self.rivals[ri].ent;
            e.f26 = 0;
            e.tick70 = 4;
        }
        self.g.set_sprite(c, 177 + self.rivals[ri].slot as u16);
        let (cx, cy, cz) = {
            let e = &self.g.ent[c];
            (
                ((e.x as u32 + 128) >> 8) as u8,
                ((e.y as u32 + 128) >> 8) as u8,
                (e.z >> 5) as i32,
            )
        };
        self.g.stamp_castle_terrain(1, cx, cy, cz);
        self.g.castle_extents(c, 0);
        self.g.ent[c].f136 = Gen::CASTLE_CAP[0];
        self.g.snd(30, c);
        self.terrain_dirty = true;
        self.entities_dirty = true;
        let _ = i;
        true
    }

    /// The per-spell emissions through the shared spawners, owner =
    /// the rival's entity slot (so friendly-fire immunity, homing and
    /// the damage plumbing all Just Work).
    #[allow(clippy::too_many_arguments)]
    fn rival_emit(
        &mut self,
        ri: usize,
        i: usize,
        s: usize,
        x: u16,
        y: u16,
        z: i16,
        yaw: u16,
        pitch: u16,
    ) {
        let owner = self.rivals[ri].ent;
        let target = self.rivals[ri].target;
        let def = &self.spells()[s];
        let speed = self.g.ent[i].f126;
        let snd = match s {
            0 | 11 | 13 | 17 | 20 => Some(9u8),
            7 | 8 => Some(15),
            2 => Some(19),
            15 => Some(23),
            1 => Some(25),
            3 => Some(40),
            _ => None,
        };
        if let Some(id) = snd {
            self.g.snd(id, i);
        }
        let pr = match s {
            0 => self.g.spawn_fireball(x, y, z),
            3 => self.g.spawn_spell_lob(1, x, y, z),
            7 => self.g.spawn_trail_bolt(x, y, z),
            8 => self.g.spawn_spell_lob(4, x, y, z),
            11 => self.g.spawn_spell_lob(7, x, y, z),
            13 => self.g.spawn_seeker(x, y, z),
            15 => self.g.spawn_zigzag(x, y, z),
            17 => self.g.spawn_spell_lob(11, x, y, z),
            20 => self.g.spawn_spell_lob(9, x, y, z),
            // Self-buffs/channels have no projectile.
            _ => None,
        };
        let Some(pr) = pr else { return };
        let e = &mut self.g.ent[pr];
        e.f126 += speed;
        e.f128 = e.f126;
        e.id24 = owner;
        e.f30 = yaw;
        e.f34 = yaw;
        e.f32 = pitch;
        e.f36 = pitch;
        e.f44 = def.damage.min(u16::MAX as u32) as u16;
        e.f140 = def.possess_mana as i32;
        // Live homing target — the class-9 re-acquire keeps it warm.
        if target != 0 {
            e.f146 = target;
            if target == PLAYER_TARGET {
                // Being targeted arms the danger music (:64013/:64095).
                self.g.player_danger = 100;
            }
        }
        match s {
            3 => {
                let e = &mut self.g.ent[pr];
                e.f68 = 10;
                e.f69 = 12;
                e.f66 = 10;
                e.f26 = 200;
                e.f80 *= 2;
                e.f82 *= 2;
                e.f84 *= 2;
            }
            7 => self.g.ent[pr].f69 = 17,
            13 => {
                let e = &mut self.g.ent[pr];
                e.f44 = 2000;
                e.f69 = 25;
            }
            15 => self.g.ent[pr].f69 = 23,
            _ => {}
        }
        self.entities_dirty = true;
    }

    // ---- mortality ------------------------------------------------------

    /// State 2 — the death fall (sub_45FC0 :55434): drift on, gravity
    /// -2/tick (min -256), a (10,1) fire-trail puff per tick; ground
    /// contact runs the impact block.
    fn rival_death_fall(&mut self, ri: usize, i: usize) {
        {
            let e = &mut self.g.ent[i];
            e.f46 = (e.f46 - 2).max(-256);
        }
        let (yaw, speed, vz) = {
            let e = &self.g.ent[i];
            (e.f30, e.f126, e.f46)
        };
        let mut pos = {
            let e = &self.g.ent[i];
            (e.x, e.y, e.z)
        };
        Gen::polar_step(&mut pos, yaw, 0, speed);
        pos.2 = pos.2.saturating_add(vz);
        let ground = self.g.ground_z(pos.0, pos.1) as i16;
        // The trail (10,1) burning puff.
        if let Some(s) = self.g.spawn_effect(1, pos.0, pos.1, pos.2) {
            self.g.ent[s].flags |= 0x80 | 0x10000;
            self.g.ent[s].id24 = self.rivals[ri].ent;
        }
        if pos.2 <= ground.saturating_add(128) {
            pos.2 = ground.saturating_add(128);
            self.g.move_relink(i, pos.0, pos.1, pos.2);
            self.rival_death_impact(ri, i);
        } else {
            self.g.move_relink(i, pos.0, pos.1, pos.2);
        }
        self.entities_dirty = true;
    }

    /// The impact block (:55488-568): kill credit, jar scatter, the
    /// grave, in-flight balls re-pointed, entity hidden, respawn
    /// timer armed.
    fn rival_death_impact(&mut self, ri: usize, i: usize) {
        // Kill credit (:55488-97): the killer wizard's tally.
        let killer = self.g.ent[i].f38;
        if let Some(k) = self.owner_slot_of_source(killer) {
            self.kill_tally[k as usize][self.rivals[ri].slot as usize] += 1;
            // A rival kill feeds the human's counter too (parity
            // with the creature kill counter track).
            if k == 0 {
                self.g.kills = self.g.kills.saturating_add(1);
            }
        }
        // Death message for the app ticker + toast (retail etext 54
        // = "has died." rendered "<Name> has died.", periods=100 —
        // :55499-517 + the drawType-0 sprintf :26518-33; NOT etext 56
        // "is dead", the wrong neighbor).
        let slot = self.rivals[ri].slot;
        self.rival_deaths.push(slot);
        let name = RIVAL_NAMES.get(slot as usize).copied().unwrap_or("?");
        self.set_notification(format!("{name} has died."), 100, [0xFF, 0, 0]);
        // JAR SCATTER (:55519-49): every owned manifestation detaches
        // into a decaying ground jar around the corpse.
        let (cx, cy) = (self.g.ent[i].x, self.g.ent[i].y);
        for s in 0..SPELL_COUNT {
            let m = self.rivals[ri].owned[s] as usize;
            self.rivals[ri].owned[s] = 0;
            if m == 0 {
                continue;
            }
            let dx = (self.g.ent_rand(m) & 0x1FF) as i32 - 256;
            let dy = (self.g.ent_rand(m) & 0x1FF) as i32 - 256;
            let jx = (cx as i32 + dx) as u16;
            let jy = (cy as i32 + dy) as u16;
            let jz = self.g.ground_z(jx, jy) as i16;
            let life = (self.g.ent_rand(m) % 90 + 200) as i16;
            {
                let e = &mut self.g.ent[m];
                e.tick70 = crate::mc1::world::DROPPED_JAR; // pickup-able, decaying
                e.f144 = 0; // no owner — a free copy
                e.f26 = life; // the decay countdown
            }
            self.g.link(m, jx, jy, jz);
        }
        // The grave (10,40) + in-flight ball re-point (:55550-65).
        let gz = self.g.ground_z(cx, cy) as i16;
        if let Some(gv) = self.g.spawn_grave(cx, cy, gz) {
            let me = self.rivals[ri].ent;
            for j in 1..self.g.ent.len() {
                let e = &mut self.g.ent[j];
                if e.class64 == 10 && e.model65 == 39 && e.flags & 0x400 == 0 && e.f144 == me {
                    e.f144 = gv as u16;
                }
            }
        }
        // Hidden (flag 0x20, :55568) + state 3 + the respawn timer
        // (:55552-57): 32*((255-tempo)/8)+32 ticks.
        {
            let e = &mut self.g.ent[i];
            e.tick70 = 3;
            e.flags = (e.flags | 0x20) & !8; // hidden + unhittable
            e.f26 = (32 * ((255 - self.rivals[ri].tempo as i32) / 8) + 32) as i16;
        }
        // Post-death truce: everyone's hate toward this slot decays
        // from the elevated baseline once it respawns (:55037-41 —
        // set at re-init).
        self.entities_dirty = true;
    }

    /// State 3 — dead on the ground (sub_46480 :55594): with a
    /// castle, count down and re-init; castle-less = ELIMINATED
    /// (checked every tick — losing the castle during the wait
    /// counts, :55622).
    fn rival_dead_wait(&mut self, ri: usize, i: usize) {
        if self.rival_castle(self.rivals[ri].ent).is_none() {
            // The FINAL-death broadcast (retail etext 62 via the
            // opcode-0x1D elimination arm, :48812-25: "<Name> has
            // been eliminated from the realm.", periods=100 — MC1
            // says "eliminated" where MC2 says "banished"). Once, on
            // the elimination edge.
            if !self.rivals[ri].eliminated {
                let slot = self.rivals[ri].slot;
                let name = RIVAL_NAMES.get(slot as usize).copied().unwrap_or("?");
                self.set_notification(
                    format!("{name} has been eliminated from the realm."),
                    100,
                    [0xFF, 0, 0],
                );
            }
            self.rivals[ri].eliminated = true;
            // The husk stays hidden; property persists (:55622).
            return;
        }
        if self.g.ent[i].f26 > 0 {
            self.g.ent[i].f26 -= 1;
            return;
        }
        self.rival_respawn(ri, i);
    }

    /// Re-init at the castle (sub_44D30 respawn arm :54857-64 +
    /// :55019-50): teleport to the castle, full life, base mana,
    /// grace 100, re-mint the remembered book, brain reset, truce.
    fn rival_respawn(&mut self, ri: usize, i: usize) {
        let Some(c) = self.rival_castle(self.rivals[ri].ent) else {
            return;
        };
        let (cx, cy) = (self.g.ent[c].x, self.g.ent[c].y);
        let z = (self.g.ground_z(cx, cy) as i16).saturating_add(256);
        {
            let e = &mut self.g.ent[i];
            e.flags = (e.flags & !0x20) | 8;
            e.tick70 = 1;
            e.f46 = 0;
            e.f126 = 0;
        }
        self.g.move_relink(i, cx, cy, z);
        self.g.refill_life(i);
        let ent = self.rivals[ri].ent;
        let known = self.rivals[ri].known;
        for (s, &k) in known.iter().enumerate() {
            if k && self.rivals[ri].owned[s] == 0 {
                if let Some(m) = self.mint_manifestation(s, ent) {
                    self.rivals[ri].owned[s] = m as u16;
                }
            }
        }
        {
            let r = &mut self.rivals[ri];
            r.mana = 1000;
            r.mana_delta = 0;
            r.grace = 100;
            r.regen_stall = 0;
            r.state = AiState::Fresh;
            r.target = 0;
            r.burst = 0;
            r.poverty = false;
            r.jink = 0;
            r.hate = [HATE_NEUTRAL; 8];
            r.war = [false; 8];
            r.cooldown = [0; SPELL_COUNT];
            r.cooldown[16] = 4 * r.slot as u16;
        }
        // Everyone else's ledger toward the respawner: the elevated-
        // but-decaying truce value (:55037-41).
        let slot = self.rivals[ri].slot as usize;
        for (oj, o) in self.rivals.iter_mut().enumerate() {
            if oj != ri {
                o.hate[slot] = HATE_RESPAWN;
            }
        }
        self.entities_dirty = true;
    }
}
