// Shopping: the profile's emblem order (5 shards each), then Power shards (an extra shard's worth
// of stats per emblem). An altar lock (10 gold) is always carried.
use crate::profile::{main_stat, Kind, Params, Stat};
use cambc::{get_item_enum, Champion, ChampionType, ConsumableType, ItemFamily, ItemStat, ItemType, Result};

fn stat_of(s: Stat, c: ChampionType, kind: Kind) -> ItemStat {
    match s {
        Stat::Main => main_stat(c, kind),
        Stat::Health => ItemStat::Health,
    }
}

// The next shard to buy, or None when the build is complete (never: Power is unbounded).
pub fn next_shard(me: &Champion, p: &Params, c: ChampionType) -> Result<ItemType> {
    let items = me.item_count()?;
    for (family, s) in p.build {
        let stat = stat_of(*s, c, p.kind);
        let emblem = get_item_enum(*family, stat, false);
        if items[emblem] > 0 {
            continue;
        }
        return Ok(get_item_enum(*family, stat, true));
    }
    Ok(get_item_enum(ItemFamily::Power, main_stat(c, p.kind), true))
}

// Spend what we can. `at_spawn`: consumables and healing purchases are allowed here.
pub fn shop(me: &Champion, p: &Params, c: ChampionType, at_spawn: bool) -> Result<u32> {
    let mut bought = 0;
    if at_spawn
        && me.consumables()[ConsumableType::AltarLock].count().unwrap_or(1) == 0
        && me.gold_count()? >= 40
        && me.can_buy_consumable(ConsumableType::AltarLock).unwrap_or(false)
    {
        let _ = me.buy_consumable(ConsumableType::AltarLock);
    }
    for _ in 0..12 {
        let shard = next_shard(me, p, c)?;
        if !me.can_buy(shard, 1).unwrap_or(false) {
            break;
        }
        me.buy(shard)?;
        bought += 1;
    }
    Ok(bought)
}

// Gold worth a trip home: a full emblem, or the rest of the current one.
pub fn worth_trip(me: &Champion, p: &Params) -> Result<bool> {
    Ok(me.gold_count()? >= p.trip_gold)
}
