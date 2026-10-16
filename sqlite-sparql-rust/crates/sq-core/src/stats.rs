//! Planner statistics (hints only) and cost constants.
use crate::id::TermId;
use rustc_hash::FxHashMap;

#[derive(Clone, Debug, Default)]
pub struct PredStats {
    /// Triples with this predicate.
    pub n: u64,
    /// Distinct subjects / objects.
    pub ds: u64,
    pub dobj: u64,
    /// Min / max inline xsd:integer object (selectivity of int ranges only).
    pub imin: Option<i64>,
    pub imax: Option<i64>,
}

/// One characteristic set: the subjects whose predicate set is exactly `preds`.
#[derive(Clone, Debug, Default)]
pub struct CharSet {
    pub subjects: u64,
    /// (predicate, number of triples with that predicate over these subjects), sorted by id.
    pub preds: Vec<(TermId, u64)>,
}

#[derive(Clone, Debug, Default)]
pub struct ScopeStats {
    pub n: u64,
    pub ds: u64,
    pub dp: u64,
    pub dobj: u64,
    pub pred: FxHashMap<TermId, PredStats>,
    /// Heavy hitters: (p, o) pairs with n >= max(100, n(p)/1000); covers rdf:type classes.
    pub po: FxHashMap<(TermId, TermId), u64>,
    /// Top characteristic sets by #subjects (at most 10_000).
    pub cs: Vec<CharSet>,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// [0] = default graph (g = 0), [1] = union of all named graphs.
    pub scope: [ScopeStats; 2],
    /// Total quads when analyzed (planner scales nothing by it; diagnostic only).
    pub analyzed_quads: u64,
}

/// Cost model constants in nanoseconds, selected by the store's reader kind.
/// Starting values from the design measurements, adjusted by the benchmark-driven performance passes.
#[derive(Clone, Debug)]
pub struct CostConstants {
    /// Sequential scan cost per row (step + column_int64 per free column).
    pub scan_row: f64,
    /// Extra per selected column.
    pub scan_col: f64,
    /// Open/reposition a range scan at a random key (inside a held read txn).
    pub seek_random: f64,
    /// Reposition at a key > previous key (sorted probes, galloping).
    pub seek_sorted: f64,
    /// Hash build per row while the table fits in L2 (< 32K entries) / beyond.
    pub hash_build_small: f64,
    pub hash_build_large: f64,
    pub hash_probe_small: f64,
    pub hash_probe_large: f64,
    /// Sort per row per log2(n).
    pub sort_row_log: f64,
    pub row_copy_per_slot: f64,
    /// Dictionary decode: miss / cache hit.
    pub decode_miss: f64,
    pub decode_hit: f64,
    /// SQLite VDBE nested-loop probe per outer row (SqlJoin hybrid) and per output column.
    pub vdbe_probe: f64,
    pub vdbe_row: f64,
    /// count(*) evaluated inside the VDBE, per counted row.
    pub vdbe_count_row: f64,
}

pub const SQL_CURSOR_COSTS: CostConstants = CostConstants {
    scan_row: 80.0,
    scan_col: 15.0,
    seek_random: 1100.0,
    seek_sorted: 700.0,
    hash_build_small: 60.0,
    hash_build_large: 250.0,
    hash_probe_small: 25.0,
    hash_probe_large: 120.0,
    sort_row_log: 3.0,
    row_copy_per_slot: 1.0,
    decode_miss: 1000.0,
    decode_hit: 50.0,
    vdbe_probe: 750.0,
    vdbe_row: 60.0,
    vdbe_count_row: 40.0,
};

/// Phase-2 direct B-tree reader (realistic pread + page-cache numbers, not the fs::read prototype).
pub const BTREE_COSTS: CostConstants = CostConstants {
    scan_row: 30.0,
    scan_col: 2.0,
    seek_random: 1300.0,
    seek_sorted: 300.0,
    ..SQL_CURSOR_COSTS
};

pub const MEMORY_COSTS: CostConstants = CostConstants {
    scan_row: 20.0,
    scan_col: 2.0,
    seek_random: 300.0,
    seek_sorted: 150.0,
    ..SQL_CURSOR_COSTS
};
