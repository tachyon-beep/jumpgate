//! Generational slot-map ids. `CraftId`/`BodyId` are `{slot, generation}` so a deleted
//! entity can't be confused with its replacement (spec §4.3). `SlotMap::cursor()`
//! is HASHED state (spec §6): it is the monotone high-water of slots ever minted,
//! constant after `reset` in v1 but present so a future `Spawn` doesn't rewrite
//! every prior tick's hash.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CraftId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StationId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProducerId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CorporationId {
    pub slot: u32,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContractId {
    pub slot: u32,
    pub generation: u32,
}

/// Generational slot-map: dense values + per-slot generation + free list + a
/// monotone `cursor` high-water. `cursor()` is included in the per-tick hash.
pub struct SlotMap<T> {
    values: Vec<Option<T>>,
    gens: Vec<u32>,
    free: Vec<u32>,
    cursor: u64,
}

impl<T> SlotMap<T> {
    pub fn new() -> Self {
        SlotMap {
            values: Vec::new(),
            gens: Vec::new(),
            free: Vec::new(),
            cursor: 0,
        }
    }
    pub fn len(&self) -> usize {
        self.values.iter().filter(|v| v.is_some()).count()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Monotone high-water of slots ever minted; HASHED state.
    pub fn cursor(&self) -> u64 {
        self.cursor
    }
    /// Returns `(slot, generation)` of the inserted value.
    pub fn insert(&mut self, value: T) -> (u32, u32) {
        if let Some(slot) = self.free.pop() {
            let i = slot as usize;
            self.values[i] = Some(value);
            (slot, self.gens[i])
        } else {
            let slot = self.values.len() as u32;
            self.values.push(Some(value));
            self.gens.push(0);
            self.cursor += 1; // only fresh slots advance the high-water
            (slot, 0)
        }
    }
    pub fn get(&self, slot: u32, generation: u32) -> Option<&T> {
        let i = slot as usize;
        if i < self.values.len() && self.gens[i] == generation {
            self.values[i].as_ref()
        } else {
            None
        }
    }
    /// Removes; bumps the slot generation; pushes the slot to the free list.
    /// Does NOT decrease `cursor`.
    pub fn remove(&mut self, slot: u32, generation: u32) -> Option<T> {
        let i = slot as usize;
        if i < self.values.len() && self.gens[i] == generation && self.values[i].is_some() {
            let taken = self.values[i].take();
            self.gens[i] = self.gens[i].wrapping_add(1);
            self.free.push(slot);
            taken
        } else {
            None
        }
    }
    pub fn gen_of(&self, slot: u32) -> Option<u32> {
        self.gens.get(slot as usize).copied()
    }

    /// Dense SoA row index for a live `(slot, generation)`. Under the v1 `slot == row`
    /// invariant this is the slot itself. Stale generation, removed slot, or out-of-range
    /// slot -> `None`.
    pub fn dense_index(&self, slot: u32, generation: u32) -> Option<usize> {
        let s = slot as usize;
        if s >= self.values.len() || self.gens[s] != generation || self.values[s].is_none() {
            return None;
        }
        Some(s)
    }

    /// The live `(slot, generation)` occupying dense row `idx`, or `None` if the row is
    /// empty/freed or out of range. Generic over `T`, so it returns the raw tuple;
    /// typed stores wrap it into `CraftId`/`BodyId`.
    pub fn id_at(&self, idx: usize) -> Option<(u32, u32)> {
        if idx >= self.values.len() || self.values[idx].is_none() {
            return None;
        }
        Some((idx as u32, self.gens[idx]))
    }

    /// Iterate every live `(slot, generation)` in ascending slot order.
    pub fn iter_ids(&self) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.values
            .iter()
            .enumerate()
            .filter_map(move |(i, v)| v.as_ref().map(|_| (i as u32, self.gens[i])))
    }
}

impl<T> Default for SlotMap<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_ordering_is_total_and_derivable() {
        let a = CraftId {
            slot: 0,
            generation: 0,
        };
        let b = CraftId {
            slot: 0,
            generation: 1,
        };
        let c = CraftId {
            slot: 1,
            generation: 0,
        };
        assert!(a < b && b < c);
        let mut v = vec![c, a, b];
        v.sort();
        assert_eq!(v, vec![a, b, c]);
    }

    #[test]
    fn new_is_empty_with_zero_cursor() {
        let m: SlotMap<u32> = SlotMap::new();
        assert_eq!(m.len(), 0);
        assert!(m.is_empty());
        assert_eq!(m.cursor(), 0);
    }

    #[test]
    fn insert_returns_fresh_slot_gen_and_advances_cursor() {
        let mut m: SlotMap<u32> = SlotMap::new();
        assert_eq!(m.insert(10), (0, 0));
        assert_eq!(m.insert(20), (1, 0));
        assert_eq!(m.len(), 2);
        assert_eq!(m.cursor(), 2);
    }

    #[test]
    fn get_returns_value_for_live_id_and_none_for_stale_gen() {
        let mut m: SlotMap<u32> = SlotMap::new();
        let (s, g) = m.insert(99);
        assert_eq!(m.get(s, g), Some(&99));
        assert_eq!(m.get(s, g + 1), None);
    }

    #[test]
    fn remove_bumps_gen_reuses_slot_but_keeps_cursor_monotone() {
        let mut m: SlotMap<u32> = SlotMap::new();
        let (s0, g0) = m.insert(1);
        assert_eq!((s0, g0), (0, 0));
        assert_eq!(m.remove(s0, g0), Some(1));
        assert_eq!(m.get(s0, g0), None); // stale
        // slot reused, generation bumped
        let (s1, g1) = m.insert(2);
        assert_eq!(s1, 0);
        assert_eq!(g1, 1);
        // cursor counts slots ever minted; it does NOT shrink on remove and
        // does NOT advance on free-list reuse.
        assert_eq!(m.cursor(), 1);
    }

    #[test]
    fn gen_of_returns_current_gen_for_valid_slot_and_none_for_oob() {
        // gen_of is called by downstream tasks (Task 4 stores / Task 13 hash)
        // per the contract-surface RULE: provider must test every downstream method.
        let mut m: SlotMap<u32> = SlotMap::new();
        let (s, _g) = m.insert(42);
        assert_eq!(m.gen_of(s), Some(0));
        // out-of-bounds slot
        assert_eq!(m.gen_of(999), None);
        // after remove, gen_of returns the bumped generation
        m.remove(s, 0);
        assert_eq!(m.gen_of(s), Some(1));
    }

    // --- Task 4 additions ---

    #[test]
    fn ids_are_copy_and_ord() {
        let a = CraftId {
            slot: 0,
            generation: 0,
        };
        let b = CraftId {
            slot: 0,
            generation: 1,
        };
        let c = CraftId {
            slot: 1,
            generation: 0,
        };
        // Copy: using `a` after passing it by value must still work.
        let _copy = a;
        assert!(a < b, "same slot, lower generation sorts first");
        assert!(
            b < c,
            "lower slot sorts before higher slot regardless of generation"
        );
        assert_eq!(a, a);

        let x = BodyId {
            slot: 2,
            generation: 5,
        };
        let y = BodyId {
            slot: 2,
            generation: 5,
        };
        assert_eq!(x, y);
    }

    #[test]
    fn insert_get_len_cursor() {
        let mut sm: SlotMap<u32> = SlotMap::new();
        assert_eq!(sm.len(), 0);
        assert_eq!(sm.cursor(), 0);

        let (s0, g0) = sm.insert(100);
        assert_eq!((s0, g0), (0, 0));
        assert_eq!(sm.len(), 1);
        assert_eq!(sm.cursor(), 1);
        assert_eq!(sm.get(s0, g0), Some(&100));

        let (s1, g1) = sm.insert(200);
        assert_eq!((s1, g1), (1, 0));
        assert_eq!(sm.len(), 2);
        assert_eq!(sm.cursor(), 2);
        assert_eq!(sm.get(s1, g1), Some(&200));

        // wrong generation reads nothing.
        assert_eq!(sm.get(s0, 99), None);
        // out-of-range slot reads nothing.
        assert_eq!(sm.get(7, 0), None);
    }

    #[test]
    fn remove_invalidates_old_id_not_replacement() {
        let mut sm: SlotMap<u32> = SlotMap::new();
        let (s0, g0) = sm.insert(100);
        assert_eq!(sm.remove(s0, g0), Some(100));
        // double-remove of the stale id is a no-op.
        assert_eq!(sm.remove(s0, g0), None);
        // old id is now stale.
        assert_eq!(sm.get(s0, g0), None);
        assert_eq!(sm.len(), 0);
        // cursor is a high-water mark: removal does NOT shrink it.
        assert_eq!(sm.cursor(), 1);

        // reinserting reuses slot 0 but with a bumped generation.
        let (s1, g1) = sm.insert(200);
        assert_eq!(s1, s0, "freed slot is reused");
        assert_eq!(g1, g0 + 1, "generation bumped on reuse");
        // the replacement is live...
        assert_eq!(sm.get(s1, g1), Some(&200));
        // ...but the old id still does NOT resolve to it.
        assert_eq!(sm.get(s0, g0), None);
        assert_eq!(sm.cursor(), 1, "reused slot does not advance cursor");
    }

    #[test]
    fn dense_index_id_at_iter_ids() {
        let mut sm: SlotMap<u32> = SlotMap::new();
        let (s0, g0) = sm.insert(10);
        let (s1, g1) = sm.insert(20);

        // dense_index: live id -> its row (slot==row in v1); stale/oob -> None.
        assert_eq!(sm.dense_index(s0, g0), Some(0));
        assert_eq!(sm.dense_index(s1, g1), Some(1));
        assert_eq!(sm.dense_index(s0, 99), None, "stale generation -> None");
        assert_eq!(sm.dense_index(7, 0), None, "out-of-range slot -> None");

        // id_at: row -> (slot,generation) of the live occupant; empty/oob row -> None.
        assert_eq!(sm.id_at(0), Some((s0, g0)));
        assert_eq!(sm.id_at(1), Some((s1, g1)));
        assert_eq!(sm.id_at(2), None, "out-of-range row -> None");

        // iter_ids: yields every live (slot,generation) in ascending slot order.
        let live: Vec<(u32, u32)> = sm.iter_ids().collect();
        assert_eq!(live, vec![(s0, g0), (s1, g1)]);

        // after a remove, the freed row is skipped by iter_ids and id_at -> None.
        assert_eq!(sm.remove(s0, g0), Some(10));
        assert_eq!(sm.id_at(0), None, "removed row -> None");
        assert_eq!(
            sm.dense_index(s0, g0),
            None,
            "stale id after remove -> None"
        );
        let live_after: Vec<(u32, u32)> = sm.iter_ids().collect();
        assert_eq!(live_after, vec![(s1, g1)]);
    }

    #[test]
    fn cursor_is_deterministic() {
        fn drive() -> (u64, usize) {
            let mut sm: SlotMap<u32> = SlotMap::new();
            let a = sm.insert(1);
            let b = sm.insert(2);
            sm.remove(a.0, a.1);
            let _c = sm.insert(3); // reuses a's slot, does not grow cursor
            sm.remove(b.0, b.1);
            let _d = sm.insert(4); // reuses b's slot
            (sm.cursor(), sm.len())
        }
        let first = drive();
        let second = drive();
        assert_eq!(first, second, "same op sequence -> same (cursor, len)");
        assert_eq!(first.0, 2, "two slots ever allocated -> cursor == 2");
        assert_eq!(first.1, 2, "two live entries");
    }

    #[test]
    fn economy_ids_are_distinct_generational() {
        let mut sm: SlotMap<()> = SlotMap::new();
        let (slot, generation) = sm.insert(());
        let s = StationId { slot, generation };
        let c = ContractId { slot, generation };
        assert_eq!(s.slot, c.slot); // same numeric slot...
        // ...but they are different *types*: this test compiles only if both exist.
        let _p = ProducerId {
            slot: 0,
            generation: 0,
        };
        let _co = CorporationId {
            slot: 1,
            generation: 0,
        };
    }
}
