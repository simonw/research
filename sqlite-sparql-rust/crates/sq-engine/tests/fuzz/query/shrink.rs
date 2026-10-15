//! One-step simplifications of a query, for shrinking a failing case: every candidate is
//! strictly smaller in some respect (fewer nodes, clauses or modifiers).
use super::*;

impl Query {
    pub fn simplifications(&self) -> Vec<Query> {
        let mut out = Vec::new();
        if self.union_default {
            out.push(Query { union_default: false, ..self.clone() });
        }
        if !self.probes {
            out.push(Query { probes: true, ..self.clone() });
        }
        if !self.from.is_empty() {
            out.push(Query { from: Vec::new(), ..self.clone() });
            for i in 0..self.from.len() {
                let mut q = self.clone();
                q.from.remove(i);
                out.push(q);
            }
        }
        if !self.from_named.is_empty() {
            out.push(Query { from_named: Vec::new(), ..self.clone() });
            for i in 0..self.from_named.len() {
                let mut q = self.clone();
                q.from_named.remove(i);
                out.push(q);
            }
        }
        let with_form = |form: Form| Query { form, ..self.clone() };
        match &self.form {
            Form::Select(s) => {
                // A SELECT whose WHERE alone still fails is the smallest repro.
                if s.items.is_some() || s.modifier != Modifier::None || s.group_by.is_some() || !s.order.is_empty() {
                    out.push(with_form(Form::Select(Select {
                        modifier: Modifier::None,
                        items: None,
                        pattern: s.pattern.clone(),
                        group_by: None,
                        having: None,
                        order_exprs: Vec::new(),
                        order: Vec::new(),
                        offset: None,
                        limit: None,
                    })));
                }
                out.extend(s.simplifications().into_iter().map(|s| with_form(Form::Select(s))));
            }
            Form::Ask(g) => {
                out.extend(g.simplifications().into_iter().map(|g| with_form(Form::Ask(g))));
            }
            Form::Construct(t, g) => {
                if t.len() > 1 {
                    for i in 0..t.len() {
                        let mut t2 = t.clone();
                        t2.remove(i);
                        out.push(with_form(Form::Construct(t2, g.clone())));
                    }
                }
                out.extend(g.simplifications().into_iter().map(|g| with_form(Form::Construct(t.clone(), g))));
            }
            Form::Describe(t, g) => {
                out.extend(g.simplifications().into_iter().map(|g| with_form(Form::Describe(t.clone(), g))));
            }
        }
        out
    }
}

impl Select {
    fn simplifications(&self) -> Vec<Select> {
        let mut out = Vec::new();
        if self.modifier != Modifier::None {
            out.push(Select { modifier: Modifier::None, ..self.clone() });
        }
        if self.having.is_some() {
            out.push(Select { having: None, ..self.clone() });
        }
        if self.limit.is_some() {
            out.push(Select { limit: None, ..self.clone() });
        }
        if self.offset.is_some() {
            out.push(Select { offset: None, ..self.clone() });
        }
        if !self.order.is_empty() && self.limit.is_none() && self.offset.is_none() {
            out.push(Select { order: Vec::new(), order_exprs: Vec::new(), ..self.clone() });
        }
        if !self.order_exprs.is_empty() {
            out.push(Select { order_exprs: Vec::new(), ..self.clone() });
        }
        if let Some(items) = &self.items
            && items.len() > 1
        {
            for i in 0..items.len() {
                let mut s = self.clone();
                let (Item::Var(v) | Item::Expr(_, v)) = s.items.as_mut().unwrap().remove(i);
                s.order.retain(|(o, _)| *o != v);
                out.push(s);
            }
        }
        if let Some(items) = &self.items {
            for (i, item) in items.iter().enumerate() {
                if let Item::Expr(e, v) = item {
                    for e2 in e.simplifications() {
                        let mut s = self.clone();
                        s.items.as_mut().unwrap()[i] = Item::Expr(e2, *v);
                        out.push(s);
                    }
                }
            }
        }
        if let Some(keys) = &self.group_by
            && !keys.is_empty()
        {
            for i in 0..keys.len() {
                let mut s = self.clone();
                let (k, _) = s.group_by.as_mut().unwrap().remove(i);
                if let Some(items) = s.items.as_mut() {
                    items.retain(|it| *it != Item::Var(k));
                }
                s.order.retain(|(o, _)| *o != k);
                out.push(s);
            }
        }
        out.extend(self.pattern.simplifications().into_iter().map(|g| Select { pattern: g, ..self.clone() }));
        out
    }
}

impl Group {
    fn simplifications(&self) -> Vec<Group> {
        let mut out = Vec::new();
        // Drop one element.
        for i in 0..self.0.len() {
            let mut g = self.clone();
            g.0.remove(i);
            out.push(g);
        }
        // Replace one element by simpler ones (inline its groups) or simplify inside it.
        for i in 0..self.0.len() {
            for repl in self.0[i].replacements() {
                let mut g = self.clone();
                g.0.splice(i..=i, repl);
                out.push(g);
            }
            for el in self.0[i].simplifications() {
                let mut g = self.clone();
                g.0[i] = el;
                out.push(g);
            }
        }
        out
    }
}

impl El {
    /// Sequences of elements that may replace this one (inlined sub-groups).
    fn replacements(&self) -> Vec<Vec<El>> {
        match self {
            El::Optional(g) | El::Minus(g) | El::Graph(_, g) | El::Group(g) => vec![g.0.clone()],
            El::Union(a, b) => vec![a.0.clone(), b.0.clone()],
            El::Sub(s) => vec![s.pattern.0.clone()],
            El::Path(s, Path::Pred(p), o) => vec![vec![El::Triple(s.clone(), T::Iri(p), o.clone())]],
            _ => Vec::new(),
        }
    }

    fn simplifications(&self) -> Vec<El> {
        match self {
            El::Optional(g) => g.simplifications().into_iter().map(El::Optional).collect(),
            El::Minus(g) => g.simplifications().into_iter().map(El::Minus).collect(),
            El::Group(g) => g.simplifications().into_iter().map(El::Group).collect(),
            El::Graph(t, g) => g.simplifications().into_iter().map(|g| El::Graph(t.clone(), g)).collect(),
            El::Union(a, b) => {
                let mut v: Vec<El> = a.simplifications().into_iter().map(|a| El::Union(a, b.clone())).collect();
                v.extend(b.simplifications().into_iter().map(|b| El::Union(a.clone(), b)));
                v
            }
            El::Sub(s) => s.simplifications().into_iter().map(|s| El::Sub(Box::new(s))).collect(),
            El::Filter(e) => e.simplifications().into_iter().map(El::Filter).collect(),
            El::Bind(e, v) => e.simplifications().into_iter().map(|e| El::Bind(e, *v)).collect(),
            El::Path(s, p, o) => p.simplifications().into_iter().map(|p| El::Path(s.clone(), p, o.clone())).collect(),
            El::Values(vars, rows) => {
                let mut v = Vec::new();
                if rows.len() > 1 {
                    for i in 0..rows.len() {
                        let mut r = rows.clone();
                        r.remove(i);
                        v.push(El::Values(vars.clone(), r));
                    }
                }
                if vars.len() > 1 {
                    for i in 0..vars.len() {
                        let mut vs = vars.clone();
                        vs.remove(i);
                        let r = rows
                            .iter()
                            .map(|row| {
                                let mut row = row.clone();
                                row.remove(i);
                                row
                            })
                            .collect();
                        v.push(El::Values(vs, r));
                    }
                }
                v
            }
            El::Triple(..) => Vec::new(),
        }
    }
}

impl Path {
    fn simplifications(&self) -> Vec<Path> {
        match self {
            Path::Pred(_) => Vec::new(),
            Path::Inv(p) | Path::Star(p) | Path::Plus(p) | Path::Opt(p) => {
                let mut v = vec![(**p).clone()];
                let wrap = |q: Path| match self {
                    Path::Inv(_) => Path::Inv(Box::new(q)),
                    Path::Star(_) => Path::Star(Box::new(q)),
                    Path::Plus(_) => Path::Plus(Box::new(q)),
                    _ => Path::Opt(Box::new(q)),
                };
                v.extend(p.simplifications().into_iter().map(wrap));
                v
            }
            Path::Seq(a, b) | Path::Alt(a, b) => {
                let mut v = vec![(**a).clone(), (**b).clone()];
                let seq = matches!(self, Path::Seq(..));
                let pair = |x: Path, y: Path| {
                    if seq { Path::Seq(Box::new(x), Box::new(y)) } else { Path::Alt(Box::new(x), Box::new(y)) }
                };
                v.extend(a.simplifications().into_iter().map(|x| pair(x, (**b).clone())));
                v.extend(b.simplifications().into_iter().map(|y| pair((**a).clone(), y)));
                v
            }
            Path::Nps(set) if set.len() > 1 => (0..set.len())
                .map(|i| {
                    let mut s = set.clone();
                    s.remove(i);
                    Path::Nps(s)
                })
                .collect(),
            Path::Nps(_) => Vec::new(),
        }
    }
}

impl E {
    fn simplifications(&self) -> Vec<E> {
        let mut out = Vec::new();
        match self {
            E::Var(_) | E::Const(_) | E::Bound(_) | E::Agg(_) => {}
            E::Bin(op, a, b) => {
                if matches!(*op, "&&" | "||") {
                    out.push((**a).clone());
                    out.push((**b).clone());
                }
                out.extend(a.simplifications().into_iter().map(|x| E::Bin(op, Box::new(x), b.clone())));
                out.extend(b.simplifications().into_iter().map(|y| E::Bin(op, a.clone(), Box::new(y))));
            }
            E::Not(a) => {
                out.push((**a).clone());
                out.extend(a.simplifications().into_iter().map(|x| E::Not(Box::new(x))));
            }
            E::Neg(a) => out.push((**a).clone()),
            E::Call(f, args) => {
                if matches!(*f, "IF" | "COALESCE") {
                    out.extend(args.iter().skip(usize::from(*f == "IF")).cloned());
                }
                for i in 0..args.len() {
                    for x in args[i].simplifications() {
                        let mut a = args.clone();
                        a[i] = x;
                        out.push(E::Call(f, a));
                    }
                }
            }
            E::In(a, list, not) => {
                if list.len() > 1 {
                    for i in 0..list.len() {
                        let mut l = list.clone();
                        l.remove(i);
                        out.push(E::In(a.clone(), l, *not));
                    }
                }
            }
            E::Exists(not, g) => {
                if *not {
                    out.push(E::Exists(false, g.clone()));
                }
                out.extend(g.simplifications().into_iter().map(|g| E::Exists(*not, g)));
            }
        }
        out
    }
}
