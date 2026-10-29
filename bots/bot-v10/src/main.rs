// bot-v10: one binary, three playstyles. The draft reads the map, picks a Profile (farm, fight or
// gem), drafts for it and writes the plan into draft memory; every champion then runs the same
// brain with that profile's parameters (profile.rs).
mod alloc;
mod brain;
mod classify;
mod combat;
mod draft;
mod mapinfo;
mod memory;
mod nav;
mod profile;
mod shop;
mod team;
mod util;

#[global_allocator]
static ALLOC: alloc::SizeClassAlloc = alloc::SizeClassAlloc::new();

fn main() -> cambc::Result<()> {
    let mut bot: Option<brain::Bot> = None;
    loop {
        let result = if cambc::is_draft() {
            draft::play()
        } else {
            match bot.as_mut() {
                Some(bot) => bot.turn(),
                None => match cambc::self_champion() {
                    Ok(Some(me)) => match brain::Bot::new(me.clone()) {
                        Ok(b) => bot.insert(b).turn(),
                        Err(e) => Err(e),
                    },
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                },
            }
        };
        if let Err(error) = result {
            println!("bot error: {error}");
        }
        cambc::end_turn();
    }
}
