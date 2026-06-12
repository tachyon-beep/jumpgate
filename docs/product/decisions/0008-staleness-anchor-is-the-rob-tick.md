# PDR-0008 — Gossip staleness anchors to the rob tick (the information horizon)

**Date:** 2026-06-12
**Status:** ACCEPTED — owner-directed
**Decider:** John (owner)
**Context:** The W4 anchor adoption call (world-gets-big console packet, agenda
item 3; spec §11 W4 ran both anchor arms as pre-registered counterfactuals).

---

## Decision

Gossip arms adopt the **rob anchor** (`staleness_from_rob_tick = true`) as the
canonical media configuration for future ensembles, chronicles, and any
scenario that ships with media on.

The `MediaCfg` **struct default stays `false`** — the default is hashed, every
shipped scenario is media-off at reset, and arms pass their knobs explicitly,
so flipping the literal would re-pin `GOLDEN_CONFIG_HASH` for zero behavioral
gain. Adoption is a recording decision about which arm is canonical, not a
config-literal change.

Future W4-style grids **keep both anchor arms**: born remains the registered
counterfactual, exactly as blind/ring remain counterfactuals today.

## Rationale

Realism argues both ways (a report is dated information from the moment it is
filed; equally, a rumor's credibility decays from the event, not the filing).
The owner's tiebreak rule: **take the anchor that is more novel for gameplay.**

That is the rob anchor, on the 2026-06-12 re-run evidence:

- **It creates an information horizon.** Measured W2 frontier hearing lag
  (median quartiles 5694/6963/9228 ticks) exceeds the 4000-tick evidence
  window, so rob-anchored evidence on long routes is dead on arrival — the
  registered "gossip degenerates toward blind on frontier routes" reading
  becomes a *feature*: the core is an informed society, the rim is
  structurally blind no matter how good the network is. Distance in space =
  distance in time = distance in trust.
- **The value cost is small.** Gossip still decisively beats ring under the
  rob anchor: +622,275 micros median final hauler credits (vs +713,500
  born-anchored; both arms' A/A twins clean, 18/20 clean seeds).
- **It creates the selection gradient for emergent curation.** Under the born
  anchor freshness is irrelevant, so there is no pressure on what carriers
  choose to carry. Under the rob anchor timeliness *is* value.

## Registered watch — gossip self-selection (NEVER shaped in)

Owner hypothesis (2026-06-12): carriers may come to self-select what gossip
they spread to maximise effectiveness. Per the no-shaping/add-capacity
principle this is a watch-for-it, not a build:

- **Falsifiable signature:** the carried-alert age distribution skews fresher
  than the available-alert age distribution by more than slot-eviction
  mechanics alone produce. Today's carry/evict path is mechanical, so any
  excess freshness skew = curation emerged.
- If the signature fires, the question routes through the principle: world
  mechanic (price it), new head-module (make it learnable), or nothing
  (keep watching). It is never a reward-shaping patch.

## Reversal trigger

If a future ensemble shows the gossip-vs-ring value delta collapsing to ≤ 0
under the rob anchor while the born arm still pays (i.e. the horizon eats the
entire value of the channel on bigger maps), re-judge at the console with both
arms in hand.
