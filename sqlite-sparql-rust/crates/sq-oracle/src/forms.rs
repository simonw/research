//! Query forms (SPARQL 1.1 §16): SELECT solutions in projection order, ASK, CONSTRUCT and
//! DESCRIBE graphs. Ids are decoded to terms only here.
use std::sync::Arc;

use rustc_hash::FxHashSet;
use sq_core::eval_api::TermResolver;
use sq_core::ir::{GraphSel, QueryForm, TemplateTerm};
use sq_core::{Kind, Result, Slot, Term, TermId};

use crate::bag::Bag;
use crate::dataset::ActiveGraph;
use crate::eval::Evaluator;
use crate::OracleOutput;

impl Evaluator<'_> {
    /// The query result for the solutions of the WHERE pattern.
    pub(crate) fn output(&mut self, solutions: &Bag) -> Result<OracleOutput> {
        let q = self.q;
        match &q.form {
            QueryForm::Select(slots) => {
                let vars = slots.iter().map(|s| q.vars.name(*s).to_owned()).collect();
                let mut rows = Vec::with_capacity(solutions.len());
                for row in solutions.rows() {
                    let mut out = Vec::with_capacity(slots.len());
                    for s in slots {
                        out.push(self.decode(row[*s as usize])?);
                    }
                    rows.push(out);
                }
                Ok(OracleOutput::Solutions { vars, rows })
            }
            QueryForm::Ask => Ok(OracleOutput::Boolean(!solutions.is_empty())),
            QueryForm::Construct(template) => {
                let triples = self.construct(solutions, template);
                self.decode_triples(triples)
            }
            QueryForm::Describe(slots) => {
                let triples = self.describe(solutions, slots)?;
                self.decode_triples(triples)
            }
        }
    }

    /// CONSTRUCT: the template instantiated with every solution, template blank nodes fresh
    /// per solution. Triples with an unbound position, a literal subject or a non-IRI
    /// predicate are skipped. The result is a set, in first-seen order.
    fn construct(&mut self, solutions: &Bag, template: &[[TemplateTerm; 3]]) -> Vec<[TermId; 3]> {
        let mut seen = FxHashSet::default();
        let mut out = Vec::new();
        let mut bnodes: Vec<(u32, TermId)> = Vec::new();
        for row in solutions.rows() {
            bnodes.clear();
            for [s, p, o] in template {
                let mut term = |t: &TemplateTerm| match *t {
                    TemplateTerm::Const(c) => Some(c),
                    TemplateTerm::Slot(x) => Some(row[x as usize]).filter(|v| v.is_bound()),
                    TemplateTerm::BNode(label) => Some(match bnodes.iter().find(|(l, _)| *l == label) {
                        Some((_, id)) => *id,
                        None => {
                            let id = self.resolver.fresh_bnode();
                            bnodes.push((label, id));
                            id
                        }
                    }),
                };
                let (Some(s), Some(p), Some(o)) = (term(s), term(p), term(o)) else { continue };
                if s.kind() == Kind::Literal || p.kind() != Kind::Iri {
                    continue;
                }
                if seen.insert([s, p, o]) {
                    out.push([s, p, o]);
                }
            }
        }
        out
    }

    /// DESCRIBE: the concise bounded description of every described resource in the default
    /// graph (its triples as subject, recursively through blank-node objects), as a set.
    fn describe(&mut self, solutions: &Bag, slots: &[Slot]) -> Result<Vec<[TermId; 3]>> {
        let graph = self.dataset.graph(GraphSel::Default, ActiveGraph::Default);
        let mut described = FxHashSet::default();
        let mut seen = FxHashSet::default();
        let mut out = Vec::new();
        let mut triples = Vec::new();
        for row in solutions.rows() {
            for s in slots {
                let resource = row[*s as usize];
                if !resource.is_bound() || resource.kind() == Kind::Literal || !described.insert(resource) {
                    continue;
                }
                let mut pending = vec![resource];
                while let Some(node) = pending.pop() {
                    triples.clear();
                    self.reader.triples(graph, Some(node), None, None, &mut triples)?;
                    for &t in &triples {
                        if seen.insert(t) {
                            out.push(t);
                        }
                        if t[2].is_bnode() && described.insert(t[2]) {
                            pending.push(t[2]);
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    fn decode(&mut self, id: TermId) -> Result<Option<Arc<Term>>> {
        if id.is_bound() { self.resolver.term(id).map(Some) } else { Ok(None) }
    }

    fn decode_triples(&mut self, triples: Vec<[TermId; 3]>) -> Result<OracleOutput> {
        let mut out = Vec::with_capacity(triples.len());
        for [s, p, o] in triples {
            out.push([self.resolver.term(s)?, self.resolver.term(p)?, self.resolver.term(o)?]);
        }
        Ok(OracleOutput::Graph(out))
    }
}
