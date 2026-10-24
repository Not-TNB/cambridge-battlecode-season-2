# Named switch edits for abtest.sh, written for bots/bot-v7's defaults (2026-10-23): each one flips
# a default. Format: FILE=SEDEXPR, FILE relative to the bot's src/. copybot.sh fails if an edit
# doesn't change its file, so a renamed or already-flipped switch can't be tested by accident.
# Add a line per new switch; keep each name's direction (ON/OFF) relative to the current default.

# --- v7 gate play (01boss-battle) ---
SEOFF='common/safety.rs=s/^pub const SHIELDED_ESCAPE: bool = true;/pub const SHIELDED_ESCAPE: bool = false;/'
GGOFF='common/base.rs=s/^pub const GATE_GUARD: bool = true;/pub const GATE_GUARD: bool = false;/'
GQOFF='common/base.rs=s/^pub const GATE_QUEUE: bool = true;/pub const GATE_QUEUE: bool = false;/'
GSON='common/collect.rs=s/^pub const GATE_SIEGE: bool = false;/pub const GATE_SIEGE: bool = true;/'
GS1H='common/collect.rs=s/^pub const GATE_SIEGE_ONE_HITTER: bool = false;/pub const GATE_SIEGE_ONE_HITTER: bool = true;/'

# --- v7 fights ---
ACM3='common/chase.rs=s/^pub const ALONE_CHASE_MARGIN: u32 = 2;/pub const ALONE_CHASE_MARGIN: u32 = 3;/'
BOUNTY200='common/teamfight.rs=s/^pub const TEAMFIGHT_BOUNTY_GEMS: f32 = 0.0;/pub const TEAMFIGHT_BOUNTY_GEMS: f32 = 200.0;/'

HSPAMOFF='champions/heart_of_ruin.rs=s/^pub const HEART_SHIELD_SPAM: bool = true;/pub const HEART_SHIELD_SPAM: bool = false;/'

DSHOFF='common/duel.rs=s/^pub const DUEL_SHIELDS: bool = true;/pub const DUEL_SHIELDS: bool = false;/'

# --- v7 fuel ---
STALE5='common/farm.rs=s/^const STALE_PLAN_ROUNDS: u32 = 3;/const STALE_PLAN_ROUNDS: u32 = 5;/'
RDR5='common/farm.rs=s/^pub const RALLY_DECISION_ROUNDS: u32 = 15;/pub const RALLY_DECISION_ROUNDS: u32 = 5;/'
SSKIPON='common/farm.rs=s/^pub const STALE_SKIP_AT_GOAL: bool = false;/pub const STALE_SKIP_AT_GOAL: bool = true;/'

# --- draft ---
GCON='common/gemcarry.rs=s/^pub const GEM_CARRY: bool = false;/pub const GEM_CARRY: bool = true;/'
METAOFF='draft.rs=s/^pub const META_DRAFT: bool = true;/pub const META_DRAFT: bool = false;/'
HOVOFF='draft.rs=s/^pub const HEART_OVER_VARG: bool = true;/pub const HEART_OVER_VARG: bool = false;/'

# --- profiling (debug output in the replay's team-a/b.stdout) ---
# TRACE alone: only turns that ran out of fuel (TRACE_ONLY_SPILLS); with TRACEALL every turn.
TRACE='common/trace.rs=s/^pub const TRACE: bool = false;/pub const TRACE: bool = true;/'
TRACEALL='common/trace.rs=s/^pub const TRACE_ONLY_SPILLS: bool = true;/pub const TRACE_ONLY_SPILLS: bool = false;/'
ACT='common/activity.rs=s/^pub const ACTIVITY: bool = false;/pub const ACTIVITY: bool = true;/'

# --- v8 (off by default while tested) ---
PRUNEON='common/farm.rs=s/^pub const FARM_PRUNE: bool = false;/pub const FARM_PRUNE: bool = true;/'
CTXON='common/collect.rs=s/^pub const CTX_ALLY_CACHE: bool = false;/pub const CTX_ALLY_CACHE: bool = true;/'
CHIPON='common/collect.rs=s/^pub const CHIP_SAFE: bool = false;/pub const CHIP_SAFE: bool = true;/'
RFFON='common/rally.rs=s/^pub const RALLY_FAIL_FAST: bool = false;/pub const RALLY_FAIL_FAST: bool = true;/'
GFON='common/walk.rs=s/^pub const GROUP_FIELDS: bool = false;/pub const GROUP_FIELDS: bool = true;/'
TPGON='common/farm.rs=s/^pub const TARGET_PER_GROUP: bool = false;/pub const TARGET_PER_GROUP: bool = true;/'
RWSOFF='common/collect.rs=s/^pub const RALLY_WEAKEST_STANDINS: bool = true;/pub const RALLY_WEAKEST_STANDINS: bool = false;/'
TEAMOFF='common/collect.rs=s/^pub const TEAM_ESCAPE_RESERVE: bool = true;/pub const TEAM_ESCAPE_RESERVE: bool = false;/'
GOON='common/farm.rs=s/^pub const RALLY_GO_WHEN_WINNABLE: bool = false;/pub const RALLY_GO_WHEN_WINNABLE: bool = true;/'
# v8 fuel: fight context kept across a step, live-camp tile lookup; plans kept when a build won't fit
CTXRON='common/collect.rs=s/^pub const CTX_ROUND_REUSE: bool = false;/pub const CTX_ROUND_REUSE: bool = true;/'
LTLON='common/farm.rs=s/^pub const LIVE_TILES_LOOKUP: bool = false;/pub const LIVE_TILES_LOOKUP: bool = true;/'
FITOFF='common/farm.rs=s/^pub const FIT_STALE_ROUNDS: u32 = 6;/pub const FIT_STALE_ROUNDS: u32 = 0;/'
FIT4='common/farm.rs=s/^pub const FIT_STALE_ROUNDS: u32 = 6;/pub const FIT_STALE_ROUNDS: u32 = 4;/'
FITNE='common/farm.rs=s/^pub const FIT_WITH_ENEMIES: bool = true;/pub const FIT_WITH_ENEMIES: bool = false;/'
# Rally log back on for profiling (RALLY lines in stdout; cmp_ctrl.py counts rallies from them)
RLOGON='common/rally.rs=s/^pub const RALLY_LOG: bool = false;/pub const RALLY_LOG: bool = true;/'
# 2026-10-24: rally replies (user's design); neutral fuel savers now on
RREPOFF='common/rally.rs=s/^pub const RALLY_REPLIES: bool = true;/pub const RALLY_REPLIES: bool = false;/'
CTXOFF='common/collect.rs=s/^pub const CTX_ALLY_CACHE: bool = true;/pub const CTX_ALLY_CACHE: bool = false;/'
LTLOFF='common/farm.rs=s/^pub const LIVE_TILES_LOOKUP: bool = true;/pub const LIVE_TILES_LOOKUP: bool = false;/'
# 2026-10-24 overnight: rally refresh/commit, hand-off models, go-home upgrade rule
RREFON='common/rally.rs=s/^pub const RALLY_REFRESH: bool = false;/pub const RALLY_REFRESH: bool = true;/'
RCOMON='common/rally.rs=s/^pub const RALLY_COMMIT: bool = false;/pub const RALLY_COMMIT: bool = true;/'
HO1='common/camps.rs=s/^pub const HANDOFF_MODEL: u8 = 0;/pub const HANDOFF_MODEL: u8 = 1;/'
HO2='common/camps.rs=s/^pub const HANDOFF_MODEL: u8 = 0;/pub const HANDOFF_MODEL: u8 = 2;/'
FMOFF='common/camps.rs=s/^pub const FUNNEL_MELEE: bool = true;/pub const FUNNEL_MELEE: bool = false;/'
TFOFF='common/camps.rs=s/^pub(super) const TANK_FUNNEL: bool = true;/pub(super) const TANK_FUNNEL: bool = false;/'
HOME15='bot.rs=s/^const HOME_STAT_GAIN: f32 = 0.0;/const HOME_STAT_GAIN: f32 = 0.15;/'
HOME25='bot.rs=s/^const HOME_STAT_GAIN: f32 = 0.0;/const HOME_STAT_GAIN: f32 = 0.25;/'
HOME40='bot.rs=s/^const HOME_STAT_GAIN: f32 = 0.0;/const HOME_STAT_GAIN: f32 = 0.40;/'
# 2026-10-24: C3 (RALLY_COMMIT, HANDOFF_MODEL 1, HOME_STAT_GAIN 0.15) now on; switch back off with:
C3OFF='common/rally.rs=s/^pub const RALLY_COMMIT: bool = true;/pub const RALLY_COMMIT: bool = false;/' 
HO0='common/camps.rs=s/^pub const HANDOFF_MODEL: u8 = 1;/pub const HANDOFF_MODEL: u8 = 0;/'
HOME0='bot.rs=s/^const HOME_STAT_GAIN: f32 = 0.15;/const HOME_STAT_GAIN: f32 = 0.0;/'
CGATE='common/chase.rs=s/^pub const CHASE_GATE: bool = false;/pub const CHASE_GATE: bool = true;/'
CGBUSY='common/chase.rs=s/^pub const CHASE_GATE_BUSY: bool = false;/pub const CHASE_GATE_BUSY: bool = true;/'
CGVV='common/chase.rs=s/^pub const CHASE_GATE_VESPER: bool = false;/pub const CHASE_GATE_VESPER: bool = true;/'
CGCU='common/chase.rs=s/^pub const CHASE_GATE_CATCH_UP: bool = false;/pub const CHASE_GATE_CATCH_UP: bool = true;/'
CU15='common/chase.rs=s/^const CATCH_UP_ROUNDS: f32 = 8.0;/const CATCH_UP_ROUNDS: f32 = 15.0;/'
EB3='common/chase.rs=s/^pub const ENEMY_BASE_NO_CHASE: i32 = 0;/pub const ENEMY_BASE_NO_CHASE: i32 = 3;/'
RAVOFF='common/rally.rs=s/^pub const RALLY_AVAIL: bool = true;/pub const RALLY_AVAIL: bool = false;/'
CCG='common/chase.rs=s/^pub const CHASE_CAMP_GUARD: bool = false;/pub const CHASE_CAMP_GUARD: bool = true;/'
DAV='common/collect.rs=s/^pub const DENSE_ALIKE_VERDICTS: bool = false;/pub const DENSE_ALIKE_VERDICTS: bool = true;/'
EB0='common/chase.rs=s/^pub const ENEMY_BASE_NO_CHASE: i32 = 3;/pub const ENEMY_BASE_NO_CHASE: i32 = 0;/'
FDIAG='common/trace.rs=s/^pub(super) const FARM_TARGET_DIAGNOSTICS: bool = false;/pub(super) const FARM_TARGET_DIAGNOSTICS: bool = true;/'
RP8='common/farm.rs=s/^pub const REWARD_PRIORITY_STEPS: i32 = 0;/pub const REWARD_PRIORITY_STEPS: i32 = 8;/'
BREP='common/farm.rs=s/^pub const BLOCKED_REPLAN: bool = false;/pub const BLOCKED_REPLAN: bool = true;/'
CF0='common/fight.rs=s/^pub const CAPTURE_FIGHT_TILES: i32 = 3;/pub const CAPTURE_FIGHT_TILES: i32 = 0;/'
BWDOFF='bot.rs=s/^const BUY_WHILE_DEAD: bool = true;/const BUY_WHILE_DEAD: bool = false;/'
HLATEOFF='bot.rs=s/^const HOME_LATE_ROUND: u32 = 800;/const HOME_LATE_ROUND: u32 = 0;/'
DOHOFF='common/movement.rs=s/^pub const DASH_OVER_HOLES: bool = true;/pub const DASH_OVER_HOLES: bool = false;/'
RDA='common/rally.rs=s/^pub const RALLY_DASH_AWARE: bool = false;/pub const RALLY_DASH_AWARE: bool = true;/'
RDG='common/rally.rs=s/^pub const RALLY_DASH_GAPS: bool = false;/pub const RALLY_DASH_GAPS: bool = true;/'
HIDE='common/safety.rs=s/^pub const HIDDEN_ATTACKER: bool = false;/pub const HIDDEN_ATTACKER: bool = true;/'
RBBOFF='common/collect.rs=s/^pub const REWARD_BEFORE_BANK: bool = true;/pub const REWARD_BEFORE_BANK: bool = false;/'
GAPR='common/movement.rs=s/^pub const GAP_ROUTE: bool = false;/pub const GAP_ROUTE: bool = true;/'
