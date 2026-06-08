# Jumpgate — terminology

Canonical names for the command/organisation hierarchy:
**craft → fleet → taskforce**, commanded by **captain → commodore → admiral**.

## Craft
The discrete mobile unit — the engine's atomic actor (position, velocity, fuel,
nav state). **`craft` is the generic term on purpose: it spans the entire size
range, from drones up to titans.** "Ship" was rejected precisely because it
implies a mid-range class; a drone is not a ship and neither is a titan, but both
are craft.

**Every craft has a captain** — the per-craft command authority — *regardless of
whether anyone is aboard*. The captain may be a human on the flight deck, a remote
operator, or a drone's onboard chip. The captain is uniform across the size range
(it is the decision-maker for that craft); it links to the unified Person model
(see the Person+Ship design). This means "craft" and "is commanded" are
inseparable: there is no captain-less craft.

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
defined by cohesion, not a static roster).

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
system-level command echelon above the fleet.

Command delays are a determinism-relevant concept: they will be modelled at the
single canonical command-ingestion seam (`ingest.rs`), so an order issued at tick
T takes effect at T + delay deterministically.

## Hierarchy at a glance

| Level | Unit | Commander | Notes |
|---|---|---|---|
| Craft | one mobile unit (drone … titan) | Captain | always commanded — human aboard / remote operator / drone chip |
| Fleet | cohesive group of craft | Commodore | unit of shared guidance policy |
| Taskforce | all fleets in a system | Admiral | command delays apply |
