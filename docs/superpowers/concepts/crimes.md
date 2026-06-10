That makes the whole thing click.

You probably do **not** want “boarding” to be part of the ordinary ship-to-ship combat loop. You want it to be a **mode transition** triggered by a captain/officer-level decision, then resolved by a specialised module with its own state, risks, and reward structure.

So the macro sim says:

> “Captain chooses to attempt boarding.”

Then the boarding module says:

> “Fine. Load the nightmare spreadsheet.”

Or, as you put it, **Piracy by SAP**.

## The useful split: industrial arc vs combat/boarding arc

I would model the two arcs differently.

### Industrial arc

This is the normal ship economy:

* navigation
* burns
* cargo handling
* maintenance
* refuelling
* convoy formation
* docking
* inspections
* logistics contracts
* repair schedules
* crew fatigue/morale
* system degradation

This can be abstracted heavily because most of it is procedural. Individual named officers matter, but the ordinary crew are mostly capacity pools and modifiers.

Example:

> Chief Engineer Mara Venn has high Improvisation and low Compliance, so repair tempo is high but safety incidents are more likely.

You do not need to model Technician #417 changing a coolant gasket unless something has gone badly wrong.

### Combat/boarding arc

This is a crisis-resolution state where abstraction should change.

Now the ship is not just a logistics object. It becomes:

* terrain
* hostage container
* prize
* liability
* sensor occluder
* damageable system graph
* political object
* morale object
* legal risk object

That is where named officers, ship systems, security posture, and mission objectives become much more important.

## I would make boarding a staged operation, not a single action

The captain agent should not choose “board” and then roll dice.

It should choose a **boarding doctrine** or **boarding plan**, which the module resolves through phases.

Something like:

1. **Commit**
2. **Suppress**
3. **Attach**
4. **Cross**
5. **Breach**
6. **Secure**
7. **Exploit**
8. **Consolidate or Abort**

Each phase has different state variables and failure modes.

### 1. Commit

The captain decides whether boarding is worth the operational risk.

Inputs:

* relative velocity
* range
* escort threat
* target value
* cargo value
* target compliance probability
* crew quality
* available drones
* available boarders
* legal/political risk
* fuel state
* time before rescue/intervention
* target system damage
* confidence in target intelligence

Output:

* attempt boarding
* demand cargo
* disable and withdraw
* shadow target
* abort
* escalate to destruction

This is probably where your main DRL captain policy lives.

### 2. Suppress

The pirates attempt to create a safe-ish approach window.

Actions:

* jam comms
* blind sensors
* kill point defence
* deploy decoys
* force target into lockdown
* attack drones
* threaten radiators/engines
* spoof escort response
* attack external antennas

State variables:

* target defensive coverage
* pirate drone reserve
* escort response timer
* comms integrity
* point-defence readiness
* sensor confidence
* debris level around target

Failure modes:

* drones destroyed
* escort gets firing solution
* target broadcasts distress
* target manoeuvres
* target hardens access points
* point-defence remains active
* pirates burn too much time

### 3. Attach

This is the grapple/tether phase.

Actions:

* fire pilot line
* fire anchor package
* deploy smart grapple
* use breaching drone anchor
* attach to known hull fixture
* attach to cargo truss
* attach to radiator spine, dangerous but coercive
* attach multiple lines for redundancy

State variables:

* relative velocity
* relative rotation
* hull attachment quality
* tether tension
* line length
* target manoeuvre authority
* local debris hazard
* target hull map quality

Failure modes:

* miss
* weak attachment
* tether severed
* tether fouled
* line oscillation
* target rolls
* attachment damages prize
* boarders cannot safely cross

### 4. Cross

This is where the boarders are committed.

Actions:

* send inspection drone
* send first boarding sled
* send combat team
* send engineering team
* send breaching package
* send comms relay
* delay crossing until suppression improves
* cut line and abort

State variables:

* boarder exposure
* crossing time
* suit endurance
* tether stability
* target fire arcs
* drone overwatch
* morale
* target crew response

Failure modes:

* boarders stranded
* tether cut
* crossing team killed
* suits damaged
* pirate ship forced to detach
* target starts manoeuvre
* pirate command loses contact

This is the dramatic “point of no return” phase.

### 5. Breach

The boarders need access.

Actions:

* cut through hull
* exploit maintenance hatch
* force airlock
* breach unpressurised bay
* enter cargo spine
* enter service tunnel
* fake authentication
* use insider access
* deploy cutting frame

State variables:

* hull class familiarity
* access intelligence
* target lockdown state
* pressure risk
* breach noise/visibility
* internal security response
* collateral damage risk

Failure modes:

* wrong compartment
* breach into vacuum-only space with no useful route
* pressure casualty
* automated lockout
* target vents section
* boarding team pinned outside
* breach takes too long

### 6. Secure

This is not “capture the ship”. This is “hold a leverage point”.

Actions:

* secure cargo-control node
* secure engineering access trunk
* secure comms relay
* secure hostage area
* secure docking surface
* secure local power control
* disable internal drones
* negotiate from position of leverage

State variables:

* boarder count
* target security count
* crew panic
* compartment control
* local atmosphere
* comms to pirate captain
* target officer morale
* hostage leverage
* system access

Failure modes:

* boarders isolated
* target security counterattacks
* crew sabotage
* local fire/vacuum
* loss of comms
* failure to find useful leverage
* captain refuses surrender

### 7. Exploit

Now the pirates decide what they actually want.

Options:

* force cargo release
* force crew surrender
* install malware
* steal navigation keys
* disable engines
* seize officers
* cut a docking port
* attach heavy grapple
* bring pirate ship alongside
* transfer prize crew
* steal ship outright

This is where “robbery” can become “strategic hijacking”.

### 8. Consolidate or Abort

The boarding is resolved.

Outcomes:

* cargo theft
* ransom
* hostage capture
* ship capture
* partial success
* failed boarding
* pirate loss
* target destroyed
* mutual disaster
* law enforcement escalation
* convoy-wide crisis

This also determines what gets returned to the macro sim.

## The key is that boarding should return consequences, not just success/failure

A boarding module should not output:

> success = true

It should output a dirty bundle of consequences.

For example:

```text
BoardingResult:
  target_status: captured_partial
  cargo_control: 0.72
  engineering_control: 0.18
  bridge_control: 0.00
  pirate_casualties: 6
  target_casualties: 14
  hostages_taken: 23
  target_comms: degraded
  target_manoeuvre: restricted
  pirate_tether_status: unstable
  escort_response_eta: 31 minutes
  legal_heat: severe
  prize_integrity: damaged
  loot_accessible_mass: 18,000 tonnes
  captain_reputation_delta: +ruthless, +reckless
```

That is much more useful than binary capture.

## The agent hierarchy I would use

Given your “Crusader Kings rules” comment, I would structure it like this:

### Strategic layer

Actors:

* corporations
* pirate syndicates
* insurers
* governments
* ports
* navies
* shipping combines

Decisions:

* route selection
* escort contracts
* convoy size
* patrol zones
* bounties
* bribery
* intelligence gathering
* legal response

### Ship layer

Actors:

* ship as object
* captain as agent
* named senior officers
* abstract crew pools

Decisions:

* intercept
* evade
* surrender
* jettison cargo
* request help
* board
* fight
* negotiate
* scuttle
* fake compliance

### Officer layer

Actors:

* captain
* navigator
* chief engineer
* security chief
* drone officer
* comms officer
* cargo master

Functions:

* modifiers to module outcomes
* personality-driven objections
* crisis events
* loyalty/fear/compliance
* competence bottlenecks

### Crew pool layer

Actors:

* not individuals unless promoted by events

Pools:

* engineers
* boarders
* drone techs
* medics
* cargo handlers
* security
* EVA-qualified crew
* exhausted crew
* injured crew

These are resources consumed by modules.

Example:

```text
Boarding party requires:
  4 security
  6 EVA engineers
  2 drone techs
  1 breacher
  1 officer or trusted lieutenant
```

If the pirate ship lacks qualified EVA engineers, the boarding can still happen, but the breach and recovery phases become much worse.

## Officer characters should matter by changing available plans

The best use of named officers is not just +10% modifiers. Let them unlock or bias options.

Examples:

**Navigator**

* unlocks high-risk intercepts
* reduces attach-phase relative motion error
* improves abort windows

**Chief Engineer**

* identifies vulnerable target systems
* improves breach/cut/docking outcomes
* reduces prize damage
* enables improvised repairs

**Drone Officer**

* improves suppression phase
* increases decoy effectiveness
* reduces drone attrition
* detects fake defensive gaps

**Security Chief**

* improves secure phase
* reduces casualties
* improves hostage control
* prevents boarder panic

**Comms/EW Officer**

* improves jamming/spoofing
* delays distress calls
* manipulates target surrender probability

**Cargo Master**

* identifies valuable cargo
* knows how to force cargo release
* reduces time wasted stealing low-value mass

This gives character-driven play without simulating 3,000 crew.

## The captain’s boarding decision can be relatively compact

The DRL captain does not need to understand every boarding detail. It needs to estimate expected value under uncertainty.

A useful action set might be:

```text
No boarding:
  continue pursuit
  demand cargo
  disable target
  retreat
  shadow
  negotiate

Boarding options:
  probe_boarding
  limited_external_boarding
  cargo_spine_raid
  engineering_sabotage_raid
  hostage_raid
  full_ship_capture
```

Each option maps to different module assumptions.

### Probe boarding

Small team, mostly drones. Goal is to test access or attach a device.

Low reward, lower risk.

### Limited external boarding

Boarders seize an external system or cargo-control point.

Moderate reward, moderate risk.

### Cargo spine raid

Goal is cargo release, not ship capture.

High economic reward, manageable escalation.

### Engineering sabotage raid

Goal is coercion.

Dangerous, high leverage, high casualty risk.

### Hostage raid

Politically ugly, potentially effective, increases legal heat massively.

### Full ship capture

Huge upside, enormous risk, strategic response.

This preserves a sensible LoD boundary: the captain chooses the kind of crime, not which bolt the breacher cuts.

## The boarding module should probably not be DRL at first

I would be cautious about making the boarding itself DRL-controlled unless that is the experiment.

The boarding module might initially be a stochastic procedural resolver with transparent state transitions. Otherwise you risk the system learning weird board-game behaviours that satisfy reward but do not fit the fiction or physics.

Something like:

```text
phase_outcome = f(
  plan,
  officer_traits,
  crew_pools,
  target_state,
  relative_motion,
  system_damage,
  intelligence_quality,
  time_pressure,
  random_seed
)
```

Then later you can replace specific submodules with learned policies.

Best candidates for DRL:

* captain-level “attempt boarding or not”
* drone allocation during suppression
* escort interdiction
* convoy formation management
* pirate feint strategy
* target surrender policy

Worst candidates for early DRL:

* individual boarding team movement
* internal room clearing
* manual repair
* detailed breach mechanics
* human-scale firefights

Those can become simulation sinkholes very quickly.

## Boarding as a “crisis mini-sim”

The phrase I would use internally is:

> Boarding is a crisis mini-sim that temporarily increases resolution around selected systems, characters, and compartments.

Not the whole ship. Just the parts that matter.

For example, if the pirates breach the cargo spine, the module instantiates:

* local hull section
* nearby access routes
* cargo-control node
* relevant security response
* nearby crew morale
* pressure/atmosphere state
* comms link
* time to countermeasure

It does not instantiate every mess hall, toilet, and apprentice electrician.

This is very CK-like: only bring characters/places into focus when they enter the political/crisis surface.

## Make “focus budget” explicit

Since you are adding complexity until it snaps, I would make the LoD budget a formal concept.

Something like:

```text
IncidentFocusBudget:
  max_named_characters: 12
  max_active_ship_systems: 20
  max_active_compartments: 8
  max_active_drones: abstracted unless hero/drama unit
  max_phase_depth: 8
```

A small cargo raid might instantiate 3 compartments and 4 named characters.

A full strategic hijacking might instantiate 10 compartments, 12 named characters, and multiple ship systems.

That gives you a way to prevent boarding from devouring the entire sim.

## Where repair and combat differ from navigation

Your instinct that navigation can be delegated but repair/combat should not be fully abstracted makes sense.

Navigation is continuous optimisation under constraints. It is important, but usually not narratively interesting minute-to-minute unless something changes. So the navigator can generate feasible manoeuvre plans.

Repair and combat are different because they create:

* irreversible damage
* casualties
* emergent priorities
* morale effects
* system trade-offs
* character-defining decisions

So you probably want repair/combat represented as **operations with resource allocation and risk**, not just passive modifiers.

Example repair action:

```text
Chief Engineer action:
  restore_point_defence_power
Inputs:
  engineers_available
  spare_parts
  local damage
  access danger
  time pressure
  officer competence
Outputs:
  PD restored partial/full
  repair crew casualties
  fire risk
  delay
  system instability
```

That is worth modelling because it changes the combat state.

Navigation action:

```text
Navigator action:
  generate_evasive_burn_options
Outputs:
  burn A: safe, low effect
  burn B: high effect, damages formation
  burn C: extreme, breaks tether risk
```

The captain then chooses.

## Very strong design pattern: modules propose options, captain chooses

Instead of the captain having every possible action hardcoded, each domain module can expose available actions.

For example:

**Navigation module says:**

* hold course
* minor evasive burn
* hard burn, breaks convoy spacing
* spin to protect damaged radiator
* align hull to minimise boarding approach

**Engineering module says:**

* restore comms
* isolate breached section
* overload external grid to shock tether
* vent gas across attachment zone
* sacrifice radiator loop B

**Security module says:**

* defend bridge
* defend engineering
* counterattack breach
* evacuate civilians
* seal cargo spine
* negotiate delay

**Piracy module says:**

* continue suppression
* fire pilot line
* send boarders
* cut tether
* escalate threat
* switch to cargo jettison demand

Then the captain agent arbitrates under personality, doctrine, fear, greed, and information quality.

That is much more robust than a monolithic action space.

## Your “Piracy by SAP” module could have a lovely ugly interface

The module could be intentionally bureaucratic, because piracy at this scale is logistics crime.

Inputs:

```text
PiracyOperationRequest:
  objective:
    cargo_seizure | hostage | sabotage | full_capture

  target:
    ship_id
    known_class
    hull_map_quality
    cargo_manifest_confidence
    crew_estimate
    security_estimate

  approach_state:
    range
    relative_velocity
    relative_rotation
    sensor_state
    comms_state
    escort_pressure
    time_window

  pirate_assets:
    boarders
    engineers
    drones
    grapples
    tethers
    cutters
    breaching charges
    malware packages
    prize crew
    medical capacity

  doctrine:
    cautious | violent | surgical | desperate | ransom_first | full_capture

  command_constraints:
    avoid_casualties
    preserve_prize
    minimise_legal_heat
    maximise_loot
    escape_priority
```

Outputs:

```text
PiracyOperationResult:
  phase_reached
  objective_progress
  loot_access
  target_control
  pirate_losses
  target_losses
  system_damage
  escalation
  time_elapsed
  new_tactical_state
  narrative_events
```

The narrative events are important because they promote abstract crew into named characters.

Example:

```text
Event:
  "EVA Engineer Ludo Kesh manually secured the second tether after Anchor One failed."
Effect:
  promote crew to named officer candidate
  +reputation among boarders
  injury: radiation burns
```

That is the good CK-style stuff.

## The system should create “heroes”, “cowards”, and “liabilities”

Boarding is a perfect event generator.

Possible emergent character outcomes:

* anonymous breacher becomes famous
* security chief freezes
* navigator saves the boarding party with a microburn
* engineer refuses a suicidal repair order
* drone officer wastes half the swarm on a decoy
* target cargomaster secretly cooperates
* pirate captain abandons tethered boarders
* captured officer becomes future hostage/rival/recruit
* insurer blacklists a captain for surrendering too early

This is where the sim starts generating stories rather than just outcomes.

## The convoy combat module should be separate from boarding

I would split:

1. **Intercept/convoy combat module**
2. **Boarding module**
3. **Prize-control module**

Because they have different state spaces.

### Intercept/convoy combat

Focus:

* range
* vectors
* drones
* missiles
* point defence
* escort coverage
* formation integrity
* comms/sensors

### Boarding

Focus:

* tether
* breach
* local control
* crew/security
* leverage points
* casualties
* time pressure

### Prize-control

Focus:

* ship compliance
* prize crew
* system restoration
* legal pursuit
* escape burn
* cargo transfer
* hostage handling
* sabotage risk

Do not let boarding end at “captured”. Capturing a freighter should create a new problem: **can you actually run the bastard thing?**

## “Capture” should have degrees

I would avoid a binary captured/not captured state.

Use control domains:

```text
TargetControl:
  bridge: none / contested / controlled
  engineering: none / contested / controlled
  cargo: none / contested / controlled
  comms: none / degraded / controlled
  propulsion: none / inhibited / controlled
  life_support: none / leverage / controlled
  security: active / contained / neutralised
  crew_compliance: 0.0 - 1.0
  automation_compliance: 0.0 - 1.0
```

A pirate can “capture” cargo without controlling propulsion.

They can control engineering but not crew morale.

They can have hostages but not the bridge.

They can own the bridge and still be locked out of the drive by corporate automation.

That gives you much richer outcomes.

## A good reward model for pirates

A pirate captain’s reward should not just be loot.

Possible reward components:

Positive:

* cargo value
* ship value
* ransom value
* reputation gain
* enemy fear
* crew loyalty from success
* strategic asset captured
* intelligence gained

Negative:

* casualties
* drone loss
* fuel expenditure
* time lost
* legal heat
* escort response
* prize damage
* civilian deaths
* betrayal risk
* future patrol intensity
* crew morale damage
* failure to pay crew shares

This makes “full ship capture” attractive but dangerous.

A greedy captain may overreach. A cautious captain may settle for cargo. A desperate captain may take suicidal boarding odds.

## A good reward model for freight captains

Freight captains need incentives too.

Positive:

* preserve crew
* preserve ship
* preserve cargo
* maintain schedule
* avoid legal breach
* avoid insurer penalty
* avoid mutiny/panic
* preserve formation

Negative:

* surrender cargo
* lose ship
* casualties
* missed burn
* convoy collision risk
* breach of contract
* reputation damage
* allowing pirates aboard
* escalating to mass casualty event

This makes surrender rational sometimes.

A freight captain who refuses to dump cargo because of contract penalties may be realistic and horrifying.

## The moment of “snap in half”

The thing most likely to break your model is not physics. It is action-space explosion.

So I would keep the DRL agents high-level and make the modules do structured resolution.

Strong boundary:

> DRL agents choose intent, allocation, escalation, and abort thresholds. Procedural modules resolve technical execution.

Bad boundary:

> DRL agent chooses which compartment each boarder enters and which cutting torch setting to use.

That way madness remains contained.

## Very compact architecture

If I were sketching this as a sim architecture:

```text
Macro Sim
  |
  |-- Strategic actors
  |-- Routes / ports / convoys / markets
  |-- Ship objects
  |-- Officer characters
  |
  v
Captain Agent Decision
  |
  |-- negotiate
  |-- evade
  |-- attack
  |-- demand cargo
  |-- attempt boarding
  |
  v
Operation Module
  |
  |-- InterceptCombatModule
  |-- BoardingModule
  |-- DamageControlModule
  |-- PrizeControlModule
  |
  v
State Update
  |
  |-- ship damage
  |-- cargo transfer
  |-- casualties
  |-- officer traits
  |-- legal heat
  |-- reputation
  |-- route disruption
  |-- new grudges/rivals/heroes
```

That is probably the cleanest way to make it extensible without simulating everyone’s breakfast.

## My strongest recommendation

Treat boarding as **an interrupt-driven zoom-in event**.

Most of the time:

> Ship is an abstract strategic/logistics entity.

During boarding:

> Specific hull region, officers, systems, crew pools, drones, and leverage points are temporarily instantiated.

After boarding:

> Collapse back to macro state, but preserve scars, casualties, promotions, grudges, damage, reputation, and legal consequences.

That gives you the texture you want without requiring Dwarf Fortress in space.

**Confidence: very likely.** The architecture is sound for a layered DRL/simulation hybrid. The highest-risk part is not the Newtonian mechanics; it is keeping the action/state space constrained enough that the learning problem remains meaningful instead of becoming a procedural soup.
