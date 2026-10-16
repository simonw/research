//! Random small datasets over a tiny vocabulary, so random queries join, hit constants and
//! traverse paths. Literals cover every id tag: inline integers, decimals, booleans, dates,
//! dateTimes and short strings; dictionary strings, language strings, doubles, floats,
//! derived integer types, non-canonical numerics, ill-typed and unknown-datatype literals.
use std::fmt::{self, Write};

use sq_core::testing::Rng;

/// Subject (and object) IRIs, as `:local` names.
pub const SUBJECTS: [&str; 12] = ["s0", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11"];
/// Small datasets use only the first subjects (denser joins).
const SMALL_SUBJECTS: usize = 6;
pub const PREDICATES: [&str; 4] = ["p0", "p1", "p2", "p3"];
pub const CLASSES: [&str; 2] = ["C0", "C1"];
/// Named graphs of the data.
pub const GRAPHS: [&str; 2] = ["g0", "g1"];
/// A named graph without quads, registered (as by CREATE GRAPH) in some datasets.
pub const EMPTY_GRAPH: &str = "g2";

/// Literals in SPARQL / Turtle syntax (prefixes `:` and `xsd:`).
pub const LITERALS: [&str; 32] = [
    "0",
    "1",
    "2",
    "-1",
    "\"01\"^^xsd:integer",
    "\"+2\"^^xsd:integer",
    "1.5",
    "2.0",
    "\"1.50\"^^xsd:decimal",
    "\"1.0e0\"^^xsd:double",
    "\"2E0\"^^xsd:double",
    "\"NaN\"^^xsd:double",
    "\"1.5\"^^xsd:float",
    "true",
    "false",
    "\"1\"^^xsd:boolean",
    "\"a\"",
    "\"b\"",
    "\"A\"",
    "\"\"",
    "\"a longer string\"",
    "\"a\"^^xsd:string",
    "\"a\"@en",
    "\"b\"@en-US",
    "\"a\"@fr",
    "\"2020-01-01\"^^xsd:date",
    "\"2020-01-01T10:00:00Z\"^^xsd:dateTime",
    "\"2020-01-01T10:00:00.000Z\"^^xsd:dateTime",
    "\"2020-01-01T10:00:00\"^^xsd:dateTime",
    "\"3\"^^xsd:int",
    "\"abc\"^^xsd:integer",
    "\"x\"^^:dt",
];

/// A data term.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum D {
    /// `:local`.
    Iri(&'static str),
    /// `rdf:type`.
    Type,
    /// A literal in Turtle syntax.
    Lit(&'static str),
    /// `_:xN`.
    BNode(u64),
}

impl fmt::Display for D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            D::Iri(i) => write!(f, ":{i}"),
            D::Type => f.write_str("rdf:type"),
            D::Lit(l) => f.write_str(l),
            D::BNode(n) => write!(f, "_:x{n}"),
        }
    }
}

/// The comment line of [`Dataset::trig`] that registers [`EMPTY_GRAPH`].
const EMPTY_MARK: &str = "#empty";

/// Integers of the "numeric column" of medium datasets (integer range pushdown reads them).
pub const INTS: [&str; 16] = ["-5", "-3", "0", "1", "2", "3", "4", "5", "7", "10", "12", "15", "20", "25", "30", "100"];

/// A quad; `graph` None = default graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quad {
    pub graph: Option<&'static str>,
    pub s: D,
    pub p: D,
    pub o: D,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dataset {
    pub quads: Vec<Quad>,
    /// Run the statistics collector after loading (changes Auto plans).
    pub analyze: bool,
    /// Register [`EMPTY_GRAPH`] in the store's graph registry.
    pub empty_graph: bool,
}

impl Dataset {
    pub fn generate(r: &mut Rng) -> Dataset {
        // Mostly small; sometimes a medium dataset (longer runs for merge joins and galloping).
        let medium = r.below(4) == 0;
        let n = if medium { 40 + r.below(100) as usize } else { 6 + r.below(30) as usize };
        let subjects = if medium { &SUBJECTS[..] } else { &SUBJECTS[..SMALL_SUBJECTS] };
        let bnodes = r.below(3);
        let mut quads = Vec::with_capacity(n);
        let pick = |r: &mut Rng, xs: &[&'static str]| xs[r.below(xs.len() as u64) as usize];
        for _ in 0..n {
            let s = if bnodes > 0 && r.below(8) == 0 { D::BNode(r.below(bnodes)) } else { D::Iri(pick(r, subjects)) };
            let (p, o) = if r.below(10) == 0 {
                (D::Type, D::Iri(pick(r, &CLASSES)))
            } else {
                let p = D::Iri(pick(r, &PREDICATES));
                let o = match r.below(20) {
                    0..=8 => D::Iri(pick(r, subjects)),
                    9 if bnodes > 0 => D::BNode(r.below(bnodes)),
                    _ => D::Lit(pick(r, &LITERALS)),
                };
                (p, o)
            };
            let graph = match r.below(10) {
                0 | 1 => Some(GRAPHS[0]),
                2 => Some(GRAPHS[1]),
                _ => None,
            };
            // Some triples are in two graphs.
            if r.below(8) == 0 {
                let other = if graph.is_none() { Some(pick(r, &GRAPHS)) } else { None };
                quads.push(Quad { graph: other, s, p, o });
            }
            quads.push(Quad { graph, s, p, o });
        }
        if medium && r.below(4) != 0 {
            // A numeric column: many integer objects of one predicate (with a few decimals,
            // doubles and non-canonical integers among them), the shape of range pushdown.
            for _ in 0..20 + r.below(40) {
                let o = if r.below(6) == 0 { pick(r, &LITERALS[4..13]) } else { pick(r, &INTS) };
                let graph = if r.below(5) == 0 { Some(GRAPHS[0]) } else { None };
                quads.push(Quad { graph, s: D::Iri(pick(r, subjects)), p: D::Iri(PREDICATES[3]), o: D::Lit(o) });
            }
        }
        Dataset { quads, analyze: r.below(3) == 0, empty_graph: r.below(4) == 0 }
    }

    /// The literals that occur in the data.
    pub fn literals(&self) -> Vec<&'static str> {
        LITERALS.iter().chain(&INTS).copied().filter(|l| self.quads.iter().any(|q| q.o == D::Lit(l))).collect()
    }

    /// TriG text (blank-node labels are shared across graphs: one document, one load). A
    /// registered empty graph, which TriG cannot express, is noted in a `#empty` comment.
    pub fn trig(&self) -> String {
        let mut out = String::from(
            "@prefix : <http://ex/> .\n@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .\n\
             @prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n",
        );
        if self.empty_graph {
            let _ = writeln!(out, "{EMPTY_MARK} :{EMPTY_GRAPH}");
        }
        for q in &self.quads {
            match q.graph {
                None => {
                    let _ = writeln!(out, "{} {} {} .", q.s, q.p, q.o);
                }
                Some(g) => {
                    let _ = writeln!(out, ":{g} {{ {} {} {} }}", q.s, q.p, q.o);
                }
            }
        }
        out
    }

    /// Parses the TriG printed by [`Dataset::trig`] back (one triple or one graph block of
    /// one triple per line), for replaying reports.
    pub fn from_trig(trig: &str, analyze: bool) -> Dataset {
        let term = |t: &str| -> D {
            if t == "rdf:type" {
                D::Type
            } else if let Some(n) = t.strip_prefix("_:x") {
                D::BNode(n.parse().expect("blank node label"))
            } else if let Some(l) = LITERALS.iter().chain(&INTS).find(|l| **l == t) {
                D::Lit(l)
            } else {
                let local = t.strip_prefix(':').expect("an IRI of the test vocabulary");
                let all = SUBJECTS.iter().chain(&PREDICATES).chain(&CLASSES).chain(&GRAPHS);
                D::Iri(all.copied().find(|x| *x == local).expect("a known local name"))
            }
        };
        let mut quads = Vec::new();
        let mut empty_graph = false;
        for line in trig.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('@')) {
            if line.starts_with(EMPTY_MARK) {
                empty_graph = true;
                continue;
            }
            let (graph, triple) = match line.strip_suffix('}') {
                Some(body) => {
                    let (g, t) = body.split_once('{').expect("graph block");
                    let g = g.trim().strip_prefix(':').expect("graph IRI");
                    (Some(*GRAPHS.iter().find(|x| **x == g).expect("known graph")), t.trim().to_owned())
                }
                None => (None, line.strip_suffix('.').expect("triple").trim().to_owned()),
            };
            // Literals in the vocabulary contain no spaces except "a longer string".
            let (s, rest) = triple.split_once(' ').expect("subject");
            let (p, o) = rest.split_once(' ').expect("predicate");
            quads.push(Quad { graph, s: term(s), p: term(p), o: term(o.trim()) });
        }
        Dataset { quads, analyze, empty_graph }
    }

    /// Smaller datasets: no statistics, no empty graph, halves, then each single quad removed.
    pub fn simplifications(&self) -> Vec<Dataset> {
        let mut out = Vec::new();
        let n = self.quads.len();
        if self.analyze {
            out.push(Dataset { analyze: false, ..self.clone() });
        }
        if self.empty_graph {
            out.push(Dataset { empty_graph: false, ..self.clone() });
        }
        if n >= 4 {
            out.push(Dataset { quads: self.quads[..n / 2].to_vec(), ..self.clone() });
            out.push(Dataset { quads: self.quads[n / 2..].to_vec(), ..self.clone() });
        }
        for i in 0..n {
            let mut d = self.clone();
            d.quads.remove(i);
            out.push(d);
        }
        out
    }
}
