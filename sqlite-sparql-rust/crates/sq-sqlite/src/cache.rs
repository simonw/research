//! Connection caches (§4.3/§4.4): a CLOCK (second-chance) cache with a cost budget, used as
//! the `DecodeCache` (id -> term, budget in bytes) and the `LookupCache` (term hash ->
//! (key, id), budget in entries).
use rustc_hash::FxHashMap;

struct Entry<V> {
    key: i64,
    value: V,
    cost: usize,
    referenced: bool,
}

pub(crate) struct ClockCache<V> {
    map: FxHashMap<i64, usize>,
    slots: Vec<Option<Entry<V>>>,
    free: Vec<usize>,
    hand: usize,
    cost: usize,
    budget: usize,
    hits: u64,
    misses: u64,
}

impl<V> ClockCache<V> {
    pub fn new(budget: usize) -> Self {
        ClockCache {
            map: FxHashMap::default(),
            slots: Vec::new(),
            free: Vec::new(),
            hand: 0,
            cost: 0,
            budget,
            hits: 0,
            misses: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Total cost of the cached entries (bytes for the decode cache).
    pub fn cost(&self) -> usize {
        self.cost
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn clear(&mut self) {
        self.map.clear();
        self.slots.clear();
        self.free.clear();
        self.hand = 0;
        self.cost = 0;
    }

    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget;
        while self.cost > self.budget && !self.map.is_empty() {
            self.evict_one();
        }
    }

    /// Looks up `key`, marking the entry as recently used.
    #[inline]
    pub fn get(&mut self, key: i64) -> Option<&V> {
        match self.map.get(&key) {
            Some(&slot) => {
                self.hits += 1;
                let e = self.slots[slot].as_mut().expect("mapped slot is occupied");
                e.referenced = true;
                Some(&e.value)
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    /// Inserts or replaces `key`. Entries costing more than the whole budget are not cached.
    pub fn insert(&mut self, key: i64, value: V, cost: usize) {
        if cost > self.budget {
            return;
        }
        if let Some(&slot) = self.map.get(&key) {
            let e = self.slots[slot].as_mut().expect("mapped slot is occupied");
            self.cost = self.cost - e.cost + cost;
            e.value = value;
            e.cost = cost;
            e.referenced = true;
        } else {
            while self.cost + cost > self.budget && !self.map.is_empty() {
                self.evict_one();
            }
            let entry = Some(Entry { key, value, cost, referenced: false });
            let slot = match self.free.pop() {
                Some(s) => {
                    self.slots[s] = entry;
                    s
                }
                None => {
                    self.slots.push(entry);
                    self.slots.len() - 1
                }
            };
            self.map.insert(key, slot);
            self.cost += cost;
        }
        while self.cost > self.budget && !self.map.is_empty() {
            self.evict_one();
        }
    }

    /// Advances the hand: referenced entries get a second chance, the first unreferenced one
    /// is evicted. Terminates because every pass clears reference bits.
    fn evict_one(&mut self) {
        let n = self.slots.len();
        debug_assert!(n > 0);
        loop {
            if self.hand >= n {
                self.hand = 0;
            }
            let slot = self.hand;
            self.hand += 1;
            match &mut self.slots[slot] {
                Some(e) if e.referenced => e.referenced = false,
                Some(_) => {
                    let e = self.slots[slot].take().expect("checked above");
                    self.map.remove(&e.key);
                    self.cost -= e.cost;
                    self.free.push(slot);
                    return;
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_respects_budget_and_second_chance() {
        let mut c: ClockCache<u32> = ClockCache::new(4);
        for k in 0..4 {
            c.insert(k, k as u32, 1);
        }
        assert_eq!(c.len(), 4);
        // Touch 0 and 1: they survive the next two evictions.
        assert_eq!(c.get(0), Some(&0));
        assert_eq!(c.get(1), Some(&1));
        c.insert(10, 10, 1);
        c.insert(11, 11, 1);
        assert!(c.get(0).is_some() && c.get(1).is_some());
        assert!(c.get(2).is_none() && c.get(3).is_none());
        assert_eq!(c.cost(), 4);
        // Replacing keeps the cost exact.
        c.insert(10, 100, 2);
        assert!(c.cost() <= 4);
        assert_eq!(c.get(10), Some(&100));
        // Oversized entries are ignored.
        c.insert(99, 99, 5);
        assert!(c.get(99).is_none());
        c.set_budget(1);
        assert!(c.cost() <= 1 && c.len() <= 1);
        c.clear();
        assert_eq!(c.len(), 0);
        assert_eq!(c.cost(), 0);
    }
}
