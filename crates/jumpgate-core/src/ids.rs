//! Generational slot-map ids. `CraftId`/`BodyId` are `{slot, gen}` so a deleted
//! entity can't be confused with its replacement (spec §4.3). `SlotMap::cursor()`
//! is HASHED state (spec §6): it is the monotone high-water of slots ever minted,
//! constant after `reset` in v1 but present so a future `Spawn` doesn't rewrite
//! every prior tick's hash.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CraftId {
    pub slot: u32,
    pub gen: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BodyId {
    pub slot: u32,
    pub gen: u32,
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
    /// Returns `(slot, gen)` of the inserted value.
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
    pub fn get(&self, slot: u32, gen: u32) -> Option<&T> {
        let i = slot as usize;
        if i < self.values.len() && self.gens[i] == gen {
            self.values[i].as_ref()
        } else {
            None
        }
    }
    /// Removes; bumps the slot generation; pushes the slot to the free list.
    /// Does NOT decrease `cursor`.
    pub fn remove(&mut self, slot: u32, gen: u32) -> Option<T> {
        let i = slot as usize;
        if i < self.values.len() && self.gens[i] == gen && self.values[i].is_some() {
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
        let a = CraftId { slot: 0, gen: 0 };
        let b = CraftId { slot: 0, gen: 1 };
        let c = CraftId { slot: 1, gen: 0 };
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
}
