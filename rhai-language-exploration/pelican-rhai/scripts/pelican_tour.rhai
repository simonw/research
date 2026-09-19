// ============================================================================
// THE GREAT PELICAN TOUR — a Rhai script that exercises as much of the
// language as possible, themed entirely around pelicans.
// Run via: cargo run --bin pelican_tour
// ============================================================================

print(">>> 1. Variables, constants, shadowing, scopes");

let colony_name = "Brackish Bay";          // mutable variable
const MAX_BEAK_LITRES = 13;                // constant (pelican pouch ~13 L!)
let fish_count = 0;
fish_count += 42;
let fish_count = fish_count * 2;           // shadowing is allowed
{
    let fish_count = 9999;                 // inner scope shadows outer
    print(`  inner scope fish_count = ${fish_count}`);
}
print(`  colony ${colony_name}: fish_count=${fish_count}, pouch max=${MAX_BEAK_LITRES}L`);

print(">>> 2. Numbers: ints, floats, hex/octal/binary, bitwise, ranges");

let brown_pelicans = 0b101010;             // binary literal = 42
let white_pelicans = 0x2A;                 // hex literal = 42
let total = brown_pelicans + white_pelicans;
let wingspan_m = 3.05;                     // Dalmatian pelican wingspan (float)
let dive_height = 20.0 ** 2 / 40.0;        // power operator
let flags = (1 << 3) | 0o7;                // bit shifts and octal
print(`  ${brown_pelicans} brown + ${white_pelicans} white = ${total}; wingspan ${wingspan_m}m; dive ${dive_height}; flags ${flags}`);
print(`  int division 7/2=${7/2}, modulo 7%2=${7 % 2}, float 7.0/2=${7.0/2}`);

let migration_range = 0..5;                // exclusive range
let nesting_range = 1..=3;                 // inclusive range
print(`  2 in 0..5? ${2 in migration_range}; 3 in 1..=3? ${3 in nesting_range}`);

print(">>> 3. Strings and chars");

let cry = "SQUAWK";
let softer = cry.to_lower();
let pouch = cry.sub_string(0, 2) + "!";
let letters = cry.split("A");             // -> ["SQU", "WK"]
let first_char = cry[0];                  // char indexing
let banner = `
  Multi-line interpolated string:
  The pelican goes ${cry}!!!
`;
print(banner);
print(`  lower=${softer}, clipped=${pouch}, split=${letters}, first char=${first_char}`);
print(`  contains 'QUA'? ${cry.contains("QUA")}, len=${cry.len}, reversed...`);
let rev = "";
for ch in cry.split("") { rev = ch + rev; }
print(`  reversed = ${rev}`);

print(">>> 4. Arrays and functional methods");

let flock = ["Percy", "Petra", "Pip", "Gulliver", "Beaky"];
flock.push("Scoop");
flock += ["Dip"];                          // concat
let name_lengths = flock.map(|n| n.len);
let long_names = flock.filter(|n| n.len > 4);
let total_letters = flock.reduce(|sum, n| sum + n.len, 0);
let sorted = flock.map(|n| n);             // copy...
sorted.sort();                             // ...then sort in place (sort returns unit)
print(`  flock (${flock.len}): ${flock}`);
print(`  name lengths: ${name_lengths}, long names: ${long_names}, total letters: ${total_letters}`);
print(`  sorted: ${sorted}`);
print(`  "Pip" in flock? ${"Pip" in flock}; some len>6? ${flock.some(|n| n.len > 6)}; all len>2? ${flock.all(|n| n.len > 2)}`);
let popped = flock.pop();
print(`  popped ${popped}, flock now ${flock.len} strong`);
print(`  slice flock[1..3] = ${flock.extract(1..3)}`);

print(">>> 5. Object maps (and nesting)");

let nest = #{
    location: "Mangrove Island",
    height_m: 1.5,
    eggs: 3,
    parents: ["Percy", "Petra"],
    materials: #{ sticks: 200, seaweed: true },
};
nest.eggs += 1;                            // dot access
nest["guard_on_duty"] = "Percy";           // index access, add new key
print(`  nest at ${nest.location}: ${nest.eggs} eggs, sticks=${nest.materials.sticks}`);
let nest_keys = nest.keys();
nest_keys.sort();
print(`  keys: ${nest_keys}`);
print(`  has 'eggs'? ${"eggs" in nest}`);
nest.remove("height_m");
print(`  after remove: ${nest.keys().len} keys`);

print(">>> 6. Control flow: if / switch / while / do / loop / for");

let fish_today = 17;
let mood = if fish_today > 15 { "delighted" } else if fish_today > 5 { "content" } else { "hangry" };
print(`  ${fish_today} fish -> pelican is ${mood}`);

let species = "Dalmatian";
let fact = switch species {
    "Brown" => "the only pelican that plunge-dives from the air",
    "Dalmatian" | "Great White" => "one of the heaviest flying birds",
    _ => "a magnificent pouch-carrier",
};
print(`  ${species} pelican: ${fact}`);

let catch_size = 17;
let verdict = switch catch_size {
    0 => "went hungry",
    1..=5 => "a light lunch",
    6..=15 => "a solid haul",
    16..=1000 if catch_size % 2 == 1 => "an odd-numbered feast",  // range case with guard
    _ => "impossible",
};
print(`  catch of ${catch_size}: ${verdict}`);

let altitude = 0;
while altitude < 30 { altitude += 10; }
print(`  climbed to ${altitude}m by while-loop`);

let glides = 0;
do { glides += 1; } while glides < 3;
print(`  glided ${glides} times (do-while)`);

let dives = 0;
let caught = loop {
    dives += 1;
    if dives * 3 > 10 { break dives * 3; }  // loop can break with a value
};
print(`  after ${dives} dives, caught ${caught} fish (loop+break value)`);

for i in range(0, 10, 2) { if i == 6 { continue; } print(`    even glide step ${i}`); }
for (name, idx) in ["Percy", "Petra", "Pip"] {
    print(`    roll-call #${idx}: ${name}`);
}

print(">>> 7. Functions, recursion, function pointers, closures, currying");

fn pouch_volume(beak_len, depth) { beak_len * depth * 0.5 }

fn fish_stack(n) {                         // recursion
    if n <= 1 { 1 } else { n * fish_stack(n - 1) }
}

fn describe() { `a pelican with a ${this.wingspan}m wingspan` }  // method-style via `this`

print(`  pouch_volume(0.4, 0.3) = ${pouch_volume(0.4, 0.3)}`);
print(`  fish_stack(6) = ${fish_stack(6)} ways to stack 6 fish`);

let fp = Fn("pouch_volume");               // function pointer by name
print(`  via Fn pointer: ${fp.call(0.5, 0.2)}`);

let scale = 3;
let scaled_catch = |fish| fish * scale;    // closure capturing `scale`
print(`  closure scaled_catch(7) = ${scaled_catch.call(7)}`);

let add = |a, b| a + b;
let add_ten = add.curry(10);               // currying
print(`  curried add_ten(5) = ${add_ten.call(5)}`);

let tally = 0;
let record_catch = |n| { tally += n; tally };  // closure mutating captured var
record_catch.call(4);
record_catch.call(6);
print(`  shared capture tally = ${tally}`);

print(">>> 8. OOP-style: object map + method closures + `this`");

let percy = #{
    name: "Percy",
    wingspan: 2.5,
    fish: 0,
    gulp: |n| { this.fish += n; },
    brag: || `${this.name} has eaten ${this.fish} fish today`,
};
percy.gulp(3);        // method-style call binds `this` to the map
percy.gulp(2);
print(`  ${percy.brag()}`);
let bound = #{ wingspan: 3.05 };
let d = Fn("describe");
print(`  Fn("describe") bound to a map: ${bound.call(d)}`);

print(">>> 9. Error handling: throw / try-catch");

fn risky_dive(depth) {
    if depth > 3.0 {
        throw #{ error: "TooDeep", depth: depth, advice: "pelicans only plunge ~3m" };
    }
    `caught a mullet at ${depth}m`
}

try {
    print(`  ${risky_dive(1.5)}`);
    print(`  ${risky_dive(9.0)}`);
} catch (err) {
    print(`  CAUGHT: ${err.error} at ${err.depth}m — ${err.advice}`);
}

try { let x = 1/0; } catch (e) { print(`  runtime error caught too: ${e}`); }

print(">>> 10. Type inspection & conversion");

print(`  type_of(3.05) = ${type_of(3.05)}, type_of("squawk") = ${type_of("squawk")}`);
print(`  type_of(#{{}}) = ${type_of(#{})}, type_of([]) = ${type_of([])}`);
print(`  "42".parse_int() + 1 = ${"42".parse_int() + 1}, 3.99.to_int()=${3.99.to_int()}, 42.to_float()/5=${42.to_float()/5}`);
print(`  to_string: ${42.to_string() + "!"}`);

print(">>> 11. Timestamps");

let t = timestamp();
let acc = 0;
for i in 0..50_000 { acc += i; }
print(`  counted ${acc} fish-scales in ${t.elapsed * 1000.0} ms`);

print(">>> 12. Modules (registered from Rust) — import");

import "colony" as colony;
print(`  motto: ${colony::MOTTO}`);
print(`  colony::headcount(3, 4) = ${colony::headcount(3, 4)}`);
print(`  colony::ration(10) = ${colony::ration(10)}`);

print(">>> 13. Custom Rust type: Pelican struct");

let big_bird = new_pelican("Gulliver", 2.8);
big_bird.feed(5);                          // Rust method
big_bird.feed(3);
big_bird.wingspan = 3.1;                   // Rust setter
print(`  ${big_bird.squawk()}`);           // Rust method returning String
print(`  ${big_bird.name} wingspan=${big_bird.wingspan} fish_eaten=${big_bird.fish_eaten}`);
print(`  as string: ${big_bird}`);         // uses registered to_string

let rival = new_pelican("Beaky", 2.2);
rival.feed(4);
// custom operator registered from Rust:
print(`  Gulliver outfishes Beaky? ${big_bird outfishes rival}`);

print(">>> 14. eval — scripts running scripts");

let formula = "MAX_BEAK_LITRES * 2 + 1";
print(`  eval("${formula}") = ${eval(formula)}`);

// A value for the host (Rust) to pick up as the script's return value:
#{
    colony: colony_name,
    flock_size: flock.len,
    champion: big_bird.name,
    champion_fish: big_bird.fish_eaten,
    mood: mood,
}
