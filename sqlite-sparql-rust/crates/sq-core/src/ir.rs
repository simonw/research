//! Logical IR: output of `sq-plan::lower_query`, input of the optimizer AND of the naive
//! `sq-oracle` evaluator. It is spargebra's algebra with variables resolved to slots,
//! constants resolved to canonical ids (TEMP ids for constants absent from the store),
//! sub-select scopes alpha-renamed, and the dataset made explicit.
use std::sync::Arc;

use crate::batch::Slot;
use crate::eval_api::TempDict;
use crate::id::TermId;

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum PatTerm {
    Slot(Slot),
    Const(TermId),
}

/// Which graph a quad pattern / path reads.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum GraphSel {
    /// The dataset's default graph (`DatasetSpec::default`).
    Default,
    /// Logical IR: the active graph of the innermost enclosing `Logical::Graph`.
    /// Physical plan: the innermost enclosing `Op::GraphLoop` register.
    Active,
    /// One named graph (membership in the dataset already checked by the planner).
    Const(TermId),
    /// Physical plan only: GRAPH ?g pushed into the pattern; ranges over the dataset's named
    /// graphs and binds the slot (never binds the default graph).
    Slot(Slot),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct QuadPat {
    pub g: GraphSel,
    pub s: PatTerm,
    pub p: PatTerm,
    pub o: PatTerm,
}

/// Property path with constants resolved. spargebra already rewrites top-level `a/b`
/// into a BGP with a fresh blank-node middle and `!(a|^b)` into Alt(Nps, Inv(Nps)).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum PathPlan {
    Pred(TermId),
    Inv(Box<PathPlan>),
    Seq(Box<PathPlan>, Box<PathPlan>),
    Alt(Box<PathPlan>, Box<PathPlan>),
    ZeroOrMore(Box<PathPlan>),
    OneOrMore(Box<PathPlan>),
    ZeroOrOne(Box<PathPlan>),
    /// Negated property set (forward direction only; inverse sets arrive wrapped in Inv).
    Nps(Vec<TermId>),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ExistsId(pub u32);

/// Scalar expression. Constants are canonical ids (TEMP ids live in the query's TempDict).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Expr {
    Const(TermId),
    Slot(Slot),
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Equal(Box<Expr>, Box<Expr>),
    SameTerm(Box<Expr>, Box<Expr>),
    Greater(Box<Expr>, Box<Expr>),
    GreaterOrEqual(Box<Expr>, Box<Expr>),
    Less(Box<Expr>, Box<Expr>),
    LessOrEqual(Box<Expr>, Box<Expr>),
    In(Box<Expr>, Vec<Expr>),
    Add(Box<Expr>, Box<Expr>),
    Subtract(Box<Expr>, Box<Expr>),
    Multiply(Box<Expr>, Box<Expr>),
    Divide(Box<Expr>, Box<Expr>),
    UnaryPlus(Box<Expr>),
    UnaryMinus(Box<Expr>),
    Bound(Slot),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Coalesce(Vec<Expr>),
    /// `exists[id]` evaluated by substitution with the current row (spec 18.6).
    Exists(ExistsId),
    /// Builtins 1:1 with spargebra (sparql-12 / sep features OFF). `Custom(xsd:*)` = casts;
    /// any other Custom is an evaluation error (unknown function).
    Call(spargebra::algebra::Function, Vec<Expr>),
}

impl Expr {
    /// Slots read by this expression, NOT including slots inside EXISTS patterns.
    pub fn slots(&self, out: &mut Vec<Slot>) {
        match self {
            Expr::Const(_) | Expr::Exists(_) => {}
            Expr::Slot(s) | Expr::Bound(s) => out.push(*s),
            Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => a.slots(out),
            Expr::Or(a, b)
            | Expr::And(a, b)
            | Expr::Equal(a, b)
            | Expr::SameTerm(a, b)
            | Expr::Greater(a, b)
            | Expr::GreaterOrEqual(a, b)
            | Expr::Less(a, b)
            | Expr::LessOrEqual(a, b)
            | Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Divide(a, b) => {
                a.slots(out);
                b.slots(out);
            }
            Expr::In(a, l) => {
                a.slots(out);
                l.iter().for_each(|e| e.slots(out));
            }
            Expr::If(a, b, c) => {
                a.slots(out);
                b.slots(out);
                c.slots(out);
            }
            Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().for_each(|e| e.slots(out)),
        }
    }
    pub fn contains_exists(&self) -> bool {
        match self {
            Expr::Exists(_) => true,
            Expr::Const(_) | Expr::Slot(_) | Expr::Bound(_) => false,
            Expr::Not(a) | Expr::UnaryPlus(a) | Expr::UnaryMinus(a) => a.contains_exists(),
            Expr::Or(a, b)
            | Expr::And(a, b)
            | Expr::Equal(a, b)
            | Expr::SameTerm(a, b)
            | Expr::Greater(a, b)
            | Expr::GreaterOrEqual(a, b)
            | Expr::Less(a, b)
            | Expr::LessOrEqual(a, b)
            | Expr::Add(a, b)
            | Expr::Subtract(a, b)
            | Expr::Multiply(a, b)
            | Expr::Divide(a, b) => a.contains_exists() || b.contains_exists(),
            Expr::In(a, l) => a.contains_exists() || l.iter().any(Expr::contains_exists),
            Expr::If(a, b, c) => a.contains_exists() || b.contains_exists() || c.contains_exists(),
            Expr::Coalesce(l) | Expr::Call(_, l) => l.iter().any(Expr::contains_exists),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum AggFunc {
    /// COUNT(*) (with `distinct`: COUNT(DISTINCT *) over all in-scope slots of the input).
    CountAll,
    Count,
    Sum,
    Avg,
    Min,
    Max,
    Sample,
    GroupConcat { separator: String },
    /// Unknown custom aggregate: evaluation error -> unbound.
    Custom(String),
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct AggSpec {
    pub func: AggFunc,
    pub distinct: bool,
    pub arg: Option<Expr>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct OrderKey {
    pub expr: Expr,
    pub desc: bool,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Logical {
    /// One empty solution (empty group pattern `{}`).
    Unit,
    /// No solutions.
    Empty,
    Bgp(Vec<QuadPat>),
    Path { g: GraphSel, s: PatTerm, path: PathPlan, o: PatTerm },
    Join(Box<Logical>, Box<Logical>),
    LeftJoin { left: Box<Logical>, right: Box<Logical>, filter: Option<Expr> },
    Minus { left: Box<Logical>, right: Box<Logical> },
    Union(Box<Logical>, Box<Logical>),
    Filter { input: Box<Logical>, expr: Expr },
    Extend { input: Box<Logical>, slot: Slot, expr: Expr },
    /// Rows use TermId::UNBOUND for UNDEF.
    Values { slots: Vec<Slot>, rows: Vec<Vec<TermId>> },
    /// GRAPH name { input }: name is Const (IRI) or Slot (variable).
    Graph { name: PatTerm, input: Box<Logical> },
    Group { input: Box<Logical>, keys: Vec<Slot>, aggs: Vec<(Slot, AggSpec)> },
    OrderBy { input: Box<Logical>, keys: Vec<OrderKey> },
    /// Keeps `slots`, resets every other slot to UNBOUND (sub-select scoping; slots that are
    /// not projected were alpha-renamed to fresh slots during lowering).
    Project { input: Box<Logical>, slots: Vec<Slot> },
    Distinct(Box<Logical>),
    Reduced(Box<Logical>),
    Slice { input: Box<Logical>, offset: u64, limit: Option<u64> },
    /// SERVICE: always Error::Unsupported unless silent (then Unit).
    Service { silent: bool },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum VarOrigin {
    User,
    /// Blank node in a pattern (one hidden slot per label per BGP scope).
    BlankNode,
    /// spargebra-generated (path middles, aggregate outputs, DESCRIBE constants).
    Generated,
    /// Alpha-renamed sub-select variable.
    Renamed,
}

#[derive(Clone, Debug, Default)]
pub struct VarTable {
    /// Index = Slot.
    pub vars: Vec<(String, VarOrigin)>,
}

impl VarTable {
    pub fn width(&self) -> usize {
        self.vars.len()
    }
    pub fn name(&self, s: Slot) -> &str {
        &self.vars[s as usize].0
    }
}

/// What the default graph of the query dataset is.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DefaultGraph {
    /// No FROM clause: the store's default graph (g = 0).
    StoreDefault,
    /// No FROM clause and `union_default_graph = 1`: RDF merge of g=0 and every named graph.
    UnionAll,
    /// FROM g1..gk (possibly empty: FROM NAMED only => empty default graph). Ids of graphs not
    /// in the store are TEMP ids (empty graphs). Multi-graph default graphs dedupe (s,p,o).
    Graphs(Arc<[TermId]>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum NamedGraphs {
    /// No dataset clause: every named graph of the store.
    All,
    /// FROM NAMED list (FROM-only queries => empty list).
    Only(Arc<[TermId]>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DatasetSpec {
    pub default: DefaultGraph,
    pub named: NamedGraphs,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum TemplateTerm {
    Const(TermId),
    Slot(Slot),
    /// Template blank node (fresh per solution).
    BNode(u32),
}

#[derive(Clone, PartialEq, Debug)]
pub enum QueryForm {
    /// Output slots in projection order (names in VarTable).
    Select(Vec<Slot>),
    Ask,
    Construct(Vec<[TemplateTerm; 3]>),
    /// Resources to describe (projected slots; spargebra binds IRIs via Extend).
    Describe(Vec<Slot>),
}

#[derive(Clone, Debug)]
pub struct LoweredQuery {
    pub form: QueryForm,
    pub root: Logical,
    pub vars: VarTable,
    pub dataset: DatasetSpec,
    /// Patterns of EXISTS expressions, indexed by ExistsId. They share the global slot layout.
    pub exists: Vec<Logical>,
    /// TEMP ids created during lowering (constants absent from the store).
    pub temps: TempDict,
    pub base_iri: Option<String>,
}
