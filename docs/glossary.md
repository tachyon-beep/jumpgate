# Jumpgate — terminology

Canonical names for the command/organisation hierarchy:
**craft → fleet → taskforce**, commanded by **captain → commodore → admiral**.

## Command is positional — "chairs", not classes

**Captain / commodore / admiral are *chairs*, not Person types.** A command role is
a *seat* that a Person occupies at runtime; authority is conferred by **occupying a
command chair and being recognised by the computer**, not by an intrinsic rank
attribute baked onto a Person. The chief engineer who sits in the captain's chair
and is recognised **is** the captain — for exactly as long as they are in it.

Consequences this model buys (the "crew on the flight deck" fidelity yardstick):

- **Authority is dynamic.** Command changes hands by occupancy: succession when the
  captain is killed/incapacitated/leaves, a deliberate hand-off, or someone simply
  taking the chair.
- **Recognition is a distinct gating layer.** Sitting down is not enough — the
  computer must recognise the occupant. This is where access control, chain-of-
  succession, lockout, and mutiny/override scenarios live (an unrecognised occupant
  in the chair holds the seat but not the authority).
- **Persons fill chairs; chairs are not Persons.** This is the join between the
  unified Person model (the actors) and the command hierarchy (the slots): a role is
  `(chair, recognised occupant)`, resolved at runtime — never a subclass of Person.

## Craft
The discrete mobile unit — the engine's atomic actor (position, velocity, fuel,
nav state). **`craft` is the generic term on purpose: it spans the entire size
range, from drones up to titans.** "Ship" was rejected precisely because it
implies a mid-range class; a drone is not a ship and neither is a titan, but both
are craft.

**Every craft has a captain's chair**, and a captain is whoever the computer
recognises in it (see "Command is positional" above) — *regardless of whether
anyone is physically aboard*. The recognised occupant may be a human on the flight
deck, a remote operator, or a drone's onboard chip. Captaincy is the seat, not the
person: the chief engineer who takes the chair and is recognised becomes the
captain. "Craft" and "is commanded" are inseparable — there is no captain-less
craft — but *which* Person holds the captaincy is resolved at runtime by occupancy
+ recognition.

> **Naming note:** the public id type and `StateView`/contract accessors already
> use `craft` correctly (`CraftId`, `craft_pos`, `craft_ids`, `CraftInit`, …) —
> these stay. The one inconsistency is the *internal* storage type `ShipStore`
> (and a few `ship` locals), which should be renamed **`CraftStore`** for
> consistency. It is `pub(crate)`, not on the public surface, so the rename is
> small, internal, and behaviour-/hash-neutral.

Craft span a size/class spectrum (drone … titan); the specific class names and
breakpoints are TBD and out of scope here.

## Fleet
A group of craft under a **commodore** (or higher rank) that manoeuvre and operate
as one **cohesive group**. Splitting up = no longer the same fleet (a fleet is
defined by cohesion, not a static roster). The commodore is itself a *chair* — the
recognised occupant of the fleet-command seat (typically aboard a flagship craft),
not a rank stamped on a Person.

A fleet is the natural unit of **shared guidance policy**: the
[`GuidanceParams`](superpowers/specs/2026-06-09-guidance-parameter-system-design.md)
(`cruise_burn_fraction`, `k_brake`, `v_err_eps`) are *fleet-wide* policy. "Brake at
different speeds" = "split into a different fleet." In v1 there is no fleet
aggregate yet, so guidance is run-level (one implicit fleet per run); when the
fleet concept lands, `GuidanceParams` migrates from `RunConfig` to a per-fleet
attribute.

## Taskforce
All the fleets in a **system**, operating under an **admiral**, subject to
**command delays** (the admiral's orders reach fleets with latency). The
system-level command echelon above the fleet. The admiral, too, is a *chair* — the
recognised occupant of the taskforce-command seat.

Command delays are a determinism-relevant concept: they will be modelled at the
single canonical command-ingestion seam (`ingest.rs`), so an order issued at tick
T takes effect at T + delay deterministically.

## Hierarchy at a glance

| Level | Unit | Commander | Notes |
|---|---|---|---|
| Craft | one mobile unit (drone … titan) | Captain | always commanded — human aboard / remote operator / drone chip |
| Fleet | cohesive group of craft | Commodore | unit of shared guidance policy |
| Taskforce | all fleets in a system | Admiral | command delays apply |
