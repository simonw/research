//! Physical plan: produced by sq-plan, consumed by sq-exec, printed by `sparql_explain`.
use std::ops::Bound;

use crate::batch::Slot;
use crate::eval_api::TempDict;
use crate::id::TermId;
use crate::ir::{AggSpec, DatasetSpec, Expr, GraphSel, OrderKey, PatTerm, PathPlan, QuadPat, QueryForm, VarTable};
use crate::store::IndexOrder;

/// How a re-opened subtree treats seed values.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum SeedMode {
    /// Sideways information passing (BindJoin): only legal for slots in safe_inject(subtree).
    /// Seeded slots behave as bound values that are merged into every output row.
    Join,
    /// EXISTS (spec 18.6 substitute): seeded slots behave as constants everywhere in the
    /// subtree, including OPTIONAL/MINUS/FILTER; Minus excludes seeded slots from its
    /// domain-intersection test; zero-length paths treat seeded endpoints as constants.
    Substitute,
}

#[derive(Clone, PartialEq, Debug)]
pub enum JoinKind {
    Inner,
    /// OPTIONAL: filter evaluated on the MERGED row; left row emitted alone iff no compatible
    /// right row passes the filter (errors count as false).
    Left { filter: Option<Expr> },
    /// Keep left rows with >= 1 compatible right row (decorrelated EXISTS).
    Semi,
    /// Keep left rows with no compatible right row (decorrelated NOT EXISTS).
    Anti,
}

/// Integer-range pushdown on the object of a Scan whose order has o as the first free column
/// (Gpos with g,p bound). Exact by construction: the Scan ALSO reads `rdf_gpox` for (g,p)
/// (dictionary-typed and decimal objects) and the original FILTER stays above the Scan.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct IntRange {
    pub lo: i64,
    pub hi: i64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ScanOp {
    pub pat: QuadPat,
    /// Chosen index; the prefix is the constant/seeded positions (in index order).
    pub order: IndexOrder,
    pub range: Option<IntRange>,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Plan {
    /// Pre-order id, unique in the QueryPlan (EXPLAIN ANALYZE counters are keyed by it).
    pub id: u32,
    pub est_rows: f64,
    pub est_cost_ns: f64,
    /// Output sorted ascending (signed i64) by this slot, if guaranteed.
    pub sorted_by: Option<Slot>,
    pub op: Op,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Op {
    Unit,
    Empty,
    Scan(ScanOp),
    Values { slots: Vec<Slot>, rows: Vec<Vec<TermId>> },
    /// Index nested loop / sideways passing: for each distinct tuple of `seed` slots of the
    /// left input (optionally sorted for locality), re-open `right` with SeedMode::Join and
    /// merge. Runs of equal keys reuse the previous probe's result. `adaptive`: switch to a
    /// hash join when actual left rows > 8x est_rows and > 50_000.
    BindJoin { left: Box<Plan>, right: Box<Plan>, seed: Vec<Slot>, kind: JoinKind, sort_left: bool, adaptive: bool },
    /// keys = certainly-bound shared slots; check = maybe-bound shared slots.
    HashJoin { build: Box<Plan>, probe: Box<Plan>, keys: Vec<Slot>, check: Vec<Slot>, kind: JoinKind, build_is_left: bool },
    /// Both inputs sorted by `key` (certainly bound on both sides). Galloping seek after
    /// `gallop` consecutive misses when the right input is a Scan.
    MergeJoin { left: Box<Plan>, right: Box<Plan>, key: Slot, check: Vec<Slot>, gallop: u32 },
    /// Leapfrog intersection on one slot over k inputs all sorted by it (subject stars).
    StarMerge { key: Slot, inputs: Vec<Plan>, check: Vec<Slot> },
    NestedLoop { left: Box<Plan>, right: Box<Plan>, check: Vec<Slot>, kind: JoinKind },
    /// SPARQL MINUS: remove a left row iff some right row is compatible AND shares at least
    /// one bound slot with it. Right evaluated independently. keys/check as in HashJoin.
    Minus { left: Box<Plan>, right: Box<Plan>, keys: Vec<Slot>, check: Vec<Slot> },
    Union(Vec<Plan>),
    Filter { input: Box<Plan>, expr: Expr },
    Extend { input: Box<Plan>, slot: Slot, expr: Expr, canonical: bool },
    Path { g: GraphSel, s: PatTerm, path: PathPlan, o: PatTerm },
    /// GRAPH ?g that cannot be pushed into patterns: for each named graph of the dataset (sorted),
    /// push it on the graph register stack, re-open `input`, drop rows whose `slot` is bound
    /// to a different graph, set `slot`.
    GraphLoop { slot: Slot, input: Box<Plan> },
    /// GRAPH ?g {} : one row per named graph of the dataset.
    GraphNames { slot: Slot },
    /// Hash (or streaming, when input sorted by keys[0] and keys.len()==1) aggregation.
    /// Group keys that evaluate to an error use TermId::ERROR (own group; output UNBOUND).
    Group { input: Box<Plan>, keys: Vec<Slot>, aggs: Vec<(Slot, AggSpec)>, streaming: bool },
    /// COUNT(*) without DISTINCT/GROUP BY: one row binding `out`. Fast paths: input Scan ->
    /// store.count() in the VDBE; input MergeJoin -> factorized sum of run products.
    Count { input: Box<Plan>, out: Slot },
    /// Full SPARQL ORDER BY (stable; precomputed SortKeys).
    Sort { input: Box<Plan>, keys: Vec<OrderKey> },
    /// Enforcer for merge joins: stable sort by raw i64 of `key`.
    SortBySlot { input: Box<Plan>, key: Slot },
    /// ORDER BY + LIMIT: bounded heap of k = offset + limit; never placed across Distinct.
    TopK { input: Box<Plan>, keys: Vec<OrderKey>, k: u64 },
    Distinct { input: Box<Plan>, slots: Vec<Slot> },
    Reduced { input: Box<Plan>, slots: Vec<Slot> },
    Slice { input: Box<Plan>, offset: u64, limit: Option<u64> },
    /// Keeps `slots`, resets all others to UNBOUND.
    Project { input: Box<Plan>, slots: Vec<Slot> },
    /// Hybrid: a left-deep join pipeline rendered as ONE SQL statement (CROSS JOIN + INDEXED BY,
    /// order fixed by our planner) and stepped by SQLite's VDBE; result columns -> `out` slots.
    SqlJoin { sql: String, params: Vec<i64>, out: Vec<Slot> },
}

impl Plan {
    pub fn children(&self) -> Vec<&Plan> {
        match &self.op {
            Op::Unit | Op::Empty | Op::Scan(_) | Op::Values { .. } | Op::Path { .. } | Op::GraphNames { .. } | Op::SqlJoin { .. } => vec![],
            Op::BindJoin { left, right, .. }
            | Op::MergeJoin { left, right, .. }
            | Op::NestedLoop { left, right, .. }
            | Op::Minus { left, right, .. } => vec![left, right],
            Op::HashJoin { build, probe, .. } => vec![build, probe],
            Op::StarMerge { inputs, .. } | Op::Union(inputs) => inputs.iter().collect(),
            Op::Filter { input, .. }
            | Op::Extend { input, .. }
            | Op::GraphLoop { input, .. }
            | Op::Group { input, .. }
            | Op::Count { input, .. }
            | Op::Sort { input, .. }
            | Op::SortBySlot { input, .. }
            | Op::TopK { input, .. }
            | Op::Distinct { input, .. }
            | Op::Reduced { input, .. }
            | Op::Slice { input, .. }
            | Op::Project { input, .. } => vec![input],
        }
    }
}

#[derive(Clone, Debug)]
pub struct QueryPlan {
    pub form: QueryForm,
    pub root: Plan,
    pub vars: VarTable,
    pub dataset: DatasetSpec,
    /// Named graphs of the dataset, resolved at plan time (sorted).
    pub named_graphs: Vec<TermId>,
    /// EXISTS sub-plans indexed by ExistsId (opened with SeedMode::Substitute).
    pub exists: Vec<Plan>,
    pub temps: TempDict,
    pub base_iri: Option<String>,
}

impl QueryPlan {
    pub fn width(&self) -> usize {
        self.vars.width()
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum EngineKind {
    #[default]
    Native,
    /// Baseline SPARQL->SQL translation (sq-sqlgen).
    Sql,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum JoinPolicy {
    #[default]
    Auto,
    PreferHash,
    PreferBind,
    PreferMerge,
    /// Never use SqlJoin (differential tests / memstore).
    NoSql,
}

#[derive(Clone, Debug)]
pub struct QueryOptions {
    pub engine: EngineKind,
    pub timeout_ms: Option<u64>,
    pub max_memory_mb: u64,
    pub union_default_graph: bool,
    pub join_policy: JoinPolicy,
    /// false = keep syntactic join order (differential testing).
    pub reorder: bool,
    pub base_iri: Option<String>,
    /// Max runtime bounded counts per query and their cap.
    pub max_probe_counts: u32,
    pub probe_count_cap: u64,
}

impl Default for QueryOptions {
    fn default() -> Self {
        QueryOptions {
            engine: EngineKind::Native,
            timeout_ms: None,
            max_memory_mb: 1024,
            union_default_graph: false,
            join_policy: JoinPolicy::Auto,
            reorder: true,
            base_iri: None,
            max_probe_counts: 32,
            probe_count_cap: 1000,
        }
    }
}

/// Per-plan-node runtime counters (EXPLAIN ANALYZE), indexed by Plan::id.
#[derive(Clone, Debug, Default)]
pub struct NodeProfile {
    pub rows_out: u64,
    pub opens: u64,
    pub seeks: u64,
    pub time_ns: u64,
}

#[derive(Clone, Debug, Default)]
pub struct ExecProfile {
    pub nodes: Vec<NodeProfile>,
    pub decode_misses: u64,
    pub decode_hits: u64,
    pub plan_ns: u64,
    pub exec_ns: u64,
}

/// Helper for templates/tests: an unbounded range.
pub const UNBOUNDED: (Bound<i64>, Bound<i64>) = (Bound::Unbounded, Bound::Unbounded);
