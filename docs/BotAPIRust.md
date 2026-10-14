# CAMBC Bot API — Rust

Event: cambc2026a. Read alongside GameRules.md. Signatures and descriptions come from the same SDK documentation shown on the Bot API page.

Examples are fragments for an initialised bot. Check visibility and action preconditions. Use ? inside a function returning cambc::Result. 

```rust
use cambc as bc;
```

## cambc

### champion_designs

```rust
pub fn champion_designs() -> Result<&'static [Champion; 12]>
```

Borrow all initialised champion designs, with no live cooldown/visibility queries.

Use this value in your bot's draft or turn logic.

```rust
let result = bc::champion_designs()?;
```

### champions

```rust
pub fn champions(team: Option<Team>) -> Result<Vec<Champion>>
```

All allied/enemy roster members, including hidden champions with cached visibility errors. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Use this value in your bot's draft or turn logic.

Inputs:
- team: Option<Team>

```rust
let result = bc::champions(team)?;
```

### constants

```rust
pub fn constants() -> &'static Constants
```



Use this value in your bot's draft or turn logic.

```rust
let result = bc::constants();
```

### draft_memory

```rust
pub fn draft_memory() -> &'static DraftMemory
```



Use this value in your bot's draft or turn logic.

```rust
let result = bc::draft_memory();
```

### end_turn

```rust
pub fn end_turn()
```

Flush buffered output and yield to end the current guest turn; resumed execution continues after this call.

Stop optional planning when little fuel remains and yield the current turn.

```rust
if bc::fuel_remaining()? < 1000 {
    bc::end_turn();
}
```

### fuel_remaining

```rust
pub fn fuel_remaining() -> Result<u64>
```

Return remaining guest fuel (10x on a team's first draft turn, 2x on each champion's first turn); invalid negative host values return an error.

Stop optional planning when little fuel remains and yield the current turn.

```rust
if bc::fuel_remaining()? < 1000 {
    bc::end_turn();
}
```

### global_submitted_gems

```rust
pub fn global_submitted_gems(team: Team) -> Result<u32>
```

Return the total number of gems submitted by a relative team. # Arguments * `team` - Team to count.

Use this value in your bot's draft or turn logic.

Inputs:
- team: Team

```rust
let result = bc::global_submitted_gems(team)?;
```

### init

```rust
pub fn init() -> Result<()>
```

Initialise immutable SDK data. A static constructor runs this before `main` on the first turn's fuel, so calling it is only needed to retry or inspect a failed initialisation. Successful repeat calls do nothing; failures publish nothing and can be retried. No map layout is cached.

Use this value in your bot's draft or turn logic.

```rust
let result = bc::init()?;
```

### is_draft

```rust
pub fn is_draft() -> bool
```



Use this value in your bot's draft or turn logic.

```rust
let is_draft = bc::is_draft();
```

### resign

```rust
pub fn resign() -> Result<()>
```

Resign the current team through the engine, preserving validation errors. A successful resign ends the game without returning, so buffered output is flushed first.

Use this value in your bot's draft or turn logic.

```rust
let result = bc::resign()?;
```

### round_number

```rust
pub fn round_number() -> Result<u32>
```

Query the current engine round number.

Use this value in your bot's draft or turn logic.

```rust
let result = bc::round_number()?;
```

### self_champion

```rust
pub fn self_champion() -> Result<Option<&'static Champion>>
```

Borrow self's initialised wrapper, or `None` in draft; errors if SDK initialisation failed.

Use this value in your bot's draft or turn logic.

```rust
let result = bc::self_champion()?;
```

### shared_memory

```rust
pub fn shared_memory() -> &'static SharedMemory
```



Use this value in your bot's draft or turn logic.

```rust
let result = bc::shared_memory();
```

## cambc::Ability

### can_target_allies

```rust
pub fn can_target_allies(&self) -> Result<bool>
```

Test the current ally-target category flag.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let can_target_allies = subject.can_target_allies()?;
```

### can_target_camps

```rust
pub fn can_target_camps(&self) -> Result<bool>
```

Test the current camp-target category flag.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let can_target_camps = subject.can_target_camps()?;
```

### can_target_enemies

```rust
pub fn can_target_enemies(&self) -> Result<bool>
```

Test the current enemy-target category flag.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let can_target_enemies = subject.can_target_enemies()?;
```

### can_target_ground

```rust
pub fn can_target_ground(&self) -> Result<bool>
```

Test the current ground-target category flag.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let can_target_ground = subject.can_target_ground()?;
```

### can_target_self

```rust
pub fn can_target_self(&self) -> Result<bool>
```

Test the current self-target category flag.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let can_target_self = subject.can_target_self()?;
```

### cast_at

```rust
pub fn cast_at(&self, destination: Vec2) -> Result<bool>
```

Cast at a ground destination; requires the current champion. Returns `Ok(false)` for a newly revealed obstruction; invalid actions return `Err`. # Arguments * `destination` - Ground tile to cast at; must be within the ability's range.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- destination: Vec2

```rust
let result = subject.cast_at(destination)?;
```

### cast_on

```rust
pub fn cast_on<T: crate::CastTarget + ?Sized>(&self, target: &T) -> Result<bool>
```

Cast on a champion, camp or unit; requires the current champion. Unit casts land on the target's tile; returns `Ok(false)` for a newly revealed obstruction. Invalid actions return `Err`. # Arguments * `target` - Champion or camp to cast on; it must be a valid target category for this ability.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- target: &T

```rust
let result = subject.cast_on(target)?;
```

### cast_untargeted

```rust
pub fn cast_untargeted(&self) -> Result<bool>
```

Cast without an entity or destination; requires the current champion. Returns `Ok(false)` for a newly revealed obstruction; invalid actions return `Err`.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.cast_untargeted()?;
```

### cooldown

```rust
pub fn cooldown(&self) -> Result<i32>
```

Query the current cooldown; requires a live owner and propagates host errors.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.cooldown()?;
```

### damage_amount

```rust
pub fn damage_amount(&self) -> Result<f32>
```

Query this live ability's damage; unsupported kinds propagate the engine error.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.damage_amount()?;
```

### damage_amount_against

```rust
pub fn damage_amount_against(&self, target: &crate::Champion) -> Result<f32>
```

Predict health lost after mitigation and shields, capped at current health. The target must be visible. # Arguments * `target` - Visible champion that would be hit.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- target: &crate::Champion

```rust
let result = subject.damage_amount_against(target)?;
```

### heal_amount

```rust
pub fn heal_amount(&self) -> Result<f32>
```

Query this live ability's healing; unsupported kinds propagate the engine error.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.heal_amount()?;
```

### heal_amount_against

```rust
pub fn heal_amount_against(&self, target: &crate::Champion) -> Result<f32>
```

Cap healing at the visible target's missing health. This does not cast or consume resources. # Arguments * `target` - Visible champion that would be healed.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- target: &crate::Champion

```rust
let result = subject.heal_amount_against(target)?;
```

### info

```rust
pub fn info(&self) -> Result<Option<&AbilityInfo>>
```

Borrow cached metadata, return `None` for an unused slot, or preserve its cached error.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.info()?;
```

### is

```rust
pub fn is(&self, kind: AbilityKind) -> bool
```

# Arguments * `kind` - Ability kind to test for.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- kind: AbilityKind

```rust
let result = subject.is(kind);
```

### is_aoe_damage

```rust
pub fn is_aoe_damage(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_aoe_damage = subject.is_aoe_damage();
```

### is_attack_reset

```rust
pub fn is_attack_reset(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_attack_reset = subject.is_attack_reset();
```

### is_cc

```rust
pub fn is_cc(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_cc = subject.is_cc();
```

### is_damage

```rust
pub fn is_damage(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_damage = subject.is_damage();
```

### is_dash

```rust
pub fn is_dash(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_dash = subject.is_dash();
```

### is_heal

```rust
pub fn is_heal(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_heal = subject.is_heal();
```

### is_on_cooldown

```rust
pub fn is_on_cooldown(&self) -> Result<bool>
```

Query whether the current cooldown is positive.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_on_cooldown = subject.is_on_cooldown()?;
```

### is_ready

```rust
pub fn is_ready(&self) -> Result<bool>
```

Check whether this ability has metadata, is off cooldown, and its owner can cast.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_ready = subject.is_ready()?;
```

### is_shield

```rust
pub fn is_shield(&self) -> bool
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let is_shield = subject.is_shield();
```

### mask_into

```rust
pub fn mask_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::valid_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- out: &mut Bitboard

```rust
let result = subject.mask_into(out)?;
```

### shield_amount

```rust
pub fn shield_amount(&self) -> Result<f32>
```

Query this live ability's shielding; unsupported kinds propagate the engine error.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.shield_amount()?;
```

### shield_amount_against

```rust
pub fn shield_amount_against(&self, target: &crate::Champion) -> Result<f32>
```

Query shielding against a particular champion, including engine target modifiers. # Arguments * `target` - Visible champion that would be shielded.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability
- target: &crate::Champion

```rust
let result = subject.shield_amount_against(target)?;
```

### slot

```rust
pub fn slot(&self) -> u32
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.slot();
```

### valid_targets

```rust
pub fn valid_targets(&self) -> Result<AbilityTargetMask>
```

Query current live targeting flags, or use cached flags for a draft design.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.valid_targets()?;
```

### valid_tiles

```rust
pub fn valid_tiles(&self) -> Result<Bitboard>
```

Build a fresh Chebyshev targeting-range mask around the live owner. Unused/unavailable slots and attack resets return errors; this is not a legality mask.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::Ability

```rust
let result = subject.valid_tiles()?;
```

## cambc::AbilityInfo

### as_dyn

```rust
pub fn as_dyn(&self) -> &dyn AbilityMetadata
```



Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityInfo

```rust
let result = subject.as_dyn();
```

## cambc::AbilityTargetMask

### all

```rust
pub fn all() -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

```rust
let result = bc::AbilityTargetMask::all();
```

### bits

```rust
pub fn bits(&self) -> u32
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let result = subject.bits();
```

### complement

```rust
pub fn complement(self) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let result = subject.complement();
```

### contains

```rust
pub fn contains(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.contains(other);
```

### difference

```rust
pub fn difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.difference(other);
```

### empty

```rust
pub fn empty() -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

```rust
let result = bc::AbilityTargetMask::empty();
```

### from_bits

```rust
pub fn from_bits(bits: u32) -> Option<Self>
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- bits: u32

```rust
let result = bc::AbilityTargetMask::from_bits(bits);
```

### from_bits_retain

```rust
pub fn from_bits_retain(bits: u32) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- bits: u32

```rust
let result = bc::AbilityTargetMask::from_bits_retain(bits);
```

### from_bits_truncate

```rust
pub fn from_bits_truncate(bits: u32) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- bits: u32

```rust
let result = bc::AbilityTargetMask::from_bits_truncate(bits);
```

### from_name

```rust
pub fn from_name(name: &str) -> Option<Self>
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- name: &str

```rust
let result = bc::AbilityTargetMask::from_name(name);
```

### insert

```rust
pub fn insert(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask (mutable)
- other: Self

```rust
let result = subject.insert(other);
```

### intersection

```rust
pub fn intersection(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.intersection(other);
```

### intersects

```rust
pub fn intersects(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.intersects(other);
```

### is_all

```rust
pub fn is_all(&self) -> bool
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let is_all = subject.is_all();
```

### is_empty

```rust
pub fn is_empty(&self) -> bool
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let is_empty = subject.is_empty();
```

### iter

```rust
pub fn iter(&self) -> bitflags::iter::Iter<Self>
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let result = subject.iter();
```

### iter_names

```rust
pub fn iter_names(&self) -> bitflags::iter::IterNames<Self>
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask

```rust
let result = subject.iter_names();
```

### remove

```rust
pub fn remove(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask (mutable)
- other: Self

```rust
let result = subject.remove(other);
```

### set

```rust
pub fn set(&mut self, other: Self, value: bool)
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask (mutable)
- other: Self
- value: bool

```rust
let result = subject.set(other, value);
```

### symmetric_difference

```rust
pub fn symmetric_difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.symmetric_difference(other);
```

### toggle

```rust
pub fn toggle(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask (mutable)
- other: Self

```rust
let result = subject.toggle(other);
```

### union

```rust
pub fn union(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Evaluate an ability before choosing a spell.

Inputs:
- subject: bc::AbilityTargetMask
- other: Self

```rust
let result = subject.union(other);
```

## cambc::AltarLock

### hits_remaining

```rust
pub fn hits_remaining(&self) -> u32
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::AltarLock

```rust
let result = subject.hits_remaining();
```

### is_ally

```rust
pub fn is_ally(&self) -> bool
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::AltarLock

```rust
let is_ally = subject.is_ally();
```

### is_enemy

```rust
pub fn is_enemy(&self) -> bool
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::AltarLock

```rust
let is_enemy = subject.is_enemy();
```

### position

```rust
pub fn position(&self) -> Vec2
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::AltarLock

```rust
let result = subject.position();
```

### team

```rust
pub fn team(&self) -> Team
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::AltarLock

```rust
let result = subject.team();
```

## cambc::Bitboard

### clear

```rust
pub fn clear(&mut self)
```

Clear all bits locally, including chunk padding.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.clear();
```

### complement

```rust
pub fn complement(&self) -> Result<Self>
```

Return an owning mask with valid map tiles complemented.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.complement()?;
```

### count

```rust
pub fn count(&self) -> Result<usize>
```

Query the number of set map tiles.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.count()?;
```

### draw_chebyshev

```rust
pub fn draw_chebyshev(&mut self, center: Vec2, radius: f32) -> Result<()>
```

Add tiles within Chebyshev `radius` of `center`; host validates the radius. # Arguments * `center` - Centre tile. * `radius` - Chebyshev radius in tiles.

Build a pathfinding or targeting mask.

Inputs:
- center: Vec2
- radius: f32

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.draw_chebyshev(center, radius)?;
```

### draw_circle

```rust
pub fn draw_circle(&mut self, center: Vec2, radius: f32) -> Result<()>
```

Add tiles within a Euclidean circle centred at `center`; host validates the radius. # Arguments * `center` - Centre tile. * `radius` - Euclidean radius in tiles.

Build a pathfinding or targeting mask.

Inputs:
- center: Vec2
- radius: f32

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.draw_circle(center, radius)?;
```

### export_linear_buffer

```rust
pub fn export_linear_buffer(&self) -> Result<Vec<u8>>
```

Return an owning row-major buffer without chunk padding.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.export_linear_buffer()?;
```

### export_linear_buffer_into

```rust
pub fn export_linear_buffer_into(&self, bytes: &mut [u8]) -> Result<()>
```

Export row-major tile values into an exact-size caller buffer. # Arguments * `bytes` - Destination with one value per tile in row-major order.

Build a pathfinding or targeting mask.

Inputs:
- bytes: &mut [u8]

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.export_linear_buffer_into(bytes)?;
```

### export_raw_buffer

```rust
pub fn export_raw_buffer(&self) -> Result<Vec<u8>>
```

Return an owning copy of the chunked storage bytes.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.export_raw_buffer()?;
```

### export_raw_buffer_into

```rust
pub fn export_raw_buffer_into(&self, bytes: &mut [u8]) -> Result<()>
```

Export chunked bytes into caller storage; its size must match `raw_buffer_size`. # Arguments * `bytes` - Destination, exactly `raw_buffer_size` long.

Build a pathfinding or targeting mask.

Inputs:
- bytes: &mut [u8]

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.export_raw_buffer_into(bytes)?;
```

### fill

```rust
pub fn fill(&mut self) -> Result<()>
```

Set every valid map tile using the host primitive.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.fill()?;
```

### height

```rust
pub fn height(&self) -> i32
```



### import_linear_buffer

```rust
pub fn import_linear_buffer(&mut self, bytes: &[u8]) -> Result<()>
```

Import row-major tile values; host validates the exact buffer size. # Arguments * `bytes` - One value per tile in row-major order.

Build a pathfinding or targeting mask.

Inputs:
- bytes: &[u8]

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.import_linear_buffer(bytes)?;
```

### import_raw_buffer

```rust
pub fn import_raw_buffer(&mut self, bytes: &[u8]) -> Result<()>
```

Import exact-size chunked bytes; wrong size or invalid layout propagates an error. # Arguments * `bytes` - Engine-chunked bytes, exactly `raw_buffer_size` long.

Build a pathfinding or targeting mask.

Inputs:
- bytes: &[u8]

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.import_raw_buffer(bytes)?;
```

### intersect

```rust
pub fn intersect(&self, b: &Self) -> Result<Self>
```

Return an owning intersection of this mask and `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.intersect(b)?;
```

### intersect_assign

```rust
pub fn intersect_assign(&mut self, b: &Self) -> Result<()>
```

Replace this mask with its intersection with `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.intersect_assign(b)?;
```

### is_empty

```rust
pub fn is_empty(&self) -> Result<bool>
```

Query whether the mask has no set map tiles.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let is_empty = subject.is_empty()?;
```

### is_equal

```rust
pub fn is_equal(&self, b: &Self) -> Result<bool>
```

Query whether two masks have equal map bits. # Arguments * `b` - Mask to compare against.

### is_set

```rust
pub fn is_set(&self, p: Vec2) -> Result<bool>
```

Query whether the bit at `p` is set; invalid coordinates propagate an engine error. # Arguments * `p` - Tile to query.

Build a pathfinding or targeting mask.

Inputs:
- p: Vec2

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let is_set = subject.is_set(p)?;
```

### linear_buffer_size

```rust
pub fn linear_buffer_size(&self) -> usize
```



Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.linear_buffer_size();
```

### new

```rust
pub fn new() -> Result<Self>
```

Allocate an empty aligned owning mask from a freshly queried, validated layout. No global initialisation or cached layout is required.

Build a pathfinding or targeting mask.

```rust
let result = bc::Bitboard::new()?;
```

### positions

```rust
pub fn positions(&self) -> Result<Vec<Vec2>>
```

Return an owning list of all set tile coordinates; oversized host lists are rejected.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.positions()?;
```

### print

```rust
pub fn print(&self) -> Result<()>
```

Print the stored board/field through the host debug primitive.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.print()?;
```

### raw_buffer_size

```rust
pub fn raw_buffer_size(&self) -> usize
```



Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.raw_buffer_size();
```

### reachable_tiles

```rust
pub fn reachable_tiles(&self, origin: Vec2, distance: u16) -> Result<Self>
```

Compute an owning reachability mask from `origin` up to `distance`. # Arguments * `origin` - Start tile. * `distance` - Maximum number of 8-directional steps from `origin`.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reachable_tiles(origin, distance)?;
```

### reachable_tiles_into

```rust
pub fn reachable_tiles_into(&self, origin: Vec2, distance: u16, out: &mut Self) -> Result<()>
```

Like [`Self::reachable_tiles`], but writes into `out`. # Arguments * `origin` - Start tile. * `distance` - Maximum number of 8-directional steps from `origin`. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reachable_tiles_into(origin, distance, out)?;
```

### reachable_tiles_with_scratch

```rust
pub fn reachable_tiles_with_scratch( &self, origin: Vec2, distance: u16, frontier: &mut Self, scratch: &mut Self, ) -> Result<Self>
```

Compute reachability from `origin` up to `distance`, reusing caller-provided scratch storage. # Arguments * `origin` - Start tile. * `distance` - Maximum number of 8-directional steps from `origin`. * `frontier` - Working buffer; its contents are overwritten. * `scratch` - Working buffer; its contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16
- frontier: &mut Self
- scratch: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reachable_tiles_with_scratch(origin, distance, frontier, scratch)?;
```

### reachable_tiles_with_scratch_into

```rust
pub fn reachable_tiles_with_scratch_into( &self, origin: Vec2, distance: u16, frontier: &mut Self, scratch: &mut Self, out: &mut Self, ) -> Result<()>
```

Like [`Self::reachable_tiles_with_scratch`], but writes into `out`. # Arguments * `origin` - Start tile. * `distance` - Maximum number of 8-directional steps from `origin`. * `frontier` - Working buffer; its contents are overwritten. * `scratch` - Working buffer; its contents are overwritten. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16
- frontier: &mut Self
- scratch: &mut Self
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reachable_tiles_with_scratch_into(origin, distance, frontier, scratch, out)?;
```

### reflect

```rust
pub fn reflect(&self, s: Symmetry) -> Result<Self>
```

Return an owning mask transformed by the requested map symmetry. # Arguments * `s` - Map symmetry to apply.

Build a pathfinding or targeting mask.

Inputs:
- s: Symmetry

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect(s)?;
```

### reflect_horizontal

```rust
pub fn reflect_horizontal(&self) -> Result<Self>
```

Return an owning horizontally reflected mask using the host map geometry.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect_horizontal()?;
```

### reflect_horizontal_into

```rust
pub fn reflect_horizontal_into(&self, out: &mut Self) -> Result<()>
```

Like [`Self::reflect_horizontal`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect_horizontal_into(out)?;
```

### reflect_into

```rust
pub fn reflect_into(&self, s: Symmetry, out: &mut Self) -> Result<()>
```

Like [`Self::reflect`], but writes into `out`. # Arguments * `s` - Map symmetry to apply. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- s: Symmetry
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect_into(s, out)?;
```

### reflect_vertical

```rust
pub fn reflect_vertical(&self) -> Result<Self>
```

Return an owning vertically reflected mask using the host map geometry.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect_vertical()?;
```

### reflect_vertical_into

```rust
pub fn reflect_vertical_into(&self, out: &mut Self) -> Result<()>
```

Like [`Self::reflect_vertical`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.reflect_vertical_into(out)?;
```

### rotate_180

```rust
pub fn rotate_180(&self) -> Result<Self>
```

Return an owning mask rotated 180 degrees using the host map geometry.

Build a pathfinding or targeting mask.

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.rotate_180()?;
```

### rotate_180_into

```rust
pub fn rotate_180_into(&self, out: &mut Self) -> Result<()>
```

Like [`Self::rotate_180`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.rotate_180_into(out)?;
```

### set

```rust
pub fn set(&mut self, p: Vec2, value: bool) -> Result<()>
```

Set or clear the bit at `p`; invalid coordinates propagate an engine error. # Arguments * `p` - Tile to update. * `value` - `true` to set the bit, `false` to clear it.

Build a pathfinding or targeting mask.

Inputs:
- p: Vec2
- value: bool

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.set(p, value)?;
```

### shadow_cast

```rust
pub fn shadow_cast(&self, origin: Vec2, distance: u16) -> Result<Self>
```

Compute an owning shadow-cast mask from `origin` up to `distance`. # Arguments * `origin` - Viewer tile. * `distance` - Maximum sight radius in tiles.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shadow_cast(origin, distance)?;
```

### shadow_cast_into

```rust
pub fn shadow_cast_into(&self, origin: Vec2, distance: u16, out: &mut Self) -> Result<()>
```

Like [`Self::shadow_cast`], but writes into `out`. # Arguments * `origin` - Viewer tile. * `distance` - Maximum sight radius in tiles. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- origin: Vec2
- distance: u16
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shadow_cast_into(origin, distance, out)?;
```

### shift_right

```rust
pub fn shift_right(&self, count: i32) -> Result<Self>
```

Return an owning mask shifted horizontally by the signed column count. # Arguments * `count` - Columns to shift; negative values shift left.

Build a pathfinding or targeting mask.

Inputs:
- count: i32

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shift_right(count)?;
```

### shift_right_into

```rust
pub fn shift_right_into(&self, count: i32, out: &mut Self) -> Result<()>
```

Like [`Self::shift_right`], but writes into `out`. # Arguments * `count` - Columns to shift; negative values shift left. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- count: i32
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shift_right_into(count, out)?;
```

### shift_up

```rust
pub fn shift_up(&self, count: i32) -> Result<Self>
```

Return an owning mask shifted vertically by the signed row count. # Arguments * `count` - Rows to shift; negative values shift down.

Build a pathfinding or targeting mask.

Inputs:
- count: i32

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shift_up(count)?;
```

### shift_up_into

```rust
pub fn shift_up_into(&self, count: i32, out: &mut Self) -> Result<()>
```

Like [`Self::shift_up`], but writes into `out`. # Arguments * `count` - Rows to shift; negative values shift down. * `out` - Destination mask; its previous contents are overwritten.

Build a pathfinding or targeting mask.

Inputs:
- count: i32
- out: &mut Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.shift_up_into(count, out)?;
```

### toggle

```rust
pub fn toggle(&mut self, p: Vec2) -> Result<()>
```

Toggle the bit at `p`; invalid coordinates propagate an engine error. # Arguments * `p` - Tile to flip.

Build a pathfinding or targeting mask.

Inputs:
- p: Vec2

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.toggle(p)?;
```

### union

```rust
pub fn union(&self, b: &Self) -> Result<Self>
```

Return an owning union of this mask and `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.union(b)?;
```

### union_assign

```rust
pub fn union_assign(&mut self, b: &Self) -> Result<()>
```

Replace this mask with its union with `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.union_assign(b)?;
```

### width

```rust
pub fn width(&self) -> i32
```



### xor

```rust
pub fn xor(&self, b: &Self) -> Result<Self>
```

Return an owning symmetric difference of this mask and `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.xor(b)?;
```

### xor_assign

```rust
pub fn xor_assign(&mut self, b: &Self) -> Result<()>
```

Replace this mask with its symmetric difference with `b`. # Arguments * `b` - Mask to combine with; may alias `self`.

Build a pathfinding or targeting mask.

Inputs:
- b: &Self

```rust
let mut subject = bc::map::walls()?.union(bc::map::holes()?)?;

let result = subject.xor_assign(b)?;
```

## cambc::BuffCollection

### dyn_info

```rust
pub fn dyn_info(&self, kind: BuffKind) -> Option<Box<dyn BuffInfo>>
```

# Arguments * `kind` - Buff kind to look up.

Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection
- kind: BuffKind

```rust
let result = subject.dyn_info(kind);
```

### get

```rust
pub fn get(&self, kind: BuffKind) -> Option<BuffPayload>
```

# Arguments * `kind` - Buff kind to look up.

Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection
- kind: BuffKind

```rust
let result = subject.get(kind);
```

### has

```rust
pub fn has(&self, kind: BuffKind) -> bool
```

# Arguments * `kind` - Buff kind to test for.

Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection
- kind: BuffKind

```rust
let result = subject.has(kind);
```

### has_enhanced_attack

```rust
pub fn has_enhanced_attack(&self) -> bool
```



Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection

```rust
let has_enhanced_attack = subject.has_enhanced_attack();
```

### has_shield

```rust
pub fn has_shield(&self) -> bool
```



Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection

```rust
let has_shield = subject.has_shield();
```

### has_stun

```rust
pub fn has_stun(&self) -> bool
```



Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection

```rust
let has_stun = subject.has_stun();
```

### is_empty

```rust
pub fn is_empty(&self) -> bool
```



Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection

```rust
let is_empty = subject.is_empty();
```

### iter

```rust
pub fn iter(&self) -> impl Iterator<Item = BuffPayload>
```

Iterate owned buff values without re-querying the host.

Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffCollection

```rust
let result = subject.iter();
```

### len

```rust
pub fn len(&self) -> usize
```



## cambc::BuffPayload

### as_dyn

```rust
pub fn as_dyn(&self) -> &dyn BuffInfo
```



Inspect a visible unit's active effects when evaluating a fight.

Inputs:
- subject: bc::BuffPayload

```rust
let result = subject.as_dyn();
```

## cambc::BuffRestrictions

### all

```rust
pub fn all() -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let result = bc::BuffRestrictions::all();
```

### bits

```rust
pub fn bits(&self) -> u8
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.bits();
```

### complement

```rust
pub fn complement(self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.complement();
```

### contains

```rust
pub fn contains(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.contains(other);
```

### difference

```rust
pub fn difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.difference(other);
```

### empty

```rust
pub fn empty() -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let result = bc::BuffRestrictions::empty();
```

### from_bits

```rust
pub fn from_bits(bits: u8) -> Option<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u8

```rust
let result = bc::BuffRestrictions::from_bits(bits);
```

### from_bits_retain

```rust
pub fn from_bits_retain(bits: u8) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u8

```rust
let result = bc::BuffRestrictions::from_bits_retain(bits);
```

### from_bits_truncate

```rust
pub fn from_bits_truncate(bits: u8) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u8

```rust
let result = bc::BuffRestrictions::from_bits_truncate(bits);
```

### from_name

```rust
pub fn from_name(name: &str) -> Option<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- name: &str

```rust
let result = bc::BuffRestrictions::from_name(name);
```

### insert

```rust
pub fn insert(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.insert(other);
```

### intersection

```rust
pub fn intersection(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.intersection(other);
```

### intersects

```rust
pub fn intersects(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.intersects(other);
```

### is_all

```rust
pub fn is_all(&self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let is_all = subject.is_all();
```

### is_empty

```rust
pub fn is_empty(&self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let is_empty = subject.is_empty();
```

### iter

```rust
pub fn iter(&self) -> bitflags::iter::Iter<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.iter();
```

### iter_names

```rust
pub fn iter_names(&self) -> bitflags::iter::IterNames<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.iter_names();
```

### remove

```rust
pub fn remove(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.remove(other);
```

### set

```rust
pub fn set(&mut self, other: Self, value: bool)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self
- value: bool

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.set(other, value);
```

### symmetric_difference

```rust
pub fn symmetric_difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.symmetric_difference(other);
```

### toggle

```rust
pub fn toggle(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.toggle(other);
```

### union

```rust
pub fn union(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.restrictions()?;

let result = subject.union(other);
```

## cambc::Camp

### base_stats

```rust
pub fn base_stats(&self) -> Result<CampStats>
```

Query immutable engine design stats, not current modified combat stats.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let result = subject.base_stats()?;
```

### camp_group

```rust
pub fn camp_group(&self) -> Result<CampGroup>
```

Query this camp's group from the host; requires successful SDK initialisation and gameplay perspective.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let result = subject.camp_group()?;
```

### is_elk

```rust
pub fn is_elk(&self) -> bool
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let is_elk = subject.is_elk();
```

### is_gem_cluster

```rust
pub fn is_gem_cluster(&self) -> bool
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let is_gem_cluster = subject.is_gem_cluster();
```

### is_gold_cluster

```rust
pub fn is_gold_cluster(&self) -> bool
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let is_gold_cluster = subject.is_gold_cluster();
```

### is_monolith

```rust
pub fn is_monolith(&self) -> bool
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let is_monolith = subject.is_monolith();
```

### is_wolf

```rust
pub fn is_wolf(&self) -> bool
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let is_wolf = subject.is_wolf();
```

### kind

```rust
pub fn kind(&self) -> CampType
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let result = subject.kind();
```

### spawn_position

```rust
pub fn spawn_position(&self) -> Vec2
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::Camp

```rust
let result = subject.spawn_position();
```

## cambc::CampGroup

### camps

```rust
pub fn camps(&self) -> &[Camp]
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.camps();
```

### gem_value

```rust
pub fn gem_value(&self) -> u32
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.gem_value();
```

### gold_value

```rust
pub fn gold_value(&self) -> u32
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.gold_value();
```

### iter

```rust
pub fn iter(&self) -> core::slice::Iter<'_, Camp>
```

Borrow an iterator over immutable camp membership.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.iter();
```

### max_respawn_timer

```rust
pub fn max_respawn_timer(&self) -> i32
```



Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.max_respawn_timer();
```

### spawn_tiles

```rust
pub fn spawn_tiles(&self) -> Result<Bitboard>
```

Build the camp-spawn tiles; computed on every call.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.spawn_tiles()?;
```

### spawn_tiles_into

```rust
pub fn spawn_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::spawn_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup
- out: &mut Bitboard

```rust
let result = subject.spawn_tiles_into(out)?;
```

### state

```rust
pub fn state(&self) -> Result<CampGroupState>
```

Query group state only when its spawn mask intersects current vision. Hidden/unknown groups return `ApiNotInVision`; invalid runtime tags remain errors.

Evaluate a visible camp before choosing where to farm.

Inputs:
- subject: bc::CampGroup

```rust
let result = subject.state()?;
```

## cambc::Champion

### abilities

```rust
pub fn abilities(&self) -> &[Ability; 2]
```



Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.abilities();
```

### attack

```rust
pub fn attack<T: crate::AttackTarget + ?Sized>(&self, target: &T) -> Result<()>
```

Attack a champion, camp, unit or altar lock; requires self and engine-valid targeting. # Arguments * `target` - Visible champion, camp or altar lock within attack range.

Evaluate your champion's options during its turn.

Inputs:
- target: &T

```rust
let subject = bc::self_champion()?;

let result = subject.attack(target)?;
```

### base_stats

```rust
pub fn base_stats(&self) -> Result<ChampionStats>
```

Query immutable engine design stats, not current modified combat stats.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.base_stats()?;
```

### buy

```rust
pub fn buy(&self, kind: ItemType) -> Result<()>
```

Buy one item for self; engine validates location, gold and recipe. # Arguments * `kind` - Item to buy.

Evaluate your champion's options during its turn.

Inputs:
- kind: ItemType

```rust
let subject = bc::self_champion()?;

let result = subject.buy(kind)?;
```

### buy_consumable

```rust
pub fn buy_consumable(&self, kind: ConsumableType) -> Result<()>
```

Buy one consumable for self; engine validates gold and carry limit. # Arguments * `kind` - Consumable to buy.

Evaluate your champion's options during its turn.

Inputs:
- kind: ConsumableType

```rust
let subject = bc::self_champion()?;

let result = subject.buy_consumable(kind)?;
```

### buy_count

```rust
pub fn buy_count(&self, kind: ItemType, count: u32) -> Result<()>
```

Buy `count` items for self in one engine request. # Arguments * `kind` - Item to buy. * `count` - Number of copies to buy.

Evaluate your champion's options during its turn.

Inputs:
- kind: ItemType
- count: u32

```rust
let subject = bc::self_champion()?;

let result = subject.buy_count(kind, count)?;
```

### can_attack

```rust
pub fn can_attack(&self) -> Result<bool>
```

Check whether this champion can attack.

Check the action allowance before spending fuel to choose an attack target.

```rust
let me = bc::self_champion()?;
let action_available = me.can_attack()?;
```

### can_buy

```rust
pub fn can_buy(&self, item: ItemType, count: u32) -> Result<bool>
```

Check whether an item can be bought in the current state. # Arguments * `item` - Item to buy. * `count` - Number of copies to buy.

Evaluate your champion's options during its turn.

Inputs:
- item: ItemType
- count: u32

```rust
let subject = bc::self_champion()?;

let can_buy = subject.can_buy(item, count)?;
```

### can_buy_consumable

```rust
pub fn can_buy_consumable(&self, consumable: ConsumableType) -> Result<bool>
```

Check whether a consumable can be bought in the current state. # Arguments * `consumable` - Consumable to buy.

Evaluate your champion's options during its turn.

Inputs:
- consumable: ConsumableType

```rust
let subject = bc::self_champion()?;

let can_buy_consumable = subject.can_buy_consumable(consumable)?;
```

### can_cast

```rust
pub fn can_cast(&self) -> Result<bool>
```

Check whether this champion can cast.

Check the action allowance before spending fuel to evaluate ready abilities.

```rust
let me = bc::self_champion()?;
let action_available = me.can_cast()?;
```

### can_move

```rust
pub fn can_move(&self) -> Result<bool>
```

Check whether this champion can move.

Check the action allowance before spending fuel to plan a path.

```rust
let me = bc::self_champion()?;
let action_available = me.can_move()?;
```

### can_sell

```rust
pub fn can_sell(&self) -> Result<bool>
```

Query self's current selling permission.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let can_sell = subject.can_sell()?;
```

### cancel_recall

```rust
pub fn cancel_recall(&self) -> Result<()>
```

Cancel self's recall; returns an error when no recall is active.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.cancel_recall()?;
```

### cancel_submit_gems

```rust
pub fn cancel_submit_gems(&self) -> Result<()>
```

Cancel self's gem submission; returns an error when none is active.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.cancel_submit_gems()?;
```

### champion_name

```rust
pub fn champion_name(&self) -> Result<ChampionType>
```

Return this champion's type; live champions require a host query.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.champion_name()?;
```

### consumables

```rust
pub fn consumables(&self) -> &Consumables
```



Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.consumables();
```

### end_turn

```rust
pub fn end_turn(&self) -> Result<()>
```

End self's current turn and yield; other/design champions return an ownership error.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.end_turn()?;
```

### gem_count

```rust
pub fn gem_count(&self) -> Result<u32>
```

Query the carried gem count of a live champion.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.gem_count()?;
```

### gold_count

```rust
pub fn gold_count(&self) -> Result<u32>
```

Query the self champion's current gold; other champions return an ownership error.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.gold_count()?;
```

### has_completed_turn

```rust
pub fn has_completed_turn(&self) -> Result<bool>
```

Check whether this champion has completed its turn.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let has_completed_turn = subject.has_completed_turn()?;
```

### is_alive

```rust
pub fn is_alive(&self) -> Result<bool>
```

Query whether this live champion is alive; draft designs return an error.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_alive = subject.is_alive()?;
```

### is_ally

```rust
pub fn is_ally(&self) -> Result<bool>
```

Check whether this champion belongs to the observing team.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_ally = subject.is_ally()?;
```

### is_enemy

```rust
pub fn is_enemy(&self) -> Result<bool>
```

Check whether this champion belongs to the opposing team.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_enemy = subject.is_enemy()?;
```

### is_recalling

```rust
pub fn is_recalling(&self) -> Result<bool>
```

Check whether this champion is recalling.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_recalling = subject.is_recalling()?;
```

### is_submitting_gems

```rust
pub fn is_submitting_gems(&self) -> Result<bool>
```

Check whether this champion is submitting gems.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_submitting_gems = subject.is_submitting_gems()?;
```

### is_teleporting

```rust
pub fn is_teleporting(&self) -> Result<bool>
```

Check whether this champion is teleporting.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let is_teleporting = subject.is_teleporting()?;
```

### item_count

```rust
pub fn item_count(&self) -> Result<ItemCounts>
```

Query and return an independent owning item-count snapshot.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.item_count()?;
```

### move_absolute

```rust
pub fn move_absolute(&self, destination: Vec2) -> Result<bool>
```

Move self to an absolute map destination; engine validates movement legality. Returns `Ok(false)` for newly revealed obstructions; known invalid destinations return `Err`. # Arguments * `destination` - Map tile to move to.

Take a planned step; false means no movement occurred.

Inputs:
- destination: a destination selected by your pathfinder

```rust
let me = bc::self_champion()?;
if me.can_move()? {
    let moved = me.move_absolute(destination)?;
}
```

### move_towards

```rust
pub fn move_towards(&self, target: Vec2) -> Result<bool>
```

Move self to the reachable tile on a shortest path to `target`, avoiding walls, holes, altars, the enemy spawn and visible units. Returns `Ok(false)` without moving when already there, unable to move, or `target` is blocked or unreachable, and for a newly revealed obstruction. # Arguments * `target` - Map tile to approach.

Take a planned step; false means no movement occurred.

Inputs:
- destination: a destination selected by your pathfinder

```rust
let me = bc::self_champion()?;
if me.can_move()? {
    let moved = me.move_towards(destination)?;
}
```

### passive

```rust
pub fn passive(&self) -> Result<PassiveType>
```

Query the checked champion passive from its engine design.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.passive()?;
```

### place_ward

```rust
pub fn place_ward(&self, destination: Vec2) -> Result<bool>
```

Place an ordinary ward for self at `destination`; engine validates all placement rules. Returns `Ok(false)` for newly revealed obstructions; known invalid destinations return `Err`. # Arguments * `destination` - Tile to place the ward on.

Evaluate your champion's options during its turn.

Inputs:
- destination: Vec2

```rust
let subject = bc::self_champion()?;

let result = subject.place_ward(destination)?;
```

### prev_priority

```rust
pub fn prev_priority(&self) -> Result<i32>
```

Query the previous scheduling priority for this live champion.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.prev_priority()?;
```

### priority

```rust
pub fn priority(&self) -> Result<i32>
```

Query the current/next scheduling priority for this live champion.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.priority()?;
```

### r#move

```rust
pub fn r#move(&self, delta: Vec2) -> Result<bool>
```

Move self by a displacement from the freshly queried current position. Returns `Ok(false)` for newly revealed obstructions; known invalid destinations return `Err`. # Arguments * `delta` - Displacement from the current position.

Take a planned step; false means no movement occurred.

Inputs:
- direction: a legal unit direction, such as east

```rust
let me = bc::self_champion()?;
if me.can_move()? {
    let moved = me.r#move(direction)?;
}
```

### recall

```rust
pub fn recall(&self) -> Result<()>
```

Begin recalling self; the engine validates channel and action restrictions.

Recall when badly injured and safe from interruption.

Inputs:
- safe_to_recall: your bot's assessment that recalling here is safe and legal

```rust
let me = bc::self_champion()?;
if safe_to_recall && me.health()? < me.max_health()? * 0.3 {
    me.recall()?;
}
```

### remaining_channel_time

```rust
pub fn remaining_channel_time(&self) -> Result<i32>
```

Query the current channel countdown for a visible live champion.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.remaining_channel_time()?;
```

### restrictions

```rust
pub fn restrictions(&self) -> Result<BuffRestrictions>
```

Query current action-restriction bits; distinct from active buff-kind flags.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.restrictions()?;
```

### sell

```rust
pub fn sell(&self, kind: ItemType) -> Result<()>
```

Sell one item for self; engine validates inventory and location. # Arguments * `kind` - Item to sell.

Evaluate your champion's options during its turn.

Inputs:
- kind: ItemType

```rust
let subject = bc::self_champion()?;

let result = subject.sell(kind)?;
```

### sell_count

```rust
pub fn sell_count(&self, kind: ItemType, count: u32) -> Result<()>
```

Sell sequentially `count` times, stopping at the first failure. Earlier successful sales are not rolled back. # Arguments * `kind` - Item to sell. * `count` - Number of copies to sell.

Evaluate your champion's options during its turn.

Inputs:
- kind: ItemType
- count: u32

```rust
let subject = bc::self_champion()?;

let result = subject.sell_count(kind, count)?;
```

### set_priority

```rust
pub fn set_priority(&self, priority: i32) -> Result<()>
```

Set self's scheduling priority through the engine. # Arguments * `priority` - New scheduling priority; takes effect from the next round and persists.

Evaluate your champion's options during its turn.

Inputs:
- priority: i32

```rust
let subject = bc::self_champion()?;

let result = subject.set_priority(priority)?;
```

### state

```rust
pub fn state(&self) -> Result<ChampionState>
```

Query champion state flags, preserving all engine bits including unknown future bits.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.state()?;
```

### submit_gems

```rust
pub fn submit_gems(&self, altar: Vec2) -> Result<()>
```

Begin submitting self's carried gems to `altar`; engine validates range and channel state. # Arguments * `altar` - Altar tile to tribute at.

Evaluate your champion's options during its turn.

Inputs:
- altar: Vec2

```rust
let subject = bc::self_champion()?;

let result = subject.submit_gems(altar)?;
```

### team

```rust
pub fn team(&self) -> Result<Team>
```

Query the champion's team relative to the guest perspective.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.team()?;
```

### visible_tiles

```rust
pub fn visible_tiles(&self) -> Result<Bitboard>
```

Query a fresh owning vision mask for this live champion.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.visible_tiles()?;
```

### visible_tiles_from

```rust
pub fn visible_tiles_from(&self, position: Vec2) -> Result<Bitboard>
```

Query an owning hypothetical vision mask at `position`, not current visibility. # Arguments * `position` - Hypothetical tile to see from.

Evaluate your champion's options during its turn.

Inputs:
- position: Vec2

```rust
let subject = bc::self_champion()?;

let result = subject.visible_tiles_from(position)?;
```

### visible_tiles_from_into

```rust
pub fn visible_tiles_from_into(&self, position: Vec2, out: &mut Bitboard) -> Result<()>
```

Like [`Self::visible_tiles_from`], but writes into `out`. # Arguments * `position` - Hypothetical tile to see from. * `out` - Destination mask; its previous contents are overwritten.

Evaluate your champion's options during its turn.

Inputs:
- position: Vec2
- out: &mut Bitboard

```rust
let subject = bc::self_champion()?;

let result = subject.visible_tiles_from_into(position, out)?;
```

### visible_tiles_into

```rust
pub fn visible_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::visible_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Evaluate your champion's options during its turn.

Inputs:
- out: &mut Bitboard

```rust
let subject = bc::self_champion()?;

let result = subject.visible_tiles_into(out)?;
```

### ward_cooldown

```rust
pub fn ward_cooldown(&self) -> Result<i32>
```

Query the current champion's ward-resource cooldown; requires self.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.ward_cooldown()?;
```

### ward_placement_tiles

```rust
pub fn ward_placement_tiles(&self) -> Result<Bitboard>
```

Build the self champion's placement-range mask using initialised constants. This geometric mask alone does not guarantee that ward placement is legal. # Panics Panics if SDK initialisation failed.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.ward_placement_tiles()?;
```

### ward_placement_tiles_into

```rust
pub fn ward_placement_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::ward_placement_tiles`], but writes into `out`. # Panics Panics if SDK initialisation failed. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Evaluate your champion's options during its turn.

Inputs:
- out: &mut Bitboard

```rust
let subject = bc::self_champion()?;

let result = subject.ward_placement_tiles_into(out)?;
```

### ward_positions

```rust
pub fn ward_positions(&self) -> Result<Vec<Vec2>>
```

Query owning positions of the current champion's ordinary wards; requires self.

Evaluate your champion's options during its turn.

```rust
let subject = bc::self_champion()?;

let result = subject.ward_positions()?;
```

## cambc::ChampionState

### all

```rust
pub fn all() -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let result = bc::ChampionState::all();
```

### bits

```rust
pub fn bits(&self) -> u32
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.bits();
```

### complement

```rust
pub fn complement(self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.complement();
```

### contains

```rust
pub fn contains(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.contains(other);
```

### difference

```rust
pub fn difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.difference(other);
```

### empty

```rust
pub fn empty() -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let result = bc::ChampionState::empty();
```

### from_bits

```rust
pub fn from_bits(bits: u32) -> Option<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u32

```rust
let result = bc::ChampionState::from_bits(bits);
```

### from_bits_retain

```rust
pub fn from_bits_retain(bits: u32) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u32

```rust
let result = bc::ChampionState::from_bits_retain(bits);
```

### from_bits_truncate

```rust
pub fn from_bits_truncate(bits: u32) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- bits: u32

```rust
let result = bc::ChampionState::from_bits_truncate(bits);
```

### from_name

```rust
pub fn from_name(name: &str) -> Option<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- name: &str

```rust
let result = bc::ChampionState::from_name(name);
```

### insert

```rust
pub fn insert(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.insert(other);
```

### intersection

```rust
pub fn intersection(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.intersection(other);
```

### intersects

```rust
pub fn intersects(&self, other: Self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.intersects(other);
```

### is_all

```rust
pub fn is_all(&self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let is_all = subject.is_all();
```

### is_dead

```rust
pub fn is_dead(&self) -> bool
```



Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let is_dead = subject.is_dead();
```

### is_empty

```rust
pub fn is_empty(&self) -> bool
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let is_empty = subject.is_empty();
```

### iter

```rust
pub fn iter(&self) -> bitflags::iter::Iter<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.iter();
```

### iter_names

```rust
pub fn iter_names(&self) -> bitflags::iter::IterNames<Self>
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.iter_names();
```

### remove

```rust
pub fn remove(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.remove(other);
```

### set

```rust
pub fn set(&mut self, other: Self, value: bool)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self
- value: bool

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.set(other, value);
```

### symmetric_difference

```rust
pub fn symmetric_difference(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.symmetric_difference(other);
```

### toggle

```rust
pub fn toggle(&mut self, other: Self)
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.toggle(other);
```

### union

```rust
pub fn union(self, other: Self) -> Self
```

Flag-set operation provided by bitflags.

Combine or inspect flags before deciding which actions or targets are allowed.

Inputs:
- other: Self

```rust
let mut subject = bc::self_champion()?.state()?;

let result = subject.union(other);
```

## cambc::Consumable

### count

```rust
pub fn count(&self) -> Result<u32>
```

Query the owning live champion's count of this consumable type.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::Consumable

```rust
let result = subject.count()?;
```

### kind

```rust
pub fn kind(&self) -> ConsumableType
```



Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::Consumable

```rust
let result = subject.kind();
```

### use_at

```rust
pub fn use_at(&self, destination: Vec2) -> Result<bool>
```

Use this consumable at `destination`; requires the self champion. Returns `Ok(false)` for newly revealed obstructions; known invalid destinations return `Err`. # Arguments * `destination` - Tile to use the consumable at.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::Consumable
- destination: Vec2

```rust
let result = subject.use_at(destination)?;
```

### use_untargeted

```rust
pub fn use_untargeted(&self) -> Result<bool>
```

Use this consumable without a destination; requires the self champion. Returns `Ok(false)` for newly revealed obstructions; known invalid destinations return `Err`.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::Consumable

```rust
let result = subject.use_untargeted()?;
```

## cambc::Consumables

### iter

```rust
pub fn iter(&self) -> impl Iterator<Item = &Consumable>
```

Iterate consumables in SDK order.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::Consumables

```rust
let result = subject.iter();
```

## cambc::ConsumableType

### carry_limit

```rust
pub fn carry_limit(self) -> Result<u32>
```

Get the maximum carried inventory count.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ConsumableType

```rust
let result = subject.carry_limit()?;
```

### cost

```rust
pub fn cost(self) -> Result<u32>
```

Get the purchase cost in gold.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ConsumableType

```rust
let result = subject.cost()?;
```

### info

```rust
pub fn info(self) -> Result<ConsumableInfo>
```

Query a fresh owned consumable cost/carry-limit record, preserving engine errors.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ConsumableType

```rust
let result = subject.info()?;
```

## cambc::debug

### draw_dot

```rust
pub fn draw_dot(position: Vec2, rgb: u32) -> Result<()>
```

Draw a replay debug dot at `position` using packed RGB colour. # Arguments * `position` - Tile to mark. * `rgb` - Packed `0xRRGGBB` colour.

Mark a decision in your replay debug overlay to understand the bot's route or target.

Inputs:
- position: Vec2
- rgb: u32

```rust
let result = bc::debug::draw_dot(position, rgb)?;
```

### draw_line

```rust
pub fn draw_line(from: Vec2, to: Vec2, rgb: u32) -> Result<()>
```

Draw a replay debug line from `from` to `to` using packed RGB colour. # Arguments * `from` - Start tile. * `to` - End tile. * `rgb` - Packed `0xRRGGBB` colour.

Mark a decision in your replay debug overlay to understand the bot's route or target.

Inputs:
- from: Vec2
- to: Vec2
- rgb: u32

```rust
let result = bc::debug::draw_line(from, to, rgb)?;
```

## cambc::DistanceField

### compute

```rust
pub fn compute( &mut self, walls: &Bitboard, roots: &Bitboard, visited: &mut Bitboard, max_distance: u16, ) -> Result<()>
```

Compute distances from `roots` around `walls`, writing both this field and `visited`. Expansion stops at `max_distance`; buffers must have compatible map layouts. # Arguments * `walls` - Impassable tiles. * `roots` - Tiles at distance zero. * `visited` - Receives the tiles the search reached. * `max_distance` - Tiles farther than this are left unreached.

Find a step towards the goal tiles. The result is a direction for move.

Inputs:
- blocked: impassable/occupied tiles, excluding your own tile
- goals: legal destination tiles

```rust
let mut field = bc::DistanceField::new()?;
let mut visited = bc::Bitboard::new()?;
field.compute(&blocked, &goals, &mut visited, u16::MAX)?;
let direction = field.next_move_from(bc::self_champion()?.position()?)?;
```

### compute_to

```rust
pub fn compute_to( &mut self, walls: &Bitboard, roots: &Bitboard, destinations: &Bitboard, visited: &mut Bitboard, max_distance: u16, ) -> Result<()>
```

Compute distances towards `destinations`, writing this field and `visited`. The host stops when destinations or the distance limit end the search. # Arguments * `walls` - Impassable tiles. * `roots` - Tiles at distance zero. * `destinations` - The search stops once these tiles are reached. * `visited` - Receives the tiles the search reached. * `max_distance` - Tiles farther than this are left unreached.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- walls: &Bitboard
- roots: &Bitboard
- destinations: &Bitboard
- visited: &mut Bitboard
- max_distance: u16

```rust
let result = subject.compute_to(walls, roots, destinations, visited, max_distance)?;
```

### compute_to_with_expansion_count

```rust
pub fn compute_to_with_expansion_count( &mut self, walls: &Bitboard, roots: &Bitboard, destinations: &Bitboard, visited: &mut Bitboard, max_distance: u16, ) -> Result<u16>
```

Compute towards `destinations` and return the host expansion count. # Arguments * `walls` - Impassable tiles. * `roots` - Tiles at distance zero. * `destinations` - The search stops once these tiles are reached. * `visited` - Receives the tiles the search reached. * `max_distance` - Tiles farther than this are left unreached.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- walls: &Bitboard
- roots: &Bitboard
- destinations: &Bitboard
- visited: &mut Bitboard
- max_distance: u16

```rust
let result = subject.compute_to_with_expansion_count(walls, roots, destinations, visited, max_distance)?;
```

### compute_with_expansion_count

```rust
pub fn compute_with_expansion_count( &mut self, walls: &Bitboard, roots: &Bitboard, visited: &mut Bitboard, max_distance: u16, ) -> Result<u16>
```

Compute distances and return the host expansion count, writing this field and `visited`. # Arguments * `walls` - Impassable tiles. * `roots` - Tiles at distance zero. * `visited` - Receives the tiles the search reached. * `max_distance` - Tiles farther than this are left unreached.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- walls: &Bitboard
- roots: &Bitboard
- visited: &mut Bitboard
- max_distance: u16

```rust
let result = subject.compute_with_expansion_count(walls, roots, visited, max_distance)?;
```

### export_linear_buffer

```rust
pub fn export_linear_buffer(&self) -> Result<Vec<u16>>
```

Return an owning row-major buffer without chunk padding.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField

```rust
let result = subject.export_linear_buffer()?;
```

### export_linear_buffer_into

```rust
pub fn export_linear_buffer_into(&self, values: &mut [u16]) -> Result<()>
```

Export row-major tile values into an exact-size caller buffer. # Arguments * `values` - Destination with one distance per tile in row-major order.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField
- values: &mut [u16]

```rust
let result = subject.export_linear_buffer_into(values)?;
```

### export_raw_buffer

```rust
pub fn export_raw_buffer(&self) -> Result<Vec<u8>>
```

Return an owning copy of the chunked storage bytes.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField

```rust
let result = subject.export_raw_buffer()?;
```

### export_raw_buffer_into

```rust
pub fn export_raw_buffer_into(&self, bytes: &mut [u8]) -> Result<()>
```

Export chunked bytes into caller storage; its size must match `raw_buffer_size`. # Arguments * `bytes` - Destination, exactly `raw_buffer_size` long.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField
- bytes: &mut [u8]

```rust
let result = subject.export_raw_buffer_into(bytes)?;
```

### get

```rust
pub fn get(&self, p: Vec2) -> Result<u16>
```

Query the distance at `p`; invalid coordinates propagate an engine error. # Arguments * `p` - Tile to query.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField
- p: Vec2

```rust
let result = subject.get(p)?;
```

### height

```rust
pub fn height(&self) -> i32
```



### import_linear_buffer

```rust
pub fn import_linear_buffer(&mut self, values: &[u16]) -> Result<()>
```

Import row-major tile values; host validates the exact buffer size. # Arguments * `values` - One distance per tile in row-major order.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- values: &[u16]

```rust
let result = subject.import_linear_buffer(values)?;
```

### import_raw_buffer

```rust
pub fn import_raw_buffer(&mut self, bytes: &[u8]) -> Result<()>
```

Import exact-size chunked bytes; wrong size or invalid layout propagates an error. # Arguments * `bytes` - Engine-chunked bytes, exactly `raw_buffer_size` long.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- bytes: &[u8]

```rust
let result = subject.import_raw_buffer(bytes)?;
```

### linear_buffer_size

```rust
pub fn linear_buffer_size(&self) -> usize
```



Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField

```rust
let result = subject.linear_buffer_size();
```

### new

```rust
pub fn new() -> Result<Self>
```

Allocate aligned owning distance storage from a freshly queried, validated layout. No global initialisation or cached layout is required; compute or set values before use.

Choose a route around blocked tiles.

```rust
let result = bc::DistanceField::new()?;
```

### next_move_from

```rust
pub fn next_move_from(&self, from: Vec2) -> Result<Option<Vec2>>
```

First optimal step from `from` towards a root, as a direction delta in compass order from north. Returns `None` at a root or unreached tile; an out-of-map `from` propagates an engine error. # Arguments * `from` - Tile to step from.

Find a step towards the goal tiles. The result is a direction for move.

Inputs:
- blocked: impassable/occupied tiles, excluding your own tile
- goals: legal destination tiles

```rust
let mut field = bc::DistanceField::new()?;
let mut visited = bc::Bitboard::new()?;
field.compute(&blocked, &goals, &mut visited, u16::MAX)?;
let direction = field.next_move_from(bc::self_champion()?.position()?)?;
```

### next_moves_from

```rust
pub fn next_moves_from(&self, from: Vec2) -> Result<Vec<Vec2>>
```

Every optimal step from `from` towards a root, as direction deltas in compass order from north. # Arguments * `from` - Tile to step from.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField
- from: Vec2

```rust
let result = subject.next_moves_from(from)?;
```

### print

```rust
pub fn print(&self) -> Result<()>
```

Print the stored board/field through the host debug primitive.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField

```rust
let result = subject.print()?;
```

### raw_buffer_size

```rust
pub fn raw_buffer_size(&self) -> usize
```



Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField

```rust
let result = subject.raw_buffer_size();
```

### set

```rust
pub fn set(&mut self, p: Vec2, value: u16) -> Result<()>
```

Set the distance at `p`; invalid coordinates propagate an engine error. # Arguments * `p` - Tile to update. * `value` - Distance to store.

Choose a route around blocked tiles.

Inputs:
- subject: bc::DistanceField (mutable)
- p: Vec2
- value: u16

```rust
let result = subject.set(p, value)?;
```

### width

```rust
pub fn width(&self) -> i32
```



## cambc::draft

### current_draft

```rust
pub fn current_draft(team: Team) -> Result<Vec<ChampionDraft>>
```

Return this team's completed draft picks in round order, skipping unavailable rounds. # Arguments * `team` - Team whose picks to return.

Choose your team's roster during the draft phase.

Inputs:
- team: Team

```rust
let result = bc::draft::current_draft(team)?;
```

### pick

```rust
pub fn pick(kind: ChampionDraft) -> Result<()>
```

Submit one draft selection; phase and availability failures propagate engine errors. # Arguments * `kind` - Champion to pick.

Choose your team's roster during the draft phase.

Inputs:
- kind: ChampionDraft

```rust
let result = bc::draft::pick(kind)?;
```

### team_size

```rust
pub fn team_size() -> Result<usize>
```

Query the number of champions per team.

Choose your team's roster during the draft phase.

```rust
let result = bc::draft::team_size()?;
```

## cambc::DraftMemory

### bytes

```rust
pub fn bytes(&self) -> &[u8]
```



Pass your drafting decisions to the bot's later turns using draft memory.

Inputs:
- subject: bc::DraftMemory

```rust
let result = subject.bytes();
```

### get

```rust
pub fn get(&self, offset: usize) -> Result<u8>
```

Read one byte from the startup snapshot. # Arguments * `offset` - Byte offset into the draft memory snapshot.

Pass your drafting decisions to the bot's later turns using draft memory.

Inputs:
- subject: bc::DraftMemory
- offset: usize

```rust
let result = subject.get(offset)?;
```

### len

```rust
pub fn len(&self) -> usize
```



### submit

```rust
pub fn submit(&self, bytes: &[u8]) -> Result<()>
```

Submit caller bytes during draft for transfer to gameplay guests. Gameplay submissions return `InvalidAction`; the startup snapshot is unchanged. # Arguments * `bytes` - Bytes to pass to this team's gameplay bots.

Pass your drafting decisions to the bot's later turns using draft memory.

Inputs:
- subject: bc::DraftMemory
- bytes: &[u8]

```rust
let result = subject.submit(bytes)?;
```

## cambc::Error

### code

```rust
pub fn code(&self) -> Option<ErrorCode>
```



Inspect a caught SDK error when diagnosing a rejected action.

Inputs:
- subject: bc::Error

```rust
let result = subject.code();
```

### raw_code

```rust
pub fn raw_code(&self) -> Option<i32>
```



Inspect a caught SDK error when diagnosing a rejected action.

Inputs:
- subject: bc::Error

```rust
let result = subject.raw_code();
```

## cambc::ItemCounts

### iter

```rust
pub fn iter(&self) -> impl Iterator<Item = (ItemType, u32)> + '_
```

Iterate over every item type and captured count, including zero counts.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemCounts

```rust
let result = subject.iter();
```

## cambc::ItemType

### combine_count

```rust
pub fn combine_count(self) -> Result<u32>
```

Get the ingredient count needed to create the result item.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.combine_count()?;
```

### cost

```rust
pub fn cost(self) -> Result<u32>
```

Get the full value in gold: the purchase price, refunded in full when sold.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.cost()?;
```

### focus_points

```rust
pub fn focus_points(self) -> Result<i32>
```

Get the focus bonus.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.focus_points()?;
```

### health_points

```rust
pub fn health_points(self) -> Result<i32>
```

Get the health bonus.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.health_points()?;
```

### info

```rust
pub fn info(self) -> Result<ItemInfo>
```

Query a fresh owned item record, checking passive/result tags. Engine errors and unknown runtime enum values remain explicit SDK errors.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.info()?;
```

### is_purchasable

```rust
pub fn is_purchasable(self) -> Result<bool>
```

Whether the item can be bought directly; emblems must be combined.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let is_purchasable = subject.is_purchasable()?;
```

### passive

```rust
pub fn passive(self) -> Result<Option<ItemPassive>>
```

Get the optional passive effect.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.passive()?;
```

### passive_magnitude

```rust
pub fn passive_magnitude(self) -> Result<f32>
```

Get the passive magnitude.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.passive_magnitude()?;
```

### result

```rust
pub fn result(self) -> Result<Option<ItemType>>
```

Get the combination result, if this item can be combined.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.result()?;
```

### strength_points

```rust
pub fn strength_points(self) -> Result<i32>
```

Get the strength bonus.

Plan a purchase or use an owned consumable.

Inputs:
- subject: bc::ItemType

```rust
let result = subject.strength_points()?;
```

## cambc::map

### altars

```rust
pub fn altars() -> Result<&'static Bitboard>
```

Borrow the initialised altar mask; errors if SDK initialisation failed.

Build a terrain-only walking mask. Also exclude occupied tiles and the enemy spawn when selecting a legal step.

```rust
let blocked = bc::map::walls()?.union(&bc::map::holes()?)?.union(&bc::map::altars()?)?;
let walkable = blocked.complement()?;
```

### bushes

```rust
pub fn bushes() -> Result<&'static Bitboard>
```

Borrow the initialised bush mask; errors if SDK initialisation failed.

Inspect map geometry when choosing a route or objective.

```rust
let result = bc::map::bushes()?;
```

### camp

```rust
pub fn camp(position: Vec2) -> Result<Camp>
```

Find the camp at an immutable spawn position, querying the host on every call. No spawn returns `ApiNotFound`; this is not a live-position/visibility lookup. # Arguments * `position` - Spawn tile of the camp.

Inspect map geometry when choosing a route or objective.

Inputs:
- position: Vec2

```rust
let result = bc::map::camp(position)?;
```

### camp_groups

```rust
pub fn camp_groups() -> Result<Vec<CampGroup>>
```

Query engine groups on every call, including merged symmetry-seam clusters; also available in draft.

Inspect map geometry when choosing a route or objective.

```rust
let result = bc::map::camp_groups()?;
```

### height

```rust
pub fn height() -> Result<i32>
```

Read initialised map height without querying or initialising the host cache.

### holes

```rust
pub fn holes() -> Result<&'static Bitboard>
```

Borrow the initialised hole mask; errors if SDK initialisation failed.

Build a terrain-only walking mask. Also exclude occupied tiles and the enemy spawn when selecting a legal step.

```rust
let blocked = bc::map::walls()?.union(&bc::map::holes()?)?.union(&bc::map::altars()?)?;
let walkable = blocked.complement()?;
```

### is_in_bounds

```rust
pub fn is_in_bounds(p: Vec2) -> Result<bool>
```

Check a position against initialised map dimensions; errors if SDK initialisation failed. # Arguments * `p` - Position to check.

Inspect map geometry when choosing a route or objective.

Inputs:
- p: Vec2

```rust
let is_in_bounds = bc::map::is_in_bounds(p)?;
```

### reflect_horizontal

```rust
pub fn reflect_horizontal(position: Vec2) -> Result<Vec2>
```

Reflect a map position horizontally, reversing tile x coordinates. # Arguments * `position` - Map position to reflect.

Inspect map geometry when choosing a route or objective.

Inputs:
- position: Vec2

```rust
let result = bc::map::reflect_horizontal(position)?;
```

### reflect_vertical

```rust
pub fn reflect_vertical(position: Vec2) -> Result<Vec2>
```

Reflect a map position vertically, reversing tile y coordinates. # Arguments * `position` - Map position to reflect.

Inspect map geometry when choosing a route or objective.

Inputs:
- position: Vec2

```rust
let result = bc::map::reflect_vertical(position)?;
```

### rotate_180

```rust
pub fn rotate_180(position: Vec2) -> Result<Vec2>
```

Rotate a map position 180 degrees within the map dimensions. # Arguments * `position` - Map position to rotate.

Inspect map geometry when choosing a route or objective.

Inputs:
- position: Vec2

```rust
let result = bc::map::rotate_180(position)?;
```

### spawn_region

```rust
pub fn spawn_region(team: Team) -> Result<&'static Bitboard>
```

Borrow the initialised spawn-region mask for the relative team. # Arguments * `team` - Team whose spawn region to return.

Inspect map geometry when choosing a route or objective.

Inputs:
- team: Team

```rust
let result = bc::map::spawn_region(team)?;
```

### symmetry

```rust
pub fn symmetry() -> Result<Symmetry>
```

Read the initialised map symmetry without a host query.

Inspect map geometry when choosing a route or objective.

```rust
let result = bc::map::symmetry()?;
```

### walls

```rust
pub fn walls() -> Result<&'static Bitboard>
```

Borrow the initialised wall mask; errors if SDK initialisation failed.

Build a terrain-only walking mask. Also exclude occupied tiles and the enemy spawn when selecting a legal step.

```rust
let blocked = bc::map::walls()?.union(&bc::map::holes()?)?.union(&bc::map::altars()?)?;
let walkable = blocked.complement()?;
```

### width

```rust
pub fn width() -> Result<i32>
```

Read initialised map width without querying or initialising the host cache.

## cambc::SharedMemory

### get

```rust
pub fn get(&self, offset: usize) -> Result<u8>
```

Read one bounds-checked byte, preserving engine errors. # Arguments * `offset` - Byte offset into shared memory.

Share an objective using your team's agreed memory layout.

Inputs:
- subject: bc::SharedMemory
- offset: usize

```rust
let result = subject.get(offset)?;
```

### read

```rust
pub fn read(&self, offset: usize, bytes: &mut [u8]) -> Result<()>
```

Read bytes at a byte offset, preserving engine errors. The entire range must fit; an empty read at capacity is allowed and makes no host call. # Arguments * `offset` - Byte offset into shared memory. * `bytes` - Receives the bytes; its length sets how many are read.

Reserve byte zero in your team's memory layout for a shared objective ID. All allied bots must agree on its meaning.

```rust
let mut data = [0u8; 1];
bc::shared_memory().read(0, &mut data)?;
let objective_id = data[0];
```

### read_values

```rust
pub fn read_values<T: bytemuck::Pod>(&self, offset: usize, values: &mut [T]) -> Result<()>
```

Read a typed slice at a byte offset using native layout and byte order, like C++ spans. Only plain-data types with no padding and valid arbitrary bit patterns may be read. # Arguments * `offset` - Byte offset into shared memory. * `values` - Receives the values; its length sets how many are read.

Share an objective using your team's agreed memory layout.

Inputs:
- subject: bc::SharedMemory
- offset: usize
- values: &mut [T]

```rust
let result = subject.read_values(offset, values)?;
```

### set

```rust
pub fn set(&self, offset: usize, value: u8) -> Result<()>
```

Write one bounds-checked byte, preserving engine errors. # Arguments * `offset` - Byte offset into shared memory. * `value` - Byte to store.

Share an objective using your team's agreed memory layout.

Inputs:
- subject: bc::SharedMemory
- offset: usize
- value: u8

```rust
let result = subject.set(offset, value)?;
```

### size

```rust
pub fn size(&self) -> usize
```



### write

```rust
pub fn write(&self, offset: usize, bytes: &[u8]) -> Result<()>
```

Write bytes at a byte offset, preserving engine errors. The entire range must fit; an empty write at capacity is allowed and makes no host call. # Arguments * `offset` - Byte offset into shared memory. * `bytes` - Bytes to write.

Reserve byte zero in your team's memory layout for a shared objective ID. All allied bots must agree on its meaning.

```rust
bc::shared_memory().write(0, &[2])?; // Agree to focus objective 2.
```

### write_values

```rust
pub fn write_values<T: bytemuck::NoUninit>(&self, offset: usize, values: &[T]) -> Result<()>
```

Write a typed slice at a byte offset using native layout and byte order, like C++ spans. Values must have no uninitialised padding bytes. # Arguments * `offset` - Byte offset into shared memory. * `values` - Values to write.

Share an objective using your team's agreed memory layout.

Inputs:
- subject: bc::SharedMemory
- offset: usize
- values: &[T]

```rust
let result = subject.write_values(offset, values)?;
```

## cambc::Team

### opponent

```rust
pub fn opponent(self) -> Self
```



Use this value in your bot's draft or turn logic.

Inputs:
- subject: bc::Team

```rust
let result = subject.opponent();
```

## cambc::Teleport

### champion

```rust
pub fn champion(&self) -> Result<Champion>
```

Construct a live wrapper for the teleport caster, preserving host visibility errors.

Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let result = subject.champion()?;
```

### is_ally

```rust
pub fn is_ally(&self) -> bool
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let is_ally = subject.is_ally();
```

### is_enemy

```rust
pub fn is_enemy(&self) -> bool
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let is_enemy = subject.is_enemy();
```

### position

```rust
pub fn position(&self) -> Vec2
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let result = subject.position();
```

### remaining_time

```rust
pub fn remaining_time(&self) -> i32
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let result = subject.remaining_time();
```

### team

```rust
pub fn team(&self) -> Team
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let result = subject.team();
```

### visible_tiles

```rust
pub fn visible_tiles(&self) -> Result<Bitboard>
```

Query hypothetical caster vision after arrival at the reserved destination. This is not a mask of tiles currently visible to the caster.

Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport

```rust
let result = subject.visible_tiles()?;
```

### visible_tiles_into

```rust
pub fn visible_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::visible_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- subject: bc::Teleport
- out: &mut Bitboard

```rust
let result = subject.visible_tiles_into(out)?;
```

## cambc::Unit

### attack_range

```rust
pub fn attack_range(&self) -> Result<i32>
```

Query attack range; camps use their engine design range.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.attack_range()?;
```

### attack_tiles

```rust
pub fn attack_tiles(&self) -> Result<Bitboard>
```

Build the live unit's Chebyshev attack-range mask without filtering targets or attack availability. Hidden units and design-only champions propagate position/range query errors.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.attack_tiles()?;
```

### attack_tiles_into

```rust
pub fn attack_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::attack_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit
- out: &mut Bitboard

```rust
let result = subject.attack_tiles_into(out)?;
```

### buffs

```rust
pub fn buffs(&self) -> Result<BuffCollection>
```

Construct a fresh owning buff collection; designs and hidden units return an error.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.buffs()?;
```

### focus

```rust
pub fn focus(&self) -> Result<f32>
```

Query current focus.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.focus()?;
```

### health

```rust
pub fn health(&self) -> Result<f32>
```

Query current health points.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.health()?;
```

### is_visible

```rust
pub fn is_visible(&self) -> Result<bool>
```

Query visibility; designs and known hidden/not-found units return `false`. Other engine errors are preserved rather than converted to `false`.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let is_visible = subject.is_visible()?;
```

### max_health

```rust
pub fn max_health(&self) -> Result<f32>
```

Query current maximum health points.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.max_health()?;
```

### position

```rust
pub fn position(&self) -> Result<Vec2>
```

Query the current map position; hidden/design units propagate unavailability.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.position()?;
```

### strength

```rust
pub fn strength(&self) -> Result<f32>
```

Query current strength.

Evaluate your champion's options during its turn.

Inputs:
- subject: bc::Unit

```rust
let result = subject.strength()?;
```

## cambc::Vec2

### chebyshev_distance

```rust
pub fn chebyshev_distance(self, b: Self) -> i64
```

# Arguments * `b` - Other vector.

Compare positions or adjust a movement direction.

Inputs:
- b: Self

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.chebyshev_distance(b);
```

### checked_div

```rust
pub fn checked_div(self, n: i32) -> Result<Self>
```

Divide each coordinate by `n`; zero divisors and signed overflow return an error. # Arguments * `n` - Divisor applied to both coordinates.

Compare positions or adjust a movement direction.

Inputs:
- n: i32

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.checked_div(n)?;
```

### cross

```rust
pub fn cross(self, b: Self) -> i64
```

# Arguments * `b` - Other vector.

Compare positions or adjust a movement direction.

Inputs:
- b: Self

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.cross(b);
```

### dot

```rust
pub fn dot(self, b: Self) -> i64
```

# Arguments * `b` - Other vector.

Compare positions or adjust a movement direction.

Inputs:
- b: Self

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.dot(b);
```

### length_squared

```rust
pub fn length_squared(self) -> i64
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.length_squared();
```

### new

```rust
pub const fn new(x: i32, y: i32) -> Self
```



### reflect_horizontal

```rust
pub const fn reflect_horizontal(self) -> Self
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.reflect_horizontal();
```

### reflect_vertical

```rust
pub const fn reflect_vertical(self) -> Self
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.reflect_vertical();
```

### rotate_180

```rust
pub const fn rotate_180(self) -> Self
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.rotate_180();
```

### rotate_ccw

```rust
pub const fn rotate_ccw(self) -> Self
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.rotate_ccw();
```

### rotate_cw

```rust
pub const fn rotate_cw(self) -> Self
```



Compare positions or adjust a movement direction.

```rust
let subject = bc::Vec2::new(3, 4);

let result = subject.rotate_cw();
```

## cambc::vision

### altar_lock_at

```rust
pub fn altar_lock_at(position: Vec2) -> Result<AltarLock>
```

Return an owning visible altar lock at `position`, preferring ally then enemy. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let result = bc::vision::altar_lock_at(position)?;
```

### altar_lock_tiles

```rust
pub fn altar_lock_tiles(team: Option<Team>) -> Result<Bitboard>
```

Query visible altar-lock occupancy for `team`, or both sides when `None`. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::altar_lock_tiles(team)?;
```

### altar_lock_tiles_into

```rust
pub fn altar_lock_tiles_into(team: Option<Team>, out: &mut Bitboard) -> Result<()>
```

Like [`altar_lock_tiles`], but writes into `out`. # Arguments * `team` - `Some(team)` for one side, or `None` for both. * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>
- out: &mut Bitboard

```rust
let result = bc::vision::altar_lock_tiles_into(team, out)?;
```

### altar_locks

```rust
pub fn altar_locks(team: Option<Team>) -> Result<Vec<AltarLock>>
```

Return owning visible altar-lock snapshots, optionally filtered by team. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::altar_locks(team)?;
```

### camp_at

```rust
pub fn camp_at(position: Vec2) -> Result<Camp>
```

Look up a visible camp at its current position and borrow initialised metadata. Absence and hidden tiles are errors; unknown handles and uninitialised caches remain errors. # Arguments * `position` - Current tile of the camp.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let result = bc::vision::camp_at(position)?;
```

### camp_groups

```rust
pub fn camp_groups() -> Result<Vec<CampGroup>>
```

Query groups with any camp spawn tile in current vision.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::camp_groups()?;
```

### camp_tiles

```rust
pub fn camp_tiles() -> Result<Bitboard>
```

Query the current visible camp occupancy mask.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::camp_tiles()?;
```

### camp_tiles_into

```rust
pub fn camp_tiles_into(out: &mut Bitboard) -> Result<()>
```

Like [`camp_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- out: &mut Bitboard

```rust
let result = bc::vision::camp_tiles_into(out)?;
```

### camps

```rust
pub fn camps() -> Result<Vec<Camp>>
```

Return owning wrappers for currently visible camps, using initialised map metadata.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::camps()?;
```

### champion_at

```rust
pub fn champion_at(position: Vec2) -> Result<Champion>
```

Return a live wrapper for the visible champion at `position`; absence and hidden tiles are errors. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let result = bc::vision::champion_at(position)?;
```

### champion_tiles

```rust
pub fn champion_tiles(team: Option<Team>) -> Result<Bitboard>
```

Query visible champion occupancy for `team`, or both sides when `None`. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::champion_tiles(team)?;
```

### champion_tiles_into

```rust
pub fn champion_tiles_into(team: Option<Team>, out: &mut Bitboard) -> Result<()>
```

Like [`champion_tiles`], but writes into `out`. # Arguments * `team` - `Some(team)` for one side, or `None` for both. * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>
- out: &mut Bitboard

```rust
let result = bc::vision::champion_tiles_into(team, out)?;
```

### champions

```rust
pub fn champions(team: Option<Team>) -> Result<Vec<Champion>>
```

Return owning wrappers for currently visible champions, optionally filtered by team. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::champions(team)?;
```

### has_altar_lock

```rust
pub fn has_altar_lock(position: Vec2) -> Result<bool>
```

Test for a visible altar lock for either team; missing or hidden locks return `false`. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let has_altar_lock = bc::vision::has_altar_lock(position)?;
```

### has_camp

```rust
pub fn has_camp(position: Vec2) -> Result<bool>
```

Query visible camp presence at `position`; invalid positions remain errors. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let has_camp = bc::vision::has_camp(position)?;
```

### has_champion

```rust
pub fn has_champion(position: Vec2) -> Result<bool>
```

Query visible champion presence at `position`; invalid positions remain errors. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let has_champion = bc::vision::has_champion(position)?;
```

### has_teleport

```rust
pub fn has_teleport(position: Vec2) -> Result<bool>
```

Query whether a visible teleport reservation exists at `position`. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let has_teleport = bc::vision::has_teleport(position)?;
```

### has_ward

```rust
pub fn has_ward(position: Vec2, team: Option<Team>) -> Result<bool>
```

Test for a visible ward belonging to `team`; missing or hidden wards return `false`. # Arguments * `position` - Tile to query. * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2
- team: Option<Team>

```rust
let has_ward = bc::vision::has_ward(position, team)?;
```

### teleport_at

```rust
pub fn teleport_at(position: Vec2) -> Result<Teleport>
```

Return the visible teleport reservation at `position`. # Arguments * `position` - Tile to query.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let result = bc::vision::teleport_at(position)?;
```

### teleport_tiles

```rust
pub fn teleport_tiles() -> Result<Bitboard>
```

Query current visible teleport-destination occupancy.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::teleport_tiles()?;
```

### teleport_tiles_into

```rust
pub fn teleport_tiles_into(out: &mut Bitboard) -> Result<()>
```

Like [`teleport_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- out: &mut Bitboard

```rust
let result = bc::vision::teleport_tiles_into(out)?;
```

### teleports

```rust
pub fn teleports() -> Result<Vec<Teleport>>
```

Return owning visible teleport reservations with checked caster types and teams.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::teleports()?;
```

### visible_tiles

```rust
pub fn visible_tiles() -> Result<Bitboard>
```

Query a fresh owning mask of current visible tiles.

Check visible objects before choosing a destination.

```rust
let result = bc::vision::visible_tiles()?;
```

### visible_tiles_into

```rust
pub fn visible_tiles_into(out: &mut Bitboard) -> Result<()>
```

Like [`visible_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- out: &mut Bitboard

```rust
let result = bc::vision::visible_tiles_into(out)?;
```

### ward_at

```rust
pub fn ward_at(position: Vec2, team: Option<Team>) -> Result<Ward>
```

Return a visible ordinary ward for `team`. # Arguments * `position` - Tile to query. * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2
- team: Option<Team>

```rust
let result = bc::vision::ward_at(position, team)?;
```

### ward_tiles

```rust
pub fn ward_tiles(team: Option<Team>) -> Result<Bitboard>
```

Query visible ordinary-ward occupancy for `team`, or both sides when `None`. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::ward_tiles(team)?;
```

### ward_tiles_into

```rust
pub fn ward_tiles_into(team: Option<Team>, out: &mut Bitboard) -> Result<()>
```

Like [`ward_tiles`], but writes into `out`. # Arguments * `team` - `Some(team)` for one side, or `None` for both. * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>
- out: &mut Bitboard

```rust
let result = bc::vision::ward_tiles_into(team, out)?;
```

### wards

```rust
pub fn wards(team: Option<Team>) -> Result<Vec<Ward>>
```

Return owning ordinary-ward observations, optionally filtered by team. Overlapping observations remain separate entries. # Arguments * `team` - `Some(team)` for one side, or `None` for both.

Check visible objects before choosing a destination.

Inputs:
- team: Option<Team>

```rust
let result = bc::vision::wards(team)?;
```

## cambc::Ward

### is_ally

```rust
pub fn is_ally(&self) -> bool
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let is_ally = subject.is_ally();
```

### is_enemy

```rust
pub fn is_enemy(&self) -> bool
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let is_enemy = subject.is_enemy();
```

### position

```rust
pub fn position(&self) -> Vec2
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let result = subject.position();
```

### remaining_time

```rust
pub fn remaining_time(&self) -> Option<i32>
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let result = subject.remaining_time();
```

### team

```rust
pub fn team(&self) -> Team
```



Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let result = subject.team();
```

### visible_tiles

```rust
pub fn visible_tiles(&self) -> Result<Bitboard>
```

Query hypothetical vision at the ward's position.

Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward

```rust
let result = subject.visible_tiles()?;
```

### visible_tiles_from

```rust
pub fn visible_tiles_from(position: Vec2) -> Result<Bitboard>
```

Query a fresh hypothetical ordinary-ward vision mask at `position`. # Arguments * `position` - Hypothetical ward tile.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2

```rust
let result = bc::Ward::visible_tiles_from(position)?;
```

### visible_tiles_from_into

```rust
pub fn visible_tiles_from_into(position: Vec2, out: &mut Bitboard) -> Result<()>
```

Like [`Self::visible_tiles_from`], but writes into `out`. # Arguments * `position` - Hypothetical ward tile. * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- position: Vec2
- out: &mut Bitboard

```rust
let result = bc::Ward::visible_tiles_from_into(position, out)?;
```

### visible_tiles_into

```rust
pub fn visible_tiles_into(&self, out: &mut Bitboard) -> Result<()>
```

Like [`Self::visible_tiles`], but writes into `out`. # Arguments * `out` - Destination mask; its previous contents are overwritten.

Check visible objects before choosing a destination.

Inputs:
- subject: bc::Ward
- out: &mut Bitboard

```rust
let result = subject.visible_tiles_into(out)?;
```
