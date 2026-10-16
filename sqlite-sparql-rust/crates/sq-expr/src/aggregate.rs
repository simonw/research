//! Aggregate accumulators (§6.7).
//!
//! The host evaluates the aggregate argument per row (with `canonical = true`, so DISTINCT can
//! dedupe by id) and calls `add(Some(id))`, or `add(None)` when the argument is unbound or an
//! error. COUNT(*) rows are added with `add_row(row, visible)` (or `add(None)`).
//!
//! * `COUNT(expr)` counts bound, non-error values; `COUNT(*)` counts rows;
//!   `COUNT(DISTINCT *)` counts distinct solutions, i.e. rows projected onto the group input's
//!   user-visible in-scope variables (hidden slots such as blank-node pattern variables and
//!   generated path middles are not part of a SPARQL solution).
//! * SUM / AVG / MIN / MAX / GROUP_CONCAT: any error makes that aggregate unbound; the group
//!   row is still produced (agg-err-01).
//! * SUM follows numeric promotion; AVG = SUM / COUNT, a decimal division for integers and
//!   decimals and a double division when any float or double was added; empty → 0.
//! * MIN / MAX use the ORDER BY key order and never error on mixed types; empty → unbound.
//! * SAMPLE returns the first non-error value.
//! * GROUP_CONCAT joins STR() values with the separator and returns a simple literal.
//! * DISTINCT dedupes by (canonical) id. A custom aggregate is always unbound.
use oxsdatatypes::{Decimal, Double};
use rustc_hash::FxHashSet;
use sq_core::ir::{AggFunc, AggSpec};
use sq_core::{Result, Slot, TermId};

use crate::EvalCtx;
use crate::numeric::{self, ArithOp, Num};
use crate::sortkey::{SortKey, sort_key};
use crate::value::{Val, Value, id_of_value, value_of};

pub trait Accumulator {
    /// One evaluated argument: `None` = unbound or error.
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()>;
    /// The aggregate value (`None` = unbound). Computed results get canonical ids.
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>>;
    /// One input row of `COUNT(*)` / `COUNT(DISTINCT *)`, in the global slot layout.
    /// `visible` lists the slots of the group input's user-visible in-scope variables (what
    /// `SELECT *` would project there: no blank-node pattern variables, spargebra-generated
    /// path middles or out-of-scope renamed slots); `COUNT(DISTINCT *)` dedupes the row
    /// projected onto them. Other aggregates ignore the row and count it as `add(None)`.
    fn add_row(&mut self, row: &[TermId], visible: &[Slot], cx: &mut EvalCtx) -> Result<()> {
        let _ = (row, visible);
        self.add(None, cx)
    }
}

pub fn accumulator(spec: &AggSpec) -> Box<dyn Accumulator> {
    let inner: Box<dyn Accumulator> = match &spec.func {
        AggFunc::CountAll => {
            return Box::new(CountAll { count: 0, seen: spec.distinct.then(FxHashSet::default) });
        }
        AggFunc::Count => Box::new(Count { count: 0 }),
        AggFunc::Sum => Box::new(Sum { sum: Some(Num::Integer(0)) }),
        AggFunc::Avg => Box::new(Avg { sum: Some(Num::Integer(0)), count: 0 }),
        AggFunc::Min => Box::new(Extreme { best: None, error: false, want: std::cmp::Ordering::Less }),
        AggFunc::Max => Box::new(Extreme { best: None, error: false, want: std::cmp::Ordering::Greater }),
        AggFunc::Sample => Box::new(Sample { value: None }),
        AggFunc::GroupConcat { separator } => {
            Box::new(GroupConcat { out: Some(String::new()), separator: separator.clone(), first: true })
        }
        AggFunc::Custom(_) => Box::new(Unbound),
    };
    if spec.distinct { Box::new(Distinct { seen: FxHashSet::default(), inner }) } else { inner }
}

/// DISTINCT wrapper: each id is passed to the inner accumulator once; errors pass through.
struct Distinct {
    seen: FxHashSet<TermId>,
    inner: Box<dyn Accumulator>,
}

impl Accumulator for Distinct {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()> {
        match v {
            Some(id) if !self.seen.insert(id) => Ok(()),
            v => self.inner.add(v, cx),
        }
    }
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>> {
        self.inner.finish(cx)
    }
}

fn integer_id(n: u64) -> TermId {
    TermId::from_int(n.min(TermId::INT_MAX as u64) as i64).expect("in range")
}

struct CountAll {
    count: u64,
    /// COUNT(DISTINCT *): the distinct solutions seen (visible slots only).
    seen: Option<FxHashSet<Box<[TermId]>>>,
}

impl Accumulator for CountAll {
    fn add(&mut self, _v: Option<TermId>, _cx: &mut EvalCtx) -> Result<()> {
        self.count += 1;
        Ok(())
    }
    fn add_row(&mut self, row: &[TermId], visible: &[Slot], cx: &mut EvalCtx) -> Result<()> {
        match &mut self.seen {
            Some(seen) => {
                let solution: Box<[TermId]> =
                    visible.iter().map(|s| row.get(*s as usize).copied().unwrap_or(TermId::UNBOUND)).collect();
                if seen.insert(solution) {
                    self.count += 1;
                }
                Ok(())
            }
            None => self.add(None, cx),
        }
    }
    fn finish(&mut self, _cx: &mut EvalCtx) -> Result<Option<TermId>> {
        Ok(Some(integer_id(self.count)))
    }
}

struct Count {
    count: u64,
}

impl Accumulator for Count {
    fn add(&mut self, v: Option<TermId>, _cx: &mut EvalCtx) -> Result<()> {
        if v.is_some_and(crate::value::is_present) {
            self.count += 1;
        }
        Ok(())
    }
    fn finish(&mut self, _cx: &mut EvalCtx) -> Result<Option<TermId>> {
        Ok(Some(integer_id(self.count)))
    }
}

/// Numeric value of an aggregate argument; None for non-numeric (an error).
fn numeric_arg(v: Option<TermId>, cx: &mut EvalCtx) -> Result<Option<Num>> {
    let Some(id) = v.filter(|id| crate::value::is_present(*id)) else { return Ok(None) };
    if let Some(i) = id.as_int() {
        return Ok(Some(Num::Integer(i)));
    }
    Ok(value_of(id, cx)?.as_num())
}

struct Sum {
    /// None once an error was seen.
    sum: Option<Num>,
}

impl Accumulator for Sum {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()> {
        if let Some(sum) = self.sum {
            self.sum = match numeric_arg(v, cx)? {
                Some(x) => numeric::arith(ArithOp::Add, sum, x),
                None => None,
            };
        }
        Ok(())
    }
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>> {
        match self.sum {
            Some(n) => id_of_value(&n.into(), cx, true).map(Some),
            None => Ok(None),
        }
    }
}

struct Avg {
    sum: Option<Num>,
    count: u64,
}

impl Accumulator for Avg {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()> {
        if let Some(sum) = self.sum {
            self.sum = match numeric_arg(v, cx)? {
                Some(x) => numeric::arith(ArithOp::Add, sum, x),
                None => None,
            };
            self.count += 1;
        }
        Ok(())
    }
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>> {
        let Some(sum) = self.sum else { return Ok(None) };
        if self.count == 0 {
            return Ok(Some(TermId::from_int(0).expect("0 is inline")));
        }
        let avg = match sum {
            Num::Integer(_) | Num::Decimal(_) => {
                let sum = sum.to_decimal().expect("integer or decimal");
                match numeric::dec_div(sum, Decimal::from(self.count)) {
                    Some(d) => Value::Decimal(d),
                    None => return Ok(None),
                }
            }
            Num::Float(_) | Num::Double(_) => Value::Double(Double::from(sum.to_f64() / self.count as f64)),
        };
        id_of_value(&avg, cx, true).map(Some)
    }
}

/// MIN (`want = Less`) or MAX (`want = Greater`) under the ORDER BY key order.
struct Extreme {
    /// Best id so far and its key (computed lazily: inline integers compare by payload).
    best: Option<(TermId, Option<SortKey>)>,
    error: bool,
    want: std::cmp::Ordering,
}

impl Accumulator for Extreme {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()> {
        if self.error {
            return Ok(());
        }
        let Some(id) = v.filter(|id| crate::value::is_present(*id)) else {
            self.error = true;
            self.best = None;
            return Ok(());
        };
        let Some((best, best_key)) = &mut self.best else {
            self.best = Some((id, None));
            return Ok(());
        };
        if let (Some(a), Some(b)) = (id.as_int(), best.as_int()) {
            if a.cmp(&b) == self.want {
                *best = id;
                *best_key = None;
            }
            return Ok(());
        }
        // Two simple literals order by their UTF-8 bytes (the key's escaping preserves it).
        if is_simple_literal(id) && is_simple_literal(*best) {
            let (Value::Str(a), Value::Str(b)) = (value_of(id, cx)?, value_of(*best, cx)?) else {
                unreachable!("simple-literal ids decode to strings")
            };
            if a.as_bytes().cmp(b.as_bytes()) == self.want {
                *best = id;
                *best_key = None;
            }
            return Ok(());
        }
        let key = sort_key(id, cx)?;
        let current = match best_key {
            Some(k) => k,
            None => best_key.insert(sort_key(*best, cx)?),
        };
        if key.cmp(current) == self.want {
            *best = id;
            *best_key = Some(key);
        }
        Ok(())
    }
    fn finish(&mut self, _cx: &mut EvalCtx) -> Result<Option<TermId>> {
        Ok(if self.error { None } else { self.best.as_ref().map(|(id, _)| *id) })
    }
}

/// Canonical simple-literal id (SHORTSTR or dictionary string).
fn is_simple_literal(id: TermId) -> bool {
    matches!(id.tag(), sq_core::Tag::ShortStr | sq_core::Tag::Str)
}

struct Sample {
    value: Option<TermId>,
}

impl Accumulator for Sample {
    fn add(&mut self, v: Option<TermId>, _cx: &mut EvalCtx) -> Result<()> {
        if self.value.is_none() {
            self.value = v.filter(|id| crate::value::is_present(*id));
        }
        Ok(())
    }
    fn finish(&mut self, _cx: &mut EvalCtx) -> Result<Option<TermId>> {
        Ok(self.value)
    }
}

struct GroupConcat {
    /// None once an error was seen.
    out: Option<String>,
    separator: String,
    first: bool,
}

impl Accumulator for GroupConcat {
    fn add(&mut self, v: Option<TermId>, cx: &mut EvalCtx) -> Result<()> {
        let Some(out) = &mut self.out else { return Ok(()) };
        let Some(id) = v.filter(|id| crate::value::is_present(*id)) else {
            self.out = None;
            return Ok(());
        };
        // STR(): stored lexical forms; blank nodes are errors.
        let s = match crate::eval::str_of(Val::Id(id), cx)? {
            Some(Val::Id(sid)) => match value_of(sid, cx)? {
                Value::Str(s) => s,
                _ => unreachable!("STR passes only simple literals through"),
            },
            Some(Val::V(Value::Str(s))) => s,
            _ => {
                self.out = None;
                return Ok(());
            }
        };
        if !self.first {
            out.push_str(&self.separator);
        }
        self.first = false;
        out.push_str(&s);
        Ok(())
    }
    fn finish(&mut self, cx: &mut EvalCtx) -> Result<Option<TermId>> {
        match self.out.take() {
            Some(s) => id_of_value(&Value::Str(s.into()), cx, true).map(Some),
            None => Ok(None),
        }
    }
}

/// Unknown custom aggregate: always unbound.
struct Unbound;

impl Accumulator for Unbound {
    fn add(&mut self, _v: Option<TermId>, _cx: &mut EvalCtx) -> Result<()> {
        Ok(())
    }
    fn finish(&mut self, _cx: &mut EvalCtx) -> Result<Option<TermId>> {
        Ok(None)
    }
}
