// What this champion remembers between turns: camp group states (we only see groups in vision)
// and where enemies were last seen.
use crate::mapinfo::MapInfo;
use crate::util::{cheb, round};
use cambc::{CampGroupState, Champion, Result, Team, Vec2};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GState {
    Alive,
    // All monsters dead; standing on a spawn tile captures it.
    Reward,
    // Respawning; ready at this round.
    Respawn(i32),
}

#[derive(Clone, Copy)]
pub struct GroupMem {
    pub state: GState,
    pub seen: i32,
    // Live monster health the last time we saw it (the full health until then).
    pub hp: f32,
    // Last round an enemy champion was seen within a few tiles of the group.
    pub enemy_near: i32,
}

#[derive(Clone)]
pub struct EnemyMem {
    pub champ: Champion,
    pub pos: Vec2,
    pub round: i32,
}

pub struct Memory {
    pub groups: Vec<GroupMem>,
    pub enemies: Vec<EnemyMem>,
    // Tile -> group id + 1 (0: no group), for mapping engine groups to ours.
    tile_group: Vec<u16>,
}

impl Memory {
    pub fn new(info: &MapInfo) -> Self {
        let mut tile_group = vec![0u16; (info.w * info.h) as usize];
        for (i, g) in info.groups.iter().enumerate() {
            for p in &g.tiles {
                tile_group[info.idx(*p)] = i as u16 + 1;
            }
        }
        Memory {
            groups: info
                .groups
                .iter()
                .map(|g| GroupMem { state: GState::Alive, seen: 0, hp: g.hp, enemy_near: -100 })
                .collect(),
            enemies: Vec::new(),
            tile_group,
        }
    }

    pub fn group_at(&self, info: &MapInfo, p: Vec2) -> Option<usize> {
        if !info.inside(p) {
            return None;
        }
        let v = self.tile_group[info.idx(p)];
        (v > 0).then(|| v as usize - 1)
    }

    // Refresh groups from this turn's visible camps (camp, group, alive, hp). A group with a living
    // camp in sight is Alive; one whose camps are all in sight and dead needs the engine's group
    // state (reward waiting, or respawning and for how long). CampGroup::state() re-queries the
    // whole vision mask on every call, so it's only asked when the answer can have changed.
    pub fn update_groups(&mut self, info: &MapInfo, camps: &[(cambc::Camp, usize, bool, f32)], vis: &[u8]) -> Result<()> {
        let now = round();
        for gm in self.groups.iter_mut() {
            if let GState::Respawn(r) = gm.state {
                if now >= r {
                    gm.state = GState::Alive;
                    gm.hp = 0.0; // refilled below when seen; full until then
                }
            }
        }
        // Living hp per group from the visible camps (dead camps aren't listed).
        let mut alive_hp: Vec<(usize, f32)> = Vec::new();
        for (_, g, alive, hp) in camps {
            if !*alive {
                continue;
            }
            match alive_hp.iter_mut().find(|s| s.0 == *g) {
                Some(s) => s.1 += hp,
                None => alive_hp.push((*g, *hp)),
            }
        }
        for g in 0..info.groups.len() {
            let visible = info.groups[g].tiles.iter().any(|p| crate::mapinfo::bit(vis, info.idx(*p)));
            if !visible {
                continue;
            }
            let gm = &mut self.groups[g];
            gm.seen = now;
            if let Some((_, hp)) = alive_hp.iter().find(|s| s.0 == g) {
                gm.state = GState::Alive;
                gm.hp = *hp;
                continue;
            }
            // In sight with no living camp: reward or respawning. Ask on a change, or every 10.
            let ask = matches!(gm.state, GState::Alive) || now % 10 == 0;
            if !ask {
                continue;
            }
            match info.handles[g].state() {
                Ok(CampGroupState::RewardAvailable) => {
                    gm.state = GState::Reward;
                    gm.hp = 0.0;
                }
                Ok(CampGroupState::Respawning(t)) => {
                    gm.state = GState::Respawn(now + t.max(0));
                    gm.hp = info.groups[g].hp;
                }
                Ok(CampGroupState::Alive) => gm.state = GState::Alive,
                Err(_) => {}
            }
        }
        for (g, gm) in self.groups.iter_mut().enumerate() {
            if gm.state == GState::Alive && gm.hp <= 0.0 {
                gm.hp = info.groups[g].hp;
            }
        }
        Ok(())
    }

    pub fn note_enemy(&mut self, info: &MapInfo, champ: &Champion, pos: Vec2) {
        let now = round();
        match self.enemies.iter_mut().find(|e| e.champ == *champ) {
            Some(e) => {
                e.pos = pos;
                e.round = now;
            }
            None => self.enemies.push(EnemyMem { champ: champ.clone(), pos, round: now }),
        }
        for (i, g) in info.groups.iter().enumerate() {
            if cheb(g.center, pos) <= 6 {
                self.groups[i].enemy_near = now;
            }
        }
    }

    // Enemies seen in the last `age` rounds, with where they were.
    pub fn recent_enemies(&self, age: i32) -> impl Iterator<Item = &EnemyMem> {
        let now = round();
        self.enemies.iter().filter(move |e| now - e.round <= age)
    }

    pub fn forget_dead(&mut self) {
        self.enemies.retain(|e| e.champ.is_alive().unwrap_or(true));
    }
}

pub fn enemy_team() -> Team {
    Team::Enemy
}
