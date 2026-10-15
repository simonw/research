//! Random SPARQL queries: a small AST, its printer, a seeded generator and the one-step
//! simplifications used to shrink a failing query.
//!
//! The generator only produces queries whose answer is deterministic: LIMIT / OFFSET appear
//! only under an ORDER BY over every projected variable, and nothing reads the clock,
//! randomness or input order (no NOW, RAND, UUID, SAMPLE or GROUP_CONCAT). Queries that
//! spargebra rejects (scoping rules) are simply skipped by the runner.
mod generate;
mod print;
mod shrink;

use super::data::GRAPHS;

pub use generate::Gen;
pub use print::PREFIXES;

/// Variable names; `?g` is mostly used for graphs.
pub const VARS: [&str; 7] = ["a", "b", "c", "d", "e", "f", "g"];
const GRAPH_VAR: usize = 6;

/// Graphs a query may name: the data's named graphs plus one that never exists.
const QUERY_GRAPHS: [&str; 3] = [GRAPHS[0], GRAPHS[1], "gx"];

/// A term in a pattern position.
#[derive(Clone, Debug, PartialEq)]
pub enum T {
    Var(usize),
    /// `:local`.
    Iri(&'static str),
    /// A literal in SPARQL / Turtle syntax.
    Lit(&'static str),
    /// A labelled pattern blank node (`_:kN`; labels are unique per query).
    BNode(usize),
    /// `rdf:type` (a well-known dictionary IRI).
    Type,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Path {
    Pred(&'static str),
    Inv(Box<Path>),
    Seq(Box<Path>, Box<Path>),
    Alt(Box<Path>, Box<Path>),
    Star(Box<Path>),
    Plus(Box<Path>),
    Opt(Box<Path>),
    /// `!(p | ^q ...)`: (inverse?, predicate).
    Nps(Vec<(bool, &'static str)>),
}

/// An element of a group graph pattern.
#[derive(Clone, Debug, PartialEq)]
pub enum El {
    Triple(T, T, T),
    Path(T, Path, T),
    Optional(Group),
    Union(Group, Group),
    Minus(Group),
    Filter(E),
    Bind(E, usize),
    Values(Vec<usize>, Vec<Vec<Option<T>>>),
    Graph(T, Group),
    Sub(Box<Select>),
    Group(Group),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Group(pub Vec<El>);

#[derive(Clone, Debug, PartialEq)]
pub enum Agg {
    CountAll(bool),
    Count(bool, usize),
    Sum(bool, usize),
    Avg(bool, usize),
    Min(usize),
    Max(usize),
    /// An aggregate of an expression: (function, distinct, argument).
    Expr(&'static str, bool, Box<E>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum E {
    Var(usize),
    /// A constant in SPARQL syntax (`:s1`, `1`, `"a"@en`, ...).
    Const(&'static str),
    /// Infix operator: `=`, `!=`, `<`, `>`, `<=`, `>=`, `+`, `-`, `*`, `/`, `&&`, `||`.
    Bin(&'static str, Box<E>, Box<E>),
    Not(Box<E>),
    Neg(Box<E>),
    Call(&'static str, Vec<E>),
    Bound(usize),
    In(Box<E>, Vec<E>, bool),
    Exists(bool, Group),
    /// Only in projections and HAVING.
    Agg(Box<Agg>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Modifier {
    None,
    Distinct,
    Reduced,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Var(usize),
    /// `(expr AS ?v)`; expr may contain aggregates when the query groups.
    Expr(E, usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Select {
    pub modifier: Modifier,
    /// None = `SELECT *`.
    pub items: Option<Vec<Item>>,
    pub pattern: Group,
    /// `Some` (possibly empty) when the query aggregates: `?v`, or `(expr AS ?v)`.
    pub group_by: Option<Vec<(usize, Option<E>)>>,
    pub having: Option<E>,
    /// Expression keys, printed before the variable keys (`order` still covers every
    /// projected variable, so the order stays total on the output).
    pub order_exprs: Vec<(E, bool)>,
    pub order: Vec<(usize, bool)>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Form {
    Select(Select),
    Ask(Group),
    Construct(Vec<(T, T, T)>, Group),
    Describe(Vec<T>, Group),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub form: Form,
    pub from: Vec<&'static str>,
    pub from_named: Vec<&'static str>,
    /// Run with `QueryOptions::union_default_graph` (no FROM: the default graph is the RDF
    /// merge of every graph).
    pub union_default: bool,
    /// Let the planner run bounded counts against the store (otherwise plans rely on
    /// statistics and defaults only): another source of plan diversity.
    pub probes: bool,
}

/// How results must be compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// A multiset.
    Bag,
    /// A set (REDUCED may drop any number of duplicates).
    Set,
    /// A sequence: ORDER BY covers every projected variable.
    Sequence,
}

impl Query {
    pub fn order(&self) -> Order {
        match &self.form {
            Form::Select(s) if s.modifier == Modifier::Reduced => Order::Set,
            Form::Select(s) if s.orders_all() => Order::Sequence,
            _ => Order::Bag,
        }
    }
}

impl Select {
    /// The projected variables, None for `SELECT *`.
    pub fn projected(&self) -> Option<Vec<usize>> {
        self.items.as_ref().map(|items| {
            items
                .iter()
                .map(|i| match i {
                    Item::Var(v) | Item::Expr(_, v) => *v,
                })
                .collect()
        })
    }

    /// ORDER BY mentions every projected variable (so the order is total on the output).
    pub fn orders_all(&self) -> bool {
        match self.projected() {
            Some(p) => !p.is_empty() && p.iter().all(|v| self.order.iter().any(|(o, _)| o == v)),
            None => false,
        }
    }
}
