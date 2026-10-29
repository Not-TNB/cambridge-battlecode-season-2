// Team communication through the 8 shared bytes:
//   [0..6]  claim per champion slot: camp group id + 1 being worked on (0: none)
//   [6]     rally: camp group id + 1 (0: no rally)
//   [7]     rally: round it was called (low byte), so stale rallies expire
// Slots follow the order of cambc::champions(Ally), the same in every bot instance.
use crate::util::round;
use cambc::Result;

pub const SLOTS: usize = 6;
// Claim byte of a dead champion.
pub const DEAD: u8 = 255;

pub fn set_dead(slot: usize) -> Result<()> {
    if slot < SLOTS {
        cambc::shared_memory().set(slot, DEAD)?;
    }
    Ok(())
}

pub fn read() -> [u8; 8] {
    let mut b = [0u8; 8];
    let _ = cambc::shared_memory().read(0, &mut b);
    b
}

pub fn set_claim(slot: usize, group: Option<usize>) -> Result<()> {
    if slot < SLOTS {
        let v = group.map(|g| (g as u8).wrapping_add(1)).unwrap_or(0);
        cambc::shared_memory().set(slot, v)?;
    }
    Ok(())
}

pub fn claims(mem: &[u8; 8], me: usize) -> impl Iterator<Item = usize> + '_ {
    (0..SLOTS)
        .filter(move |s| *s != me && mem[*s] != 0 && mem[*s] != DEAD)
        .map(move |s| mem[s] as usize - 1)
}

pub fn rally(mem: &[u8; 8]) -> Option<(usize, i32)> {
    if mem[6] == 0 {
        return None;
    }
    // Age from the stored low byte of the round it was called.
    let age = (round() as u8).wrapping_sub(mem[7]) as i32;
    Some((mem[6] as usize - 1, age))
}

pub fn call_rally(group: usize) -> Result<()> {
    cambc::shared_memory().write(6, &[(group as u8).wrapping_add(1), round() as u8])
}

pub fn cancel_rally(group: usize) -> Result<()> {
    let mem = read();
    if mem[6] as usize == group + 1 {
        cambc::shared_memory().write(6, &[0, 0])?;
    }
    Ok(())
}

// Fight beacon (bytes 6-7, which the rally used to hold): where an ally is in a fight, x + 1 and
// y (0: none). Allies close enough come to help; whoever finds the spot quiet clears it.
pub fn beacon(mem: &[u8; 8]) -> Option<cambc::Vec2> {
    (mem[6] != 0).then(|| cambc::Vec2::new(mem[6] as i32 - 1, mem[7] as i32))
}

pub fn set_beacon(p: cambc::Vec2) -> Result<()> {
    cambc::shared_memory().write(6, &[(p.x + 1) as u8, p.y as u8])
}

pub fn clear_beacon() -> Result<()> {
    cambc::shared_memory().write(6, &[0, 0])
}
