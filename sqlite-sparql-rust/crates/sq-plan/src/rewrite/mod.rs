//! Logical rewrites run before physical planning (spec §7.3): `GRAPH <c>` resolution, the
//! GRAPH ?g push-or-loop decision, filter placement and sameTerm seeds. TopK, Count and the
//! canonical flags are physical decisions (see `physical`).
mod filter;
mod graph;

use sq_core::ir::Logical;
use sq_core::{Result, TermId};

use crate::props::LogicalProps;

/// The rewritten query body: root pattern and EXISTS patterns (same ids as before).
pub(crate) struct Rewritten {
    pub root: Logical,
    pub exists: Vec<Logical>,
}

/// Apply all logical rewrites. `named` is the dataset's sorted named-graph list. Debug builds
/// check that filter placement moved no conjunct into an OPTIONAL / MINUS right side.
pub(crate) fn rewrite(root: Logical, mut exists: Vec<Logical>, named: &[TermId]) -> Result<Rewritten> {
    // 1. GRAPH <c>: membership check + Active -> Const(c) (EXISTS patterns included).
    let mut root = graph::resolve_const_graphs(root, &mut exists, named);
    for i in 0..exists.len() {
        let pat = std::mem::replace(&mut exists[i], Logical::Unit);
        exists[i] = graph::resolve_const_graphs(pat, &mut exists, named);
    }

    // 2. GRAPH ?g: push into the patterns or keep for a GraphLoop.
    let snapshot = exists.clone();
    {
        let lp = LogicalProps::new(&snapshot);
        root = graph::push_graph_vars(root, &mut exists, &lp);
        for i in 0..exists.len() {
            let pat = std::mem::replace(&mut exists[i], Logical::Unit);
            exists[i] = graph::push_graph_vars(pat, &mut exists, &lp);
        }
    }

    // 3. Filter placement and seed constants. Slots of EXISTS patterns do not change.
    let snapshot = exists.clone();
    let lp = LogicalProps::new(&snapshot);
    let placer = filter::FilterPlacer::new(&lp);
    let place = |l: Logical| -> Result<Logical> {
        if !cfg!(debug_assertions) {
            return Ok(placer.place(l));
        }
        let placed = placer.place(l.clone());
        filter::check_barriers(&l, &placed)?;
        Ok(placed)
    };
    let root = place(root)?;
    let exists = exists.into_iter().map(place).collect::<Result<_>>()?;
    Ok(Rewritten { root, exists })
}
