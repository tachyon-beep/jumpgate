# Jumpgate — terminology

Canonical names for the command/organisation hierarchy. The hierarchy is naval:
**ship → fleet → taskforce**, commanded by **commodore → admiral**.

## Ship
The discrete mobile unit — "just a ship." The atomic actor the engine integrates
(position, velocity, fuel, nav state).

> **Naming note:** the codebase currently mixes two names — the storage type is
> `ShipStore` but the id type and the public `StateView`/contract accessors are
> `Craft*` (`CraftId`, `craft_pos`, `craft_ids`, `CraftInit`, …). **`ship` is
> canonical**; the `Craft*` symbols are legacy to be renamed (tracked separately;
> a behaviour-preserving, hash-neutral rename of the contract surface). Use *ship*
> in all new design/code. ("Craft" is rejected as the generic term: the hierarchy
> is naval and ship-centric; stations/probes/drones, if ever modelled, get their
> own names rather than making the unit generic.)

## Fleet
A group of ships under a **commodore** (or higher rank) that manoeuvre and operate
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
| Ship | one mobile unit | (pilot) | engine's atomic actor |
| Fleet | cohesive group of ships | Commodore | unit of shared guidance policy |
| Taskforce | all fleets in a system | Admiral | command delays apply |
