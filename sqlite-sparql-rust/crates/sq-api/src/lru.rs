//! The parsed-query cache (§4.4): an LRU of [`CAPACITY`] parsed queries keyed by (text, base).
//!
//! Only parsing is cached: lowered and physical plans are never cached, because query
//! constants resolve against the dictionary as it is at execution time.
use std::sync::Arc;

use rustc_hash::FxHashMap;

/// Entries kept (§4.4).
pub(crate) const CAPACITY: usize = 128;

struct Entry {
    base: Option<Box<str>>,
    query: Arc<spargebra::Query>,
    used: u64,
}

/// Keyed by text first (lookups borrow the text: no allocation on a hit), then by base.
#[derive(Default)]
pub(crate) struct ParsedLru {
    tick: u64,
    len: usize,
    entries: FxHashMap<Box<str>, Vec<Entry>>,
    hits: u64,
    misses: u64,
}

impl ParsedLru {
    pub(crate) fn get(&mut self, text: &str, base: Option<&str>) -> Option<Arc<spargebra::Query>> {
        self.tick += 1;
        let found = self.entries.get_mut(text).and_then(|v| v.iter_mut().find(|e| e.base.as_deref() == base));
        match found {
            Some(e) => {
                e.used = self.tick;
                self.hits += 1;
                Some(e.query.clone())
            }
            None => {
                self.misses += 1;
                None
            }
        }
    }

    pub(crate) fn insert(&mut self, text: &str, base: Option<&str>, query: Arc<spargebra::Query>) {
        self.tick += 1;
        if self.len >= CAPACITY {
            self.evict_oldest();
        }
        let entry = Entry { base: base.map(Into::into), query, used: self.tick };
        match self.entries.get_mut(text) {
            Some(v) => match v.iter_mut().find(|e| e.base.as_deref() == base) {
                Some(e) => *e = entry,
                None => {
                    v.push(entry);
                    self.len += 1;
                }
            },
            None => {
                self.entries.insert(text.into(), vec![entry]);
                self.len += 1;
            }
        }
    }

    /// O(CAPACITY) removal of the least recently used entry; parsing costs far more.
    fn evict_oldest(&mut self) {
        let oldest = self
            .entries
            .iter()
            .flat_map(|(text, v)| v.iter().enumerate().map(move |(i, e)| (e.used, text, i)))
            .min_by_key(|(used, _, _)| *used)
            .map(|(_, text, i)| (text.clone(), i));
        if let Some((text, i)) = oldest
            && let Some(v) = self.entries.get_mut(&text)
        {
            v.swap_remove(i);
            self.len -= 1;
            if v.is_empty() {
                self.entries.remove(&text);
            }
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    pub(crate) fn counters(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(text: &str) -> Arc<spargebra::Query> {
        Arc::new(spargebra::SparqlParser::new().parse_query(text).unwrap())
    }

    #[test]
    fn keyed_by_text_and_base() {
        let mut lru = ParsedLru::default();
        let a = q("SELECT * { ?s ?p ?o }");
        let b = q("ASK {}");
        lru.insert("A", None, a.clone());
        lru.insert("A", Some("http://ex/"), b.clone());
        assert!(Arc::ptr_eq(&lru.get("A", None).unwrap(), &a));
        assert!(Arc::ptr_eq(&lru.get("A", Some("http://ex/")).unwrap(), &b));
        assert!(lru.get("A", Some("http://other/")).is_none());
        assert!(lru.get("B", None).is_none());
        assert_eq!(lru.counters(), (2, 2));
        assert_eq!(lru.len(), 2);
        lru.insert("A", None, b.clone());
        assert_eq!(lru.len(), 2, "re-inserting replaces");
    }

    #[test]
    fn evicts_the_least_recently_used_entry() {
        let mut lru = ParsedLru::default();
        let query = q("ASK {}");
        for i in 0..CAPACITY {
            lru.insert(&format!("q{i}"), None, query.clone());
        }
        assert!(lru.get("q0", None).is_some(), "q0 becomes the most recently used");
        lru.insert("new", None, query.clone());
        assert_eq!(lru.len(), CAPACITY);
        assert!(lru.get("q0", None).is_some());
        assert!(lru.get("q1", None).is_none(), "q1 was the least recently used");
        assert!(lru.get("new", None).is_some());
    }
}
