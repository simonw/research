//! Adversarial review tests (WP5 review). Each `review_*` test pins a defect found in review.
mod common;

use common::*;
use sq_core::plan::{JoinPolicy, QueryOptions};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

fn triple(r: &mut Rng) -> String {
    let vars = ["?a", "?b", "?c", "?v", "?x"];
    let preds = [":knows", ":age", ":city", ":name", "a"];
    let p = r.pick(&preds);
    let s = r.pick(&vars);
    let o = match p {
        "a" => r.pick(&[":C1", ":C2", "?t"]).to_owned(),
        ":name" => r.pick(&["\"Person 1\"", "?n"]).to_owned(),
        ":city" => r.pick(&[":city1", "?c"]).to_owned(),
        _ => r.pick(&vars).to_owned(),
    };
    format!("{s} {p} {o} .")
}

fn group(r: &mut Rng, depth: u32) -> String {
    let mut out = String::new();
    let n = 1 + r.next() % 3;
    for _ in 0..n {
        out.push_str(&triple(r));
        out.push(' ');
    }
    if depth > 0 {
        match r.next() % 7 {
            0 => out.push_str(&format!("OPTIONAL {{ {} }} ", group(r, depth - 1))),
            1 => out.push_str(&format!("{{ {} }} UNION {{ {} }} ", group(r, depth - 1), group(r, depth - 1))),
            2 => out.push_str(&format!("{{ {} }} ", group(r, depth - 1))),
            3 => out.push_str(&format!("MINUS {{ {} }} ", group(r, depth - 1))),
            4 => out.push_str(&format!("{{ SELECT * {{ {} }} LIMIT 3 }} ", group(r, depth - 1))),
            5 => out.push_str(&format!("FILTER EXISTS {{ {} }} ", group(r, depth - 1))),
            _ => {}
        }
    }
    if r.next() % 3 == 0 {
        let a = r.pick(&["?a", "?b", "?v", "?x"]);
        let b = r.pick(&["?a", "?b", "?v", "?x", "1"]);
        out.push_str(&format!("FILTER({a} != {b}) "));
    }
    out
}

#[test]
fn review_probe_dump() {
    let st = social(200);
    let mut r = Rng(0x1234_5678_9abc_def1);
    let policies = [
        QueryOptions::default(),
        QueryOptions { join_policy: JoinPolicy::PreferBind, ..Default::default() },
        QueryOptions { join_policy: JoinPolicy::PreferHash, ..Default::default() },
        QueryOptions { reorder: false, ..Default::default() },
    ];
    let mut fails = std::collections::BTreeMap::<String, (usize, String)>::new();
    let mut total = 0;
    for _ in 0..3000 {
        let q = format!("SELECT * {{ {} }}", group(&mut r, 2));
        let Ok(parsed) = sq_plan::parse_query(&format!("{PREFIXES}{q}"), None) else { continue };
        for (pi, opts) in policies.iter().enumerate() {
            total += 1;
            let l = sq_plan::lower_query(&parsed, &st, opts).unwrap();
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sq_plan::optimize(l, &st, opts)));
            let msg = match res {
                Err(_) => Some("PANIC".to_owned()),
                Ok(Err(e)) => Some(format!("{e}")),
                Ok(Ok(p)) => sq_plan::validate(&p).err().map(|e| format!("{e}")),
            };
            if let Some(m) = msg {
                let key = m.split(": ").skip(2).collect::<Vec<_>>().join(": ");
                let e = fails.entry(key).or_insert((0, format!("[policy {pi}] {q}")));
                e.0 += 1;
                if q.len() < e.1.len() { e.1 = format!("[policy {pi}] {q}"); }
            }
        }
    }
    println!("total {total}");
    for (k, (n, ex)) in fails {
        println!("{n:5} {k}\n      e.g. {ex}");
    }
}
