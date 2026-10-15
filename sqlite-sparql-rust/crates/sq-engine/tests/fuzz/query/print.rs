//! SPARQL text of the query AST.
use std::fmt::{self, Write};

use super::*;

pub const PREFIXES: &str = "PREFIX : <http://ex/>\nPREFIX xsd: <http://www.w3.org/2001/XMLSchema#>\nPREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>\n";

impl fmt::Display for T {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            T::Var(v) => write!(f, "?{}", VARS[*v]),
            T::Iri(i) => write!(f, ":{i}"),
            T::Lit(l) => f.write_str(l),
            T::BNode(n) => write!(f, "_:k{n}"),
            T::Type => f.write_str("rdf:type"),
        }
    }
}

impl fmt::Display for Path {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Path::Pred(p) => write!(f, ":{p}"),
            Path::Inv(p) => write!(f, "^({p})"),
            Path::Seq(a, b) => write!(f, "({a}/{b})"),
            Path::Alt(a, b) => write!(f, "({a}|{b})"),
            Path::Star(p) => write!(f, "({p})*"),
            Path::Plus(p) => write!(f, "({p})+"),
            Path::Opt(p) => write!(f, "({p})?"),
            Path::Nps(set) => {
                f.write_str("!(")?;
                for (i, (inv, p)) in set.iter().enumerate() {
                    if i > 0 {
                        f.write_str("|")?;
                    }
                    write!(f, "{}:{p}", if *inv { "^" } else { "" })?;
                }
                f.write_str(")")
            }
        }
    }
}

impl fmt::Display for Agg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = |b: &bool| if *b { "DISTINCT " } else { "" };
        match self {
            Agg::CountAll(dist) => write!(f, "COUNT({}*)", d(dist)),
            Agg::Count(dist, v) => write!(f, "COUNT({}?{})", d(dist), VARS[*v]),
            Agg::Sum(dist, v) => write!(f, "SUM({}?{})", d(dist), VARS[*v]),
            Agg::Avg(dist, v) => write!(f, "AVG({}?{})", d(dist), VARS[*v]),
            Agg::Min(v) => write!(f, "MIN(?{})", VARS[*v]),
            Agg::Max(v) => write!(f, "MAX(?{})", VARS[*v]),
            Agg::Expr(func, dist, e) => write!(f, "{func}({}{e})", d(dist)),
        }
    }
}

impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            E::Var(v) => write!(f, "?{}", VARS[*v]),
            E::Const(c) => f.write_str(c),
            E::Bin(op, a, b) => write!(f, "({a} {op} {b})"),
            E::Not(a) => write!(f, "(!{a})"),
            E::Neg(a) => write!(f, "(-{a})"),
            E::Call(name, args) => {
                write!(f, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{a}")?;
                }
                f.write_str(")")
            }
            E::Bound(v) => write!(f, "BOUND(?{})", VARS[*v]),
            E::In(a, list, not) => {
                write!(f, "({a} {}IN (", if *not { "NOT " } else { "" })?;
                for (i, x) in list.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{x}")?;
                }
                f.write_str("))")
            }
            E::Exists(not, g) => {
                write!(f, "{}EXISTS ", if *not { "NOT " } else { "" })?;
                let mut s = String::new();
                g.print(&mut s, 0);
                f.write_str(s.trim_end())
            }
            E::Agg(a) => write!(f, "{a}"),
        }
    }
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

impl Group {
    pub fn print(&self, out: &mut String, depth: usize) {
        out.push_str("{\n");
        for el in &self.0 {
            indent(out, depth + 1);
            el.print(out, depth + 1);
            out.push('\n');
        }
        indent(out, depth);
        out.push('}');
    }
}

impl El {
    fn print(&self, out: &mut String, depth: usize) {
        match self {
            El::Triple(s, p, o) => {
                let _ = write!(out, "{s} {p} {o} .");
            }
            El::Path(s, p, o) => {
                let _ = write!(out, "{s} {p} {o} .");
            }
            El::Optional(g) => {
                out.push_str("OPTIONAL ");
                g.print(out, depth);
            }
            El::Union(a, b) => {
                a.print(out, depth);
                out.push_str(" UNION ");
                b.print(out, depth);
            }
            El::Minus(g) => {
                out.push_str("MINUS ");
                g.print(out, depth);
            }
            El::Filter(e) => {
                let _ = write!(out, "FILTER({e})");
            }
            El::Bind(e, v) => {
                let _ = write!(out, "BIND({e} AS ?{})", VARS[*v]);
            }
            El::Values(vars, rows) => {
                out.push_str("VALUES (");
                for (i, v) in vars.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    let _ = write!(out, "?{}", VARS[*v]);
                }
                out.push_str(") {");
                for row in rows {
                    out.push_str(" (");
                    for (i, t) in row.iter().enumerate() {
                        if i > 0 {
                            out.push(' ');
                        }
                        match t {
                            Some(t) => {
                                let _ = write!(out, "{t}");
                            }
                            None => out.push_str("UNDEF"),
                        }
                    }
                    out.push(')');
                }
                out.push_str(" }");
            }
            El::Graph(g, p) => {
                let _ = write!(out, "GRAPH {g} ");
                p.print(out, depth);
            }
            El::Sub(s) => {
                out.push_str("{ ");
                s.print(out, depth);
                out.push_str(" }");
            }
            El::Group(g) => g.print(out, depth),
        }
    }
}

impl Select {
    fn print(&self, out: &mut String, depth: usize) {
        out.push_str("SELECT ");
        match self.modifier {
            Modifier::None => {}
            Modifier::Distinct => out.push_str("DISTINCT "),
            Modifier::Reduced => out.push_str("REDUCED "),
        }
        match &self.items {
            None => out.push('*'),
            Some(items) => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    match item {
                        Item::Var(v) => {
                            let _ = write!(out, "?{}", VARS[*v]);
                        }
                        Item::Expr(e, v) => {
                            let _ = write!(out, "({e} AS ?{})", VARS[*v]);
                        }
                    }
                }
            }
        }
        out.push_str(" WHERE ");
        self.pattern.print(out, depth);
        self.print_modifiers(out);
    }

    fn print_modifiers(&self, out: &mut String) {
        if let Some(keys) = &self.group_by
            && !keys.is_empty()
        {
            out.push_str(" GROUP BY");
            for (k, e) in keys {
                match e {
                    None => {
                        let _ = write!(out, " ?{}", VARS[*k]);
                    }
                    Some(e) => {
                        let _ = write!(out, " ({e} AS ?{})", VARS[*k]);
                    }
                }
            }
        }
        if let Some(h) = &self.having {
            let _ = write!(out, " HAVING ({h})");
        }
        if !self.order.is_empty() {
            out.push_str(" ORDER BY");
            for (e, desc) in &self.order_exprs {
                if *desc {
                    let _ = write!(out, " DESC({e})");
                } else {
                    let _ = write!(out, " ASC({e})");
                }
            }
            for (v, desc) in &self.order {
                if *desc {
                    let _ = write!(out, " DESC(?{})", VARS[*v]);
                } else {
                    let _ = write!(out, " ?{}", VARS[*v]);
                }
            }
        }
        if let Some(l) = self.limit {
            let _ = write!(out, " LIMIT {l}");
        }
        if let Some(o) = self.offset {
            let _ = write!(out, " OFFSET {o}");
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut out = String::from(PREFIXES);
        let dataset = |out: &mut String| {
            for g in &self.from {
                let _ = writeln!(out, "FROM :{g}");
            }
            for g in &self.from_named {
                let _ = writeln!(out, "FROM NAMED :{g}");
            }
        };
        match &self.form {
            Form::Select(s) => {
                let mut head = String::new();
                s.print(&mut head, 0);
                // Put the dataset clause between the projection and WHERE.
                let at = head.find(" WHERE ").expect("printed SELECT has WHERE");
                out.push_str(&head[..at]);
                out.push('\n');
                dataset(&mut out);
                out.push_str(&head[at + 1..]);
            }
            Form::Ask(g) => {
                out.push_str("ASK\n");
                dataset(&mut out);
                out.push_str("WHERE ");
                g.print(&mut out, 0);
            }
            Form::Construct(template, g) => {
                out.push_str("CONSTRUCT {");
                for (s, p, o) in template {
                    let _ = write!(out, " {s} {p} {o} .");
                }
                out.push_str(" }\n");
                dataset(&mut out);
                out.push_str("WHERE ");
                g.print(&mut out, 0);
            }
            Form::Describe(ts, g) => {
                out.push_str("DESCRIBE");
                for t in ts {
                    let _ = write!(out, " {t}");
                }
                out.push('\n');
                dataset(&mut out);
                out.push_str("WHERE ");
                g.print(&mut out, 0);
            }
        }
        out.push('\n');
        f.write_str(&out)
    }
}
