// Compare fuel charged by wasmi 1.1 and wasmi 2.0 for a loop whose br_table
// dispatch has a large never-executed case.
fn wat(dead: usize) -> String {
    let body = |k: usize, n: usize| (0..n).map(|_| format!("(local.set 1 (i32.add (local.get 1) (i32.const {k})))")).collect::<Vec<_>>().join(" ");
    format!(r#"(module (func (export "run") (param i32) (result i32) (local i32)
      (block $exit (loop $L
        (br_if $exit (i32.eqz (local.get 0)))
        (local.set 0 (i32.sub (local.get 0) (i32.const 1)))
        (block $done (block $c2 (block $c1 (block $c0
              (br_table $c0 $c1 $c2 (i32.and (local.get 0) (i32.const 1))))
            {} (br $done))
            {} (br $done))
          {} (br $done))
        (br $L)))
      (local.get 1)))"#, body(1, 5), body(2, 5), body(3, dead))
}

fn run_v2(wat: &str, iters: i32) -> u64 {
    use wasmi2::*;
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wat.as_bytes()).unwrap();
    let mut store = Store::new(&engine, ());
    store.set_fuel(u64::MAX / 4).unwrap();
    let instance = Linker::<()>::new(&engine).instantiate_and_start(&mut store, &module).unwrap();
    let f = instance.get_typed_func::<i32, i32>(&store, "run").unwrap();
    f.call(&mut store, iters).unwrap();
    u64::MAX / 4 - store.get_fuel().unwrap()
}

fn run_v1(wat: &str, iters: i32) -> u64 {
    use wasmi1::*;
    let mut config = Config::default();
    config.consume_fuel(true);
    let engine = Engine::new(&config);
    let module = Module::new(&engine, wat.as_bytes()).unwrap();
    let mut store = Store::new(&engine, ());
    store.set_fuel(u64::MAX / 4).unwrap();
    let instance = Linker::<()>::new(&engine).instantiate_and_start(&mut store, &module).unwrap();
    let f = instance.get_typed_func::<i32, i32>(&store, "run").unwrap();
    f.call(&mut store, iters).unwrap();
    u64::MAX / 4 - store.get_fuel().unwrap()
}

fn main() {
    let iters = 1000;
    println!("{:>8} {:>16} {:>16}", "dead", "wasmi 1.1", "wasmi 2.0");
    for dead in [10usize, 1000, 100000] {
        let w = wat(dead);
        let v1 = run_v1(&w, iters) as f64 / iters as f64;
        let v2 = run_v2(&w, iters) as f64 / iters as f64;
        println!("{:>8} {:>16.1} {:>16.1}   fuel per iteration", dead, v1, v2);
    }
}
