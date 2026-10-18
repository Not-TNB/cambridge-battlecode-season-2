// Placing our ward on hidden tiles (bushes, behind walls).
use super::me;
use super::trace::{checkpoint, trace};
use super::world::{blocked_tiles, distance, forget_sight, sight};
use cambc::{Result, Vec2};

// Ward `spot` (or a free tile next to it) if it is hidden and within placement range. Prefer open
// tiles to bushes, then prefer the tile closest to the remembered spot.
pub(super) fn ward_hidden_tile(spot: Vec2) -> Result<()> {
    let me = me()?;
    if me.ward_cooldown()? > 0 || cambc::vision::visible_tiles()?.is_set(spot)? {
        return Ok(());
    }
    let radius = cambc::constants().ward_placement_radius as i32;
    let blocked = blocked_tiles()?;
    let bushes = cambc::map::bushes()?;
    let mut best: Option<(Vec2, (bool, i32))> = None;
    for dx in -1..=1 {
        for dy in -1..=1 {
            let tile = Vec2::new(spot.x + dx, spot.y + dy);
            if distance(me.position()?, tile) > radius || blocked.is_set(tile).unwrap_or(true) {
                continue;
            }
            let rank = (!bushes.is_set(tile)?, distance(tile, spot));
            if best.is_none_or(|(_, r)| rank < r) {
                best = Some((tile, rank));
            }
        }
    }
    // An unseen enemy on the tile makes placement fail or error; either way we just walk on.
    if let Some((tile, _)) = best {
        let _ = me.place_ward(tile);
    }
    Ok(())
}

// Ward a hidden tile next to us (a bush, or behind a wall) so we see into it: bushes first, then
// the furthest from us (the ward sees 6 tiles round it). Wards are free and recharge after a
// round; we keep our current one while it's within WARD_KEEP_TILES of us.
pub const WARD_HIDDEN: bool = true;
const WARD_KEEP_TILES: i32 = 5;

pub fn ward_hidden_spots() -> Result<()> {
    let me = me()?;
    if !WARD_HIDDEN || me.ward_cooldown()? > 0 {
        return Ok(());
    }
    let here = me.position()?;
    if me
        .ward_positions()?
        .iter()
        .any(|&w| distance(w, here) <= WARD_KEEP_TILES)
    {
        return Ok(());
    }
    checkpoint("ward-search-start");
    let radius = cambc::constants().ward_placement_radius as i32;
    let sight = sight()?;
    let visible = &sight.visible;
    let (bushes, walls, holes) = (
        cambc::map::bushes()?,
        cambc::map::walls()?,
        cambc::map::holes()?,
    );
    let mut best: Option<(Vec2, (bool, i32))> = None;
    for dx in -radius..=radius {
        for dy in -radius..=radius {
            let tile = Vec2::new(here.x + dx, here.y + dy);
            if !cambc::map::is_in_bounds(tile)?
                || visible.is_set(tile)?
                || walls.is_set(tile)?
                || holes.is_set(tile)?
            {
                continue;
            }
            let rank = (bushes.is_set(tile)?, distance(tile, here));
            if best.is_none_or(|(_, r)| rank > r) {
                best = Some((tile, rank));
            }
        }
    }
    checkpoint("ward-search-done");
    // An unseen unit on the tile makes placement fail or error; either way we just carry on. The
    // new ward replaces the old one, so what we see changes.
    if let Some((tile, _)) = best {
        if me.place_ward(tile).unwrap_or(false) {
            trace("ward");
        }
        forget_sight();
    }
    checkpoint("ward-place-done");
    Ok(())
}
