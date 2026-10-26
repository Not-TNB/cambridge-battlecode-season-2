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
CRC='common/collect.rs=s/^pub const COMMIT_RECHECK: bool = false;/pub const COMMIT_RECHECK: bool = true;/'
FRE='common/duel.rs=s/^pub const FIGHT_REMEMBERS_ENEMIES: bool = false;/pub const FIGHT_REMEMBERS_ENEMIES: bool = true;/'
GRAPHON='common/graph.rs=s/^pub const CAMP_GRAPH: bool = false;/pub const CAMP_GRAPH: bool = true;/'
# bot-v9 camp graph (common/graph.rs): both on by default in v9
G9OFF='common/graph.rs=s/^pub const CAMP_GRAPH: bool = true;/pub const CAMP_GRAPH: bool = false;/'
G9NOSTEPS='common/graph.rs=s/^pub const GRAPH_STEPS: bool = true;/pub const GRAPH_STEPS: bool = false;/'
# bot-v9 lazy fight walking grid (common/movement.rs): off = always copy the field out
LWG0='common/movement.rs=s/^pub const LAZY_WALKGRID_TILES: i32 = 5000;/pub const LAZY_WALKGRID_TILES: i32 = 0;/'
# bot-v9 Health emblems for Vesper Vale and Vendetta (champions/mod.rs)
HPB='champions/mod.rs=s/^pub const HEALTH_BUILDS: bool = false;/pub const HEALTH_BUILDS: bool = true;/'
# 2026-10-25: HEALTH_BUILDS now on in bot-v9; switch back off with:
HPBOFF='champions/mod.rs=s/^pub const HEALTH_BUILDS: bool = true;/pub const HEALTH_BUILDS: bool = false;/'
# bot-v9 capped melee funnel (common/camps.rs)
FCE2='common/camps.rs=s/^pub const FUNNEL_CAP_EMBLEMS: u32 = 0;/pub const FUNNEL_CAP_EMBLEMS: u32 = 2;/'
FCE3='common/camps.rs=s/^pub const FUNNEL_CAP_EMBLEMS: u32 = 0;/pub const FUNNEL_CAP_EMBLEMS: u32 = 3;/'
FCR300='common/camps.rs=s/^pub const FUNNEL_CAP_ROUND: u32 = 0;/pub const FUNNEL_CAP_ROUND: u32 = 300;/'
# bot-v9 plan fuel guard (common/farm.rs)
PFR25='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 0;/pub const PLAN_FUEL_RESERVE: u64 = 25_000;/'
PFR30='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 0;/pub const PLAN_FUEL_RESERVE: u64 = 30_000;/'
PFR35='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 0;/pub const PLAN_FUEL_RESERVE: u64 = 35_000;/'
PFR40='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 0;/pub const PLAN_FUEL_RESERVE: u64 = 40_000;/'
PFR50='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 0;/pub const PLAN_FUEL_RESERVE: u64 = 50_000;/'
# 2026-10-25: PLAN_FUEL_RESERVE now 50k in bot-v9; off with:
PFROFF='common/farm.rs=s/^pub const PLAN_FUEL_RESERVE: u64 = 50_000;/pub const PLAN_FUEL_RESERVE: u64 = 0;/'
# bot-v9 big-group diagnostics (common/farm.rs)
BIGDIAG='common/farm.rs=s/^pub const BIG_DIAG: bool = false;/pub const BIG_DIAG: bool = true;/'
# bot-v9 reward-rate targets and respawn waits (common/farm.rs)
RATE='common/farm.rs=s/^pub const RATE_TARGETS: bool = false;/pub const RATE_TARGETS: bool = true;/'
RSPW='common/farm.rs=s/^pub const RESPAWN_RATE: bool = false;/pub const RESPAWN_RATE: bool = true;/'
# 2026-10-25 big-group batch (bot-v9)
VP2='common/farm.rs=s/^pub const VALUE_PULL_STEPS: f32 = 0.0;/pub const VALUE_PULL_STEPS: f32 = 2.0;/'
NA4='common/farm.rs=s/^pub const NEAR_ALLY_BONUS: i32 = 0;/pub const NEAR_ALLY_BONUS: i32 = 4;/'
RETB='common/farm.rs=s/^pub const RETURN_BIG: bool = false;/pub const RETURN_BIG: bool = true;/'
HELPC='common/rally.rs=s/^pub const HELP_CALLS: bool = false;/pub const HELP_CALLS: bool = true;/'
RGOW='common/farm.rs=s/^pub const RALLY_GO_WHEN_WINNABLE: bool = false;/pub const RALLY_GO_WHEN_WINNABLE: bool = true;/'
RR26='common/rally.rs=s/^const RALLY_REPLY_MAX_STEPS: i32 = 18;/const RALLY_REPLY_MAX_STEPS: i32 = 26;/'
RR34='common/rally.rs=s/^const RALLY_REPLY_MAX_STEPS: i32 = 18;/const RALLY_REPLY_MAX_STEPS: i32 = 34;/'
RLOG='common/rally.rs=s/^pub const RALLY_LOG: bool = false;/pub const RALLY_LOG: bool = true;/'
# bot-v9 rally handshake modes (common/rally.rs)
HS1='common/rally.rs=s/^pub const RALLY_HANDSHAKE: u8 = 0;/pub const RALLY_HANDSHAKE: u8 = 1;/'
HS2='common/rally.rs=s/^pub const RALLY_HANDSHAKE: u8 = 0;/pub const RALLY_HANDSHAKE: u8 = 2;/'
HS3='common/rally.rs=s/^pub const RALLY_HANDSHAKE: u8 = 0;/pub const RALLY_HANDSHAKE: u8 = 3;/'
# 2026-10-25 late-game richer camps (bot-v9, RALLY_HANDSHAKE 3 on)
VP4='common/farm.rs=s/^pub const VALUE_PULL_STEPS: f32 = 0.0;/pub const VALUE_PULL_STEPS: f32 = 4.0;/'
VP8='common/farm.rs=s/^pub const VALUE_PULL_STEPS: f32 = 0.0;/pub const VALUE_PULL_STEPS: f32 = 8.0;/'
VPF500='common/farm.rs=s/^pub const VALUE_PULL_FROM_ROUND: u32 = 0;/pub const VALUE_PULL_FROM_ROUND: u32 = 500;/'
VPF800='common/farm.rs=s/^pub const VALUE_PULL_FROM_ROUND: u32 = 0;/pub const VALUE_PULL_FROM_ROUND: u32 = 800;/'
RTF500='common/farm.rs=s/^pub const RATE_FROM_ROUND: u32 = 0;/pub const RATE_FROM_ROUND: u32 = 500;/'
RTF800='common/farm.rs=s/^pub const RATE_FROM_ROUND: u32 = 0;/pub const RATE_FROM_ROUND: u32 = 800;/'
HS3OFF='common/rally.rs=s/^pub const RALLY_HANDSHAKE: u8 = 3;/pub const RALLY_HANDSHAKE: u8 = 0;/'
RTNOWALK='common/farm.rs=s/^pub const RATE_WALK_RERANK: bool = true;/pub const RATE_WALK_RERANK: bool = false;/'
REC30='common/farm.rs=s/^pub const RATE_RECOVER_ROUNDS: f32 = 12.0;/pub const RATE_RECOVER_ROUNDS: f32 = 30.0;/'
VPCHEAP='common/farm.rs=s/^pub const VALUE_PULL_IN_CHEAP: bool = false;/pub const VALUE_PULL_IN_CHEAP: bool = true;/'
BALT='bot.rs=s/^pub const BUILD_ALTERNATE_TAIL: bool = false;/pub const BUILD_ALTERNATE_TAIL: bool = true;/'
DET6='common/farm.rs=s/^pub const RATE_DETOUR_STEPS: i32 = 0;/pub const RATE_DETOUR_STEPS: i32 = 6;/'
DET12='common/farm.rs=s/^pub const RATE_DETOUR_STEPS: i32 = 0;/pub const RATE_DETOUR_STEPS: i32 = 12;/'
ASHARE='common/farm.rs=s/^pub const RATE_ALLY_SHARE: bool = false;/pub const RATE_ALLY_SHARE: bool = true;/'
RSKIP='common/farm.rs=s/^pub const RATE_SKIP_UNWINNABLE: bool = false;/pub const RATE_SKIP_UNWINNABLE: bool = true;/'
RPRE='common/farm.rs=s/^pub const RATE_PRECALL: bool = false;/pub const RATE_PRECALL: bool = true;/'
# bot-v9 strength turn order and judging rallies by their responders
SORDER='bot.rs=s/^pub const STRENGTH_ORDER: bool = false;/pub const STRENGTH_ORDER: bool = true;/'
RJUDGE='common/rally.rs=s/^pub const RALLY_JUDGE_RESPONDERS: bool = false;/pub const RALLY_JUDGE_RESPONDERS: bool = true;/'
# bot-v9 2026-10-26 ladder-observation fixes
SSTICK='common/farm.rs=s/^pub const RALLY_SIDE_STICKY: bool = false;/pub const RALLY_SIDE_STICKY: bool = true;/'
DEADMEM='common/collect.rs=s/^pub const DEAD_CAMP_MEMORY: bool = false;/pub const DEAD_CAMP_MEMORY: bool = true;/'
UNCOMMIT='common/rally.rs=s/^pub const RALLY_END_UNCOMMIT: bool = false;/pub const RALLY_END_UNCOMMIT: bool = true;/'
THREATMEM='common/threats.rs=s/^pub const CAMP_THREAT_MEMORY: bool = false;/pub const CAMP_THREAT_MEMORY: bool = true;/'
RYIELD='bot.rs=s/^const RECALL_YIELD: bool = false;/const RECALL_YIELD: bool = true;/'
RLOWHP='common/collect.rs=s/^pub const REWARD_BEFORE_LOW_HP: bool = false;/pub const REWARD_BEFORE_LOW_HP: bool = true;/'
RYNOENEMY='bot.rs=s/^const RECALL_YIELD_ENEMY: bool = true;/const RECALL_YIELD_ENEMY: bool = false;/'
