//! MC2 class-10 effects band: the smoke-column emitters
//! (10,59)/(10,60) and their (10,13)/(10,14)
//! smoke particles, plus the (10,6) standing ground fire. Trace
//! bank: docs/traces/mc2-class10-m59-m60.md +
//! mc2-class10-m6-m9-m11-m28-m31.md (`EF:` = remc2
//! EventsFunctions.cpp).
//!
//! The emitters are retail's "quest point" smoke columns: invisible,
//! never map-linked, untargetable logic entities that shed one rising
//! smoke particle per tick for 800..899 ticks. The particles carry
//! the visuals (sprite rows 67 / 9, growing through the row band as
//! they rise). Nothing in the emitter family collides, damages, or
//! sounds; the standing fire is the band's damage dealer (per-tick
//! ch0 area heat).

use crate::engine::features::{Gen, lcg32};
use crate::mc1::mobs::MobCtx;

impl Gen {
    // ---- ctors ---------------------------------------------------------------

    /// `ArriveCheckpoint_4EB50` / `AddSmoke_4EC10` (EF:35663/:35685)
    /// — the (10,59)/(10,60) emitter, byte-identical bodies. Gated on
    /// ≥32 free pool slots (`sub_4A810`); TWO entity-RNG draws (life
    /// 800..899, particle-speed bonus 0..16); NEVER map-linked and
    /// carries no sprite — invisible by construction, like the
    /// class-11 volumes.
    pub(crate) fn mc2_spawn_smoke_emitter(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        if self.free.len() < 32 {
            return None;
        }
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = if model == 59 { 0x40 } else { 0x41 };
        }
        let d = self.mc2_rand(i);
        self.ent[i].max_life = d % 0x64 + 800;
        // byte[0] = (&0xF6)|1: bit 0 set, bit 3 (targetable) cleared.
        self.ent[i].flags = (self.ent[i].flags & !0x8) | 1;
        let d = self.mc2_rand(i);
        {
            let e = &mut self.ent[i];
            e.f126 = (d % 0x11) as i16; // actSpeed = the speed bonus
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.refill_life(i);
        Some(i)
    }

    /// `SetSmoke4_4EAA0` (EF:35639) — the shared smoke-particle body:
    /// state = model (13/14), ONE entity-RNG draw (actSpeed 51..103),
    /// maxSpeed 30, xtype 10/xsubtype = model, map-linked, half-speed
    /// sprite. Flag ops ≡ the (10,0) fire ctor (`dword &= 0xFFFDFFF7`
    /// then `byte[2] |= 2`).
    pub(crate) fn mc2_spawn_smoke_particle(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
        life: u32,
        sprite: u16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = model;
            e.max_life = life;
            e.f130 = 30; // maxSpeed
            e.f66 = 10; // xtype
            e.f67 = model; // xsubtype
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        let d = self.mc2_rand(i);
        self.ent[i].f126 = (d % 0x35 + 51) as i16; // actSpeed = rise rate
        self.link(i, x, y, z);
        self.mc2_set_sprite(i, sprite);
        self.refill_life(i);
        Some(i)
    }

    /// `SetParticleSmoke3B_4E9E0` / `SetParticleSmoke3C_4EA20` /
    /// `sub_4EA60` (EF:35618/:35625/:35632) — the per-model wrapper:
    /// ONE global-RNG draw for the life roll (m13: 17..39 sprite 67;
    /// m14: 28..60 sprite 9; m87 = the THIRD PUFF, m13's roll and
    /// sprite under its own action 0x5E —
    /// docs/traces/mc2-class10-m29-m5-m13.md §4.3). The roll only
    /// survives on direct (authored) spawns — the emitter overwrites
    /// it to 32.
    pub(crate) fn mc2_spawn_smoke_particle_for(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let g = lcg32(&mut self.rand);
        let (life, sprite) = match model {
            13 | 87 => (g % 0x17 + 17, 67),
            _ => (g % 0x21 + 28, 9),
        };
        let i = self.mc2_spawn_smoke_particle(model, x, y, z, life, sprite)?;
        if model == 87 {
            self.ent[i].tick70 = 0x5E; // action != model for the third puff
        }
        Some(i)
    }

    /// `NewAdd0A05_4E570` (EF:35436) — the (10,5) water splash: life
    /// 8, sprite 244, snapped to the water surface, no RNG, no
    /// motion. Flag ops ≡ the fire/smoke ctors.
    pub(crate) fn mc2_spawn_splash(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 5;
            e.tick70 = 5;
            e.max_life = 8;
            e.f44 = 0; // subSpellIndex = 0
            e.f26 = 0;
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(lx, ly) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 244);
        Some(i)
    }

    /// `CreateManaSphere512_50080` / `CreateManaSphere2560_500A0`
    /// (EF:36595/:36601, both thunks into `CreateManaSphere_500C0`
    /// EF:36607) — the authored ground mana economy: (10,39) = the
    /// 512-mana sphere, (10,58) = the 2560 variant (strA1 rows
    /// 0x27/0x3A; the created entity is ALWAYS model 39, action 0x29
    /// — the m59-m60 §8 numbering note) — plus (10,57) = the
    /// RANDOM-VALUE sphere (`sub_50130` EF:36631, its own model with
    /// action 0x3E; docs/traces/mc2-class10-m57.md): mana = one draw
    /// of the sphere's own stream `% 0x7D0` = 0..1999. Unowned (the
    /// neutral 52 sprite family). All ride the shared MC1 ball
    /// machinery exactly like the death-drop spheres (mobs.rs
    /// module-doc APPROX: the MC2 action-0x29/0x3E tick columns are
    /// unported; the MC1 (10,39) ball tick rests/flies/claims them —
    /// m57's AI-avoidance gate `word_0x244_580` rides the same
    /// APPROX).
    pub(crate) fn mc2_spawn_mana_sphere(
        &mut self,
        model: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.spawn_mana_ball(x, y, z)?;
        self.ent[i].f140 = match model {
            58 => 2560,
            57 => (self.ent_rand(i) % 0x7D0) as i32,
            _ => 512,
        };
        self.ent[i].f144 = 0;
        self.ball_resize(i);
        Some(i)
    }

    /// `NewAdd0A06_4E5F0` (EF:35458) — the (10,6) STANDING GROUND
    /// FIRE, the real damaging self-sustaining flame: 240-tick life,
    /// per-tick ch0 area heat of 50 (subSpell home = f140,
    /// the class-10 effect column's amount field like the (10,0)
    /// fire), sprite 228 with ShiftRot(272, 1536), z snapped to
    /// terrain + the `word_0x2C_44` lift (f44 — runtime spawners
    /// raise it; the ctor zeroes it, overriding NewEvent's 100).
    /// NOT targetable (byte[0] bit 3 cleared — fire cannot be
    /// attacked); byte[2] bit 1 set (reclaimable). No RNG.
    ///
    /// APPROX register: `AddEvent2_847D0(80, 11, 1)` — the
    /// Night/Cave dynamic light registration — is presentation,
    /// unported (the same note as the (10,1) big explosion).
    pub(crate) fn mc2_spawn_fire6(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 6;
            e.tick70 = 6;
            e.f140 = 50; // subSpellIndex = the per-tick ch0 amount
            e.max_life = 240;
            e.f44 = 0; // word_0x2C_44 = the z lift
            e.f26 = 0; // dword_0x10_16 = the grow/shrink step
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = self.ground_z(lx, ly) as i16;
        self.refill_life(i);
        self.mc2_set_sprite(i, 228);
        self.mc2_shift_rot(i, 272, 1536);
        Some(i)
    }

    /// `sub_50840` (EF:36960) — the Magic Mine (spell 23) persistent
    /// proximity mine `(10,78)`: sprite 66, sits on the ground, life =
    /// the tier lifespan (1000/5000/10000). Placed by the carrier's
    /// landing (mc2_proj_impact `(10,78)`); the owner is stamped by the
    /// impact tail (id24). `f44` = the blast intensity (`byte_0x43`,
    /// 1/2/4/8 by tier); `f26` = the random arm delay (16..65 ticks,
    /// `rand%0x32 + 16`). Ticks via [`Gen::mc2_mine_tick`] (action 0x55)
    /// — docs/spell-audit/magic-mine.md.
    pub(crate) fn mc2_spawn_magic_mine(
        &mut self,
        x: u16,
        y: u16,
        tier: u8,
        lifespan: i32,
    ) -> Option<usize> {
        let i = self.new_event()?;
        let gz = self.ground_z(x, y) as i16;
        let blast = 1u16 << tier.min(3); // 0→1, 1→2, 2→4, 3→8
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 78;
            e.tick70 = 85; // action 0x55 = sub_3A8B0
            e.max_life = lifespan.max(1) as u32;
            e.f44 = blast; // blast intensity (byte_0x43)
            e.flags = (e.flags & !0x2_0008) | 0x2_0000;
        }
        self.link(i, x, y, gz);
        self.refill_life(i);
        let r = self.ent_rand(i);
        self.ent[i].f26 = ((r % 0x32) + 16) as i16; // arm delay 16..65
        self.mc2_set_sprite(i, 66);
        Some(i)
    }

    /// `sub_3A8B0` (EF:29749), class-10 action 0x55 — the Magic Mine
    /// tick. Lifespan countdown (self-expire at 0), then a random arm
    /// delay (`f26`), then a PROXIMITY scan every 16 ticks for an enemy
    /// wizard/castle (class 3, model ≤ 1, or the out-of-pool human)
    /// within 14 tiles (3584 units), excluding the owner. On a hit it
    /// detonates. The exact `sub_6DCA0` detonation family + the
    /// `word_0x36_54` armed gate are untraced (OPEN, magic-mine.md §6);
    /// the port arms after the random delay and delivers a direct ch0
    /// blast scaled by the tier intensity.
    /// EXPIRY TEARDOWN (EF:30043-86). `byte_0x46_70` is NOT an engine
    /// action index — MC2 dispatches on `actionIndex_0x45_69` (offset
    /// 69, our `tick70`) — it is offset 70, our `f71`, and here it is a
    /// sub-state machine switched on inside `sub_3A8B0` itself
    /// (EF:29881). A mine whose lifespan runs out enters 6, which
    /// clears the draw bit and (once `f69` is clear) advances to 7 with
    /// a 10-tick timer; 7 counts down into 9 with a counter of 3; 9
    /// SINKS the mine by an accelerating `32 * counter` per tick until
    /// it meets the ground, then drops a class-10 puff — model 5 over
    /// water, model 0 over land — and despawns.
    pub(crate) fn mc2_mine_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        // EF:29840-45 — sub-states 7 and 9 skip the lifespan countdown.
        // The expiry test is post-decrement `<= 0`, and it enters the
        // teardown rather than despawning; the switch below then runs
        // sub-state 6 on this SAME tick.
        if self.ent[i].f71 != 7 && self.ent[i].f71 != 9 {
            self.ent[i].act_life -= 1;
            if self.ent[i].act_life <= 0 {
                self.ent[i].f71 = 6;
            }
            // EF:29862-72 — the mine clamps UP out of the ground, then
            // FLOATS toward ground + 1024 in +/-48 steps with a 96-unit
            // deadband (gated on f69 == 0). Player-observed in retail:
            // the mine rises to roughly castle-tower height rather than
            // resting on the ground, which is where ours sat because
            // this whole block was missing.
            let (x, y) = (self.ent[i].x, self.ent[i].y);
            let g = self.ground_z(x, y);
            if (self.ent[i].z as i32) < g {
                self.ent[i].z = g as i16;
            }
            let target = g + 1024;
            let delta = self.ent[i].z as i32 - target;
            if self.ent[i].f69 == 0 && delta.abs() > 96 {
                let step = if delta <= 0 { 48 } else { -48 };
                self.ent[i].z = (self.ent[i].z as i32 + step) as i16;
            }
        }
        match self.ent[i].f71 {
            // EF:30043-54 — clear the draw bit and wait for f69.
            6 => {
                self.ent[i].flags &= !1;
                if self.ent[i].f69 == 0 {
                    self.ent[i].f71 = 7;
                    self.ent[i].f26 = 10;
                }
                return false;
            }
            // EF:30055-62 — the 10-tick pause before the sink.
            7 => {
                self.ent[i].f26 -= 1;
                if self.ent[i].f26 == 0 {
                    self.ent[i].f71 = 9;
                    self.ent[i].f26 = 3;
                }
                return false;
            }
            // EF:30073-85 — the accelerating sink, then the puff.
            9 => {
                self.ent[i].f26 += 1;
                let step = 32 * self.ent[i].f26 as i32;
                self.ent[i].z = (self.ent[i].z as i32 - step) as i16;
                let (x, y) = (self.ent[i].x, self.ent[i].y);
                let g = self.ground_z(x, y);
                if self.ent[i].z as i32 >= g {
                    return false;
                }
                self.ent[i].z = g as i16;
                let model = if self.on_water_pub(x, y) { 5 } else { 0 };
                let z = self.ent[i].z;
                self.spawn_effect(model, x, y, z);
                self.ent[i].flags |= 0x400;
                return false;
            }
            _ => {}
        }
        if self.ent[i].f26 > 0 {
            self.ent[i].f26 -= 1; // arming
            return false;
        }
        if self.ent[i].act_life & 0xF != 0 {
            return false; // scan cadence: every 16 ticks
        }
        let (mx, my, own) = {
            let e = &self.ent[i];
            (e.x, e.y, e.id24)
        };
        // A rival-owned mine triggers on the out-of-pool human; a
        // player-owned mine scans the pool for rival avatars/castles.
        // Remember WHICH wizard tripped it — the detonation spits at it.
        let mut victim = (own != crate::mc1::mobs::PLAYER_TARGET
            && Self::isqrt(Self::dist2_sq(mx, my, ctx.px, ctx.py) as u32) < 3584)
            .then_some(crate::mc1::mobs::PLAYER_TARGET);
        if victim.is_none() {
            for j in 1..self.ent.len() {
                if j == i {
                    continue;
                }
                let e = &self.ent[j];
                if e.class64 != 3
                    || e.model65 > 1
                    || e.act_life < 0
                    || e.flags & 0x400 != 0
                    || e.id24 == own
                {
                    continue;
                }
                if Self::isqrt(Self::dist2_sq(mx, my, e.x, e.y) as u32) < 3584 {
                    victim = Some(j as u16);
                    break;
                }
            }
        }
        if let Some(v) = victim {
            self.mc2_mine_detonate(i, ctx, v);
        }
        false
    }

    /// The mine's detonation: a ch0 area blast scaled by the tier
    /// intensity (`f44` = 1/2/4/8) plus the big-explosion visual, then
    /// despawn. APPROX for the untraced `sub_6DCA0` relaunch (OPEN); the
    /// owner-immunity rides `area_write` (the mine's id24). The trip
    /// awards the owner one spell-23 XP (`sub_6D8B0(id, 23, 1)`,
    /// EF:29979) through the `mc2_cast_xp` mail — the world tick's drain
    /// re-applies `sub_6D8B0`'s own human-only guard, so a rival mine's
    /// award is filtered there (like every other pool-side award site).
    fn mc2_mine_detonate(&mut self, i: usize, ctx: &MobCtx, victim: u16) {
        let (x, y, z, blast, owner) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.f44 as u32, e.id24)
        };
        let dmg = blast.saturating_mul(250);
        // THE BLAST NEEDS A BOX. `ent_overlap` sums BOTH parties'
        // extents, and the mine ctor never set f80/f82/f84 — so the
        // blast was a POINT and a wizard standing right beside it took
        // nothing (player-reported), which the 1024 hover then made
        // unmissable. Retail's real blast is the untraced `sub_6DCA0`
        // relaunch (magic-mine.md §6 Q2); 1024 (4 tiles) is our
        // stand-in. Restored after the write so the box does not
        // linger through the sink.
        let saved = {
            let e = &mut self.ent[i];
            let s = (e.f80, e.f82, e.f84);
            e.f80 = 1024;
            e.f82 = 1024;
            e.f84 = 1024;
            s
        };
        self.area_write(i, 0, dmg, ctx, false, false);
        {
            let e = &mut self.ent[i];
            (e.f80, e.f82, e.f84) = saved;
        }
        // ...and it SPITS at whatever tripped it. magic-mine.md §5
        // step 4 calls the detonation a "relaunch" (`sub_6DCA0`), i.e.
        // a spell LAUNCH and not a bare area write, and the player
        // expected a projectile from mine to wizard. The exact family
        // for spell 23 is OPEN, so we reuse the (9,0) bolt.
        // DELIBERATE — see docs/DEVIATIONS.md.
        self.mc2_atk_bolt(i, victim, ctx);
        self.mc2_cast_xp.0.push((owner, 23, 1));
        self.mc2_spawn_big_explosion(x, y, z);
        // Instead of vanishing on the spot, hand the spent mine to the
        // SAME teardown retail runs at lifespan expiry (f71 = 6 -> 7 ->
        // 9): it hangs a moment, sinks to the ground and goes out in a
        // puff. The 7/9 states are excluded from the hover block above,
        // so the sink is not fought by the float.
        self.ent[i].f71 = 6;
    }

    /// `sub_4FE40` (EF:36506) — the (10,34) MC2 TELEPORTER pad
    /// (docs/traces/mc2-class10-m50-chains-and-tail.md §2): a
    /// self-contained player-only warp, NOT the MC1 paired-portal
    /// arm. Visible sprite 223, extents 256, hovers 640 above
    /// terrain, targets class 3 (players), persistent (maxLife 0).
    /// ONE entity-RNG draw whose fling of the launch axis is dead —
    /// the THING post-init overwrites the destination with the
    /// par-authored tile (par1 = dest Y / par2 = dest X, EF:33077 —
    /// the shared (10,34) post-init in the spawn seam); the draw
    /// stays for RNG-stream parity.
    pub(crate) fn mc2_spawn_portal(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 34;
            e.tick70 = 36; // actionIndex 0x24
            e.max_life = 0;
            e.f66 = 3; // xtype: class-3 players only
            e.f67 = 0xFF;
            e.flags &= !8;
        }
        self.mc2_set_sprite(i, 223);
        self.mc2_shift_rot(i, 256, 256);
        self.refill_life(i);
        self.link(i, x, y, z);
        let (lx, ly) = (self.ent[i].x, self.ent[i].y);
        self.ent[i].z = (self.ground_z(lx, ly) as i16).wrapping_add(640);
        let _ = self.mc2_rand(i); // the dead launch-axis fling draw
        Some(i)
    }

    /// `sub_4FD70` (EF:36468) — the (10,51) traveling RIDGE/DAMAGE
    /// BEAM, the (10,50) chain's per-segment child
    /// (docs/traces/mc2-class10-m50-chains-and-tail.md §1.4). Not
    /// map-linked, no sprite (invisible), extents 768, actSpeed
    /// 1024/tick, life = the chain stamper's dist/1024. The damage
    /// amount stays NewEvent's subSpell default 100 (neither the ctor
    /// nor sub_48880 overrides it), homed in f140 like the rest of the
    /// class-10 effect column.
    pub(crate) fn mc2_spawn_load_beam(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = 51;
            e.tick70 = 0x37;
            e.max_life = 0;
            e.f26 = 256; // dword_0x10_16
            e.f126 = 1024; // actSpeed
            e.f140 = 100; // subSpellIndex (the NewEvent default)
            e.flags &= !8;
            e.x = x;
            e.y = y;
            e.z = z;
        }
        self.mc2_shift_rot(i, 768, 768);
        self.refill_life(i);
        Some(i)
    }

    /// `sub_352C0` (EF:25739) — the (10,51) beam tick: post-decrement
    /// despawn OR a class-0 (water/void) cell under it (`sub_104A0 &
    /// 1`, the unrounded cell); otherwise ONE entity-RNG draw feeds
    /// the terrain RAISE — `sub_572C0(0, 1024, r%0xF+10, 0)` walks
    /// the disc of radius pitch/256 = 3 tiles applying the
    /// unprotected +delta cell write (`sub_56F10` ≡ the shared
    /// chassis `dig_cell` — same clamp 0..200, angle-nibble → 1,
    /// water-conversion, (0,0) latch); the walk always exhausts
    /// (nothing refuses in unprotected mode, sub_572C0 → 0) so the
    /// `sub_10C80` ch0 area damage + sound 10 fire EVERY tick; then
    /// advance 1024 along yaw. Returns terrain-dirty.
    pub(crate) fn mc2_load_beam_tick(&mut self, i: usize, ctx: &MobCtx) -> bool {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let raw = crate::engine::features::tile((x >> 8) as u8, (y >> 8) as u8);
        if life < 0 || self.t.angle[raw] & 0xF == 0 {
            self.ent[i].flags |= 0x400;
            return false;
        }
        let d = self.mc2_rand(i);
        let delta = (d % 0xF + 10) as i16;
        let r = (self.ent[i].f80 as i32) >> 8;
        let (cx, cy) = (
            (x.wrapping_add(128) >> 8) as i16,
            (y.wrapping_add(128) >> 8) as i16,
        );
        for (dx, dy) in self.ring_cells(0, r) {
            self.dig_cell_pub(
                cx.wrapping_add((dx as i8) as i16),
                cy.wrapping_add((dy as i8) as i16),
                delta,
                false,
            );
        }
        let amt = self.ent[i].f140 as u32;
        self.area_write(i, 0, amt, ctx, false, false);
        self.snd(10, i);
        let (yaw, spd) = (self.ent[i].f30, self.ent[i].f126);
        let mut pos = (x, y, self.ent[i].z);
        Self::polar_step(&mut pos, yaw, 0, spd);
        {
            let e = &mut self.ent[i];
            e.x = pos.0;
            e.y = pos.1;
            e.z = pos.2;
        }
        true
    }

    /// `sub_4FA00` (EF:36274) — the (10,29) stage/quest marker:
    /// INVISIBLE (no sprite), life 0, lives exactly one tick
    /// (action 0x1F = DisableEntityDrawing). Its whole job is
    /// donating position/identity to the stage binder at spawn —
    /// our stage engine reads the authored checkpoint rows directly,
    /// so the entity is pure churn, exactly like retail.
    pub(crate) fn mc2_spawn_stage_marker(&mut self, x: u16, y: u16, z: i16) -> Option<usize> {
        self.mc2_spawn_stage_marker_for(29, 0x1F, x, y, z)
    }

    /// The shared one-tick marker ctor shape ((10,29) `sub_4FA00`
    /// EF:36274, (10,50) `sub_4FDE0` EF:36488 — byte-identical
    /// bodies modulo model/action): invisible, life 0, untargetable,
    /// map-registered, gone on the first tick.
    pub(crate) fn mc2_spawn_stage_marker_for(
        &mut self,
        model: u8,
        state: u8,
        x: u16,
        y: u16,
        z: i16,
    ) -> Option<usize> {
        let i = self.new_event()?;
        {
            let e = &mut self.ent[i];
            e.class64 = 10;
            e.model65 = model;
            e.tick70 = state;
            e.max_life = 0;
            e.flags &= !0x8;
        }
        self.link(i, x, y, z);
        self.refill_life(i);
        Some(i)
    }

    // ---- ticks ---------------------------------------------------------------

    /// `AddAsh0A_05_318B0` (EF:23169) — the splash tick: 8 ticks of
    /// frame animation at the water surface, sound 27 once (the
    /// flags-bit-2 latch), then despawn.
    pub(crate) fn mc2_splash_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        self.ent[i].frame88 = self.ent[i].frame88.saturating_add(1);
        if self.ent[i].flags & 2 == 0 {
            self.ent[i].flags |= 2;
            self.snd(27, i);
        }
    }

    /// `AddParticleSmoke0A_3D_32420` (EF:23666) — the shared emitter
    /// tick: post-decrement despawn, THREE entity-RNG draws (x-jitter
    /// 0..159, z-jitter 0..159 — retail jitters x and z only, NEVER
    /// y — and the particle speed bonus 0..76), one particle per tick
    /// with life forced to 32.
    pub(crate) fn mc2_smoke_emitter_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (ex, ey, ez, model) = {
            let e = &self.ent[i];
            (e.x, e.y, e.z, e.model65)
        };
        let d = self.mc2_rand(i);
        let px = ex.wrapping_add((d % 0xA0) as u16);
        let d = self.mc2_rand(i);
        let pz = ez.wrapping_add((d % 0xA0) as i16);
        let pm = if model == 59 { 13 } else { 14 };
        if let Some(p) = self.mc2_spawn_smoke_particle_for(pm, px, ey, pz) {
            let d = self.mc2_rand(i);
            self.ent[p].act_life = 32;
            self.ent[p].max_life = 32;
            self.ent[p].f126 += self.ent[i].f126 + (d % 0x4D) as i16;
        }
    }

    /// `sub_32160` / `sub_322A0` (EF:23572/:23613) — the particle
    /// tick, identical except the sprite-row band (m13: grow to 74,
    /// end-of-life floor 67; m14: 16/9). Rise by actSpeed (−4/tick,
    /// clamped [64,128]) with the terrain floor, drift yaw-forward
    /// for the first 16 phase ticks (maxSpeed −52/tick clamped
    /// [30,1024]), grow the sprite row on even ticks, shrink it when
    /// life < 6. No RNG, no sound.
    pub(crate) fn mc2_smoke_particle_tick(&mut self, i: usize) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            return;
        }
        let (grow_cap, shrink_floor) = if self.ent[i].model65 == 13 {
            (74, 67)
        } else {
            (16, 9)
        };
        {
            let e = &mut self.ent[i];
            e.f126 = (e.f126 - 4).clamp(64, 128);
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let mut pos = (x, y, self.ent[i].z.wrapping_add(self.ent[i].f126));
        let alt = self.ground_z(x, y) as i16;
        if pos.2 < alt {
            pos.2 = alt;
        }
        self.ent[i].f26 += 1;
        if self.ent[i].f26 < 16 {
            let (yaw, spd) = (self.ent[i].f30, self.ent[i].f130);
            Self::polar_step(&mut pos, yaw, 0, spd);
            let e = &mut self.ent[i];
            e.f130 = (e.f130 - 52).clamp(30, 1024);
            if e.f26 & 1 == 0 && e.type86 < grow_cap {
                e.type86 += 1;
            }
        }
        if self.ent[i].act_life < 6 && self.ent[i].type86 > shrink_floor {
            self.ent[i].type86 -= 1;
        }
        self.move_relink(i, pos.0, pos.1, pos.2);
    }

    /// `sub_31760` (EF:23099) — the (10,6) standing-fire tick.
    /// Post-decrement despawn WITH one last damage pulse; the
    /// grow/shrink sprite machine on `word_0x5A_90` (type86: 6-step
    /// ramp up while life >= 12, ramp down under 12 with a ~1/7
    /// (10,14) smoke puff per shrink tick — life forced 15, drift
    /// phase disabled, sprite row +2, id inherited); z = f44 lift +
    /// terrain each tick; extinguished by water; ch0 area heat of
    /// `subSpell` EVERY tick (`sub_11400` — the void mailbox writer;
    /// trees take a tenth, hence `building_tenth`), gated only on
    /// byte[2] bit 0 which nothing here sets.
    ///
    /// APPROX register: `sub_5C870` (EF:43602, the player
    /// nearest-hazard distance for HUD/audio proximity) has no
    /// ported consumer — skipped, no gameplay observable.
    pub(crate) fn mc2_fire6_tick(&mut self, i: usize, ctx: &MobCtx) {
        let life = self.ent[i].act_life;
        self.ent[i].act_life -= 1;
        if life < 0 {
            self.ent[i].flags |= 0x400;
            if self.ent[i].flags & 0x1_0000 == 0 {
                let amt = self.ent[i].f140 as u32;
                self.area_write(i, 0, amt, ctx, true, false);
            }
            return;
        }
        if self.ent[i].act_life < 12 {
            if self.ent[i].f26 > 0 {
                self.ent[i].f26 -= 1;
                self.ent[i].type86 = self.ent[i].type86.wrapping_sub(1);
                if self.ent[i].flags & 0x80 == 0 {
                    let d = self.mc2_rand(i);
                    if d % 7 == 0 {
                        let (x, y, z, id) = {
                            let e = &self.ent[i];
                            (e.x, e.y, e.z, e.id24)
                        };
                        if let Some(p) = self.mc2_spawn_smoke_particle_for(14, x, y, z) {
                            let e = &mut self.ent[p];
                            e.f26 = 100;
                            e.act_life = 15;
                            e.id24 = id;
                            e.type86 += 2;
                        }
                    }
                }
            }
        } else if self.ent[i].f26 <= 6 {
            self.ent[i].type86 += 1;
            self.ent[i].f26 += 1;
        }
        let (x, y) = (self.ent[i].x, self.ent[i].y);
        let ground = self.ground_z(x, y) as i16;
        self.ent[i].z = (self.ent[i].f44 as i16).wrapping_add(ground);
        if self.cap_bit(x, y) == 1 {
            self.ent[i].flags |= 0x400;
        }
        if self.ent[i].flags & 0x1_0000 == 0 {
            let amt = self.ent[i].f140 as u32;
            self.area_write(i, 0, amt, ctx, true, false);
        }
    }
}
