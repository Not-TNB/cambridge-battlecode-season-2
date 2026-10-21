// bot-v2's turn logic, copied in by scripts/sync-v2-into-squad.py (every other file here comes from
// bots/bot-v2/src: change it there and re-run the script). Followers play it while they're on their
// own (squad::follower_free): while Tavi is dead, and once the squad splits up (squad::SOLO_ROUND).
// Its draft-time parts (the champion picks, the opening plan) aren't used, so some of it is dead
// code here.
#![allow(dead_code)]

pub mod bot;
mod champions;
mod common;
mod constants;
mod opening;
pub mod regions;

pub fn random_number(limit: u32) -> u32 {
    crate::random_number(limit)
}

// bot-v2 keeps its rallies, opening team target and lead damage dealer in the team's 8 bytes of
// shared memory, which the squad's commands fill while it's together. Until the squad hands them
// over (squad::v2_owns_shared_memory), reads see zeros (nothing set) and writes are dropped.
pub struct SharedMemory;

impl SharedMemory {
    pub fn read(&self, offset: usize, bytes: &mut [u8]) -> cambc::Result<()> {
        if crate::squad::v2_owns_shared_memory()? {
            cambc::shared_memory().read(offset, bytes)
        } else {
            bytes.fill(0);
            Ok(())
        }
    }

    pub fn write(&self, offset: usize, bytes: &[u8]) -> cambc::Result<()> {
        if crate::squad::v2_owns_shared_memory()? {
            cambc::shared_memory().write(offset, bytes)
        } else {
            Ok(())
        }
    }
}

pub fn shared_memory() -> &'static SharedMemory {
    &SharedMemory
}
