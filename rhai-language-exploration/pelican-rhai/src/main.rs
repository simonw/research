//! Host program for the Great Pelican Tour.
//!
//! Demonstrates the Rust-side embedding API of Rhai:
//! - registering a custom Rust type (`Pelican`) with methods, getters/setters
//! - registering plain Rust functions
//! - a custom operator (`outfishes`)
//! - a module (`colony`) defined in Rhai and made importable
//! - capturing `print` output
//! - extracting a typed result from the script and calling script functions
//!   back from Rust (`call_fn`)

use rhai::{CallFnOptions, Dynamic, Engine, Map, Module, Scope, AST};

#[derive(Debug, Clone)]
struct Pelican {
    name: String,
    wingspan: f64,
    fish_eaten: i64,
}

impl Pelican {
    fn new(name: &str, wingspan: f64) -> Self {
        Self {
            name: name.into(),
            wingspan,
            fish_eaten: 0,
        }
    }
    fn feed(&mut self, fish: i64) {
        self.fish_eaten += fish;
    }
    fn squawk(&mut self) -> String {
        format!(
            "SQUAWK! I am {}, wingspan {:.2}m, {} fish down the pouch!",
            self.name, self.wingspan, self.fish_eaten
        )
    }
}

fn build_engine() -> Engine {
    let mut engine = Engine::new();

    // --- Custom type ------------------------------------------------------
    engine
        .register_type_with_name::<Pelican>("Pelican")
        .register_fn("new_pelican", |name: &str, wingspan: f64| {
            Pelican::new(name, wingspan)
        })
        .register_fn("feed", Pelican::feed)
        .register_fn("squawk", Pelican::squawk)
        .register_get("name", |p: &mut Pelican| p.name.clone())
        .register_get_set(
            "wingspan",
            |p: &mut Pelican| p.wingspan,
            |p: &mut Pelican, v: f64| p.wingspan = v,
        )
        .register_get("fish_eaten", |p: &mut Pelican| p.fish_eaten)
        .register_fn("to_string", |p: &mut Pelican| {
            format!("Pelican({}, {:.2}m, {} fish)", p.name, p.wingspan, p.fish_eaten)
        });

    // --- Custom operator: `a outfishes b` ---------------------------------
    engine
        .register_custom_operator("outfishes", 160)
        .expect("valid operator");
    engine.register_fn("outfishes", |a: &mut Pelican, b: Pelican| {
        a.fish_eaten > b.fish_eaten
    });

    // --- A module written in Rhai, importable as `colony` -----------------
    let module_ast = engine
        .compile(
            r#"
            export const MOTTO = "A wonderful bird is the pelican";
            fn headcount(browns, whites) { browns + whites }
            fn ration(fish) { fish / 2 }
            "#,
        )
        .expect("colony module compiles");
    let colony =
        Module::eval_ast_as_new(Scope::new(), &module_ast, &engine).expect("colony module evals");
    let mut resolver = rhai::module_resolvers::StaticModuleResolver::new();
    resolver.insert("colony", colony);
    engine.set_module_resolver(resolver);

    // --- Route script `print`/`debug` through the host --------------------
    engine.on_print(|msg| println!("{msg}"));
    engine.on_debug(|msg, src, pos| println!("DEBUG {src:?} @ {pos}: {msg}"));

    engine
}

fn main() -> Result<(), Box<rhai::EvalAltResult>> {
    let engine = build_engine();

    let script = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/pelican_tour.rhai"),
    )
    .expect("script file readable");

    let ast: AST = engine.compile(&script)?;
    let mut scope = Scope::new();
    let result: Map = engine.eval_ast_with_scope(&mut scope, &ast)?;

    println!("\n>>> 15. Back in Rust: typed result extracted from the script");
    println!("  script returned an object map with {} keys:", result.len());
    let champion = result["champion"].clone().into_string().unwrap();
    let champion_fish = result["champion_fish"].as_int().unwrap();
    println!("  champion = {champion} ({champion_fish} fish), mood = {}", result["mood"]);

    println!("\n>>> 16. Calling a script function from Rust (call_fn)");
    // eval_ast(false): don't re-run the whole script body before the call.
    let opts = CallFnOptions::new().eval_ast(false).rewind_scope(true);
    let vol: f64 =
        engine.call_fn_with_options(opts, &mut scope, &ast, "pouch_volume", (0.45_f64, 0.25_f64))?;
    println!("  pouch_volume(0.45, 0.25) called from Rust = {vol}");

    let opts = CallFnOptions::new().eval_ast(false).rewind_scope(true);
    let mut this = Dynamic::from(Map::from_iter([("wingspan".into(), Dynamic::from(2.9_f64))]));
    let desc: String =
        engine.call_fn_with_options(opts.bind_this_ptr(&mut this), &mut scope, &ast, "describe", ())?;
    println!("  describe() with bound `this` from Rust = {desc}");

    println!("\nTour complete. SQUAWK!");
    Ok(())
}
