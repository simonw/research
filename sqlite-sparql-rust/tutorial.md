# sparqlite tutorial: SPARQL inside SQLite

sparqlite is a SQLite extension, written in Rust, that stores RDF data in ordinary SQLite tables
and answers SPARQL 1.1 queries and updates with its own query engine. You call it from SQL:

<!-- verify: skip -->
```sql
SELECT sparql_query('SELECT ?name WHERE { ?language <http://www.w3.org/2000/01/rdf-schema#label> ?name }');
```

This tutorial is for people who know a little SQL and nothing about RDF or SPARQL. It takes you
from building the extension, through a short introduction to RDF, to a lesson-by-lesson SPARQL
course on a real dataset, and then shows how to mix SPARQL results with ordinary SQL tables.

**How to read the examples.** Run every command from the project folder, `sqlite-sparql-rust/`
(section 1.2 shows how to get it). A block marked `sql` is typed into the `sqlite3` shell; the
block after it shows what the shell
printed. All outputs were copied from real runs (SQLite 3.45.1 on Linux) and are re-checked by
[`tutorial/verify_tutorial.py`](tutorial/verify_tutorial.py), which replays the whole tutorial in a
scratch folder: `python3 tutorial/verify_tutorial.py --ext ext/target/release/libsparqlite.so`
(it needs bash, and takes under a minute, most of it for the larger dataset of section 7).
Timings depend on your machine, so yours will differ.

Contents

1. [Install](#1-install)
2. [First steps in the sqlite3 shell](#2-first-steps-in-the-sqlite3-shell)
3. [RDF in five minutes](#3-rdf-in-five-minutes)
4. [The tutorial dataset](#4-the-tutorial-dataset)
5. [A SPARQL course](#5-a-sparql-course)
6. [Using sparqlite from SQL](#6-using-sparqlite-from-sql)
7. [Performance](#7-performance)
8. [Reference and troubleshooting](#8-reference-and-troubleshooting)

---

## 1. Install

### 1.1 What you need

- **Rust**, installed with [rustup](https://rustup.rs). The project pins its compiler version in
  `rust-toolchain.toml` (Rust 1.97); rustup downloads that version by itself the first time you
  build. Rust also needs the platform's linker: `build-essential` on Debian/Ubuntu, the Xcode
  Command Line Tools on macOS (`xcode-select --install`), the Visual Studio Build Tools on Windows.
- **A `sqlite3` shell that can load extensions**, SQLite **3.34.1 or newer**.
  - Debian/Ubuntu: `sudo apt install sqlite3`.
  - macOS: the built-in `/usr/bin/sqlite3` cannot load extensions (Apple builds it without that
    feature). Install Homebrew's: `brew install sqlite`. Homebrew does not put it on your `PATH`
    (it would shadow Apple's), so run it as `$(brew --prefix sqlite)/bin/sqlite3`, or add that
    directory to your `PATH`.
  - Windows: the "sqlite-tools" zip from <https://sqlite.org/download.html>.
- **Python 3.8 or newer** for the script that generates the tutorial data. It uses only the
  standard library.

Why 3.34.1? The extension is compiled against SQLite 3.34.1's extension interface, and it uses
`sqlite3_txn_state()`, which first appeared in SQLite 3.34.0. When it is loaded into an older
SQLite library it refuses with an error of the form `SQLite version mismatch: 3031001 < 3034001`
(the running version, then the version it needs). Everything in this tutorial was tested with
SQLite 3.45.1.

Check your shell's version with `sqlite3 --version`.

### 1.2 Get the code and build the extension

sparqlite is the `sqlite-sparql-rust` folder of the GitHub repository
[simonw/research](https://github.com/simonw/research). Clone the repository (`--depth 1` skips
its history) and change into that folder, the *project folder* where every later command runs:

```sh
git clone --depth 1 https://github.com/simonw/research
cd research/sqlite-sparql-rust
```

Then build the extension:

```sh
cargo build --release --manifest-path ext/Cargo.toml
```

The first build downloads the dependencies and takes a few minutes (3.5 minutes on the 4-core
machine used for this tutorial; the release profile uses whole-program optimisation). The result
is a single file:

| System  | File                                    |
|---------|-----------------------------------------|
| Linux   | `ext/target/release/libsparqlite.so`    |
| macOS   | `ext/target/release/libsparqlite.dylib` |
| Windows | `ext\target\release\sparqlite.dll`      |

The rest of the tutorial uses these paths. If you have set `CARGO_TARGET_DIR` (or
`build.target-dir` in Cargo's configuration), Cargo writes the file to `release/` under that
directory instead; either use that path wherever the tutorial writes `ext/target/release/`, or
build with the setting switched off (`env -u CARGO_TARGET_DIR cargo build ...`).

(`ext/` is a separate Cargo workspace on purpose: it builds the code as a loadable extension,
which needs different settings from the test builds in the main workspace.)

### 1.3 Load it and check that it works

Start the shell. With no file name it opens a temporary in-memory database:

<!-- verify: session -->
```sh
sqlite3
```

Then load the extension and ask for its version:

```sql
.load ext/target/release/libsparqlite
SELECT sparql_version();
```

```output
sparqlite 0.1.0 (SQLite 3.45.1)
```

`.load` takes a path. You can leave out the `.so` / `.dylib` / `.dll` suffix; the shell tries
the name as given and then with the platform's suffix. The shell also works out the name of the
extension's entry point from the file name (`libsparqlite` → `sqlite3_sparqlite_init`). If you
copy the file under another name, give the entry point yourself:
`.load /some/where/my-copy.so sqlite3_sparqlite_init`.

**On Windows** the file has no `lib` prefix (`sparqlite.dll`). Wherever this tutorial writes
`ext/target/release/libsparqlite`, in `.load`, in Python and in the `--ext` options of section
4, write `ext/target/release/sparqlite` instead; the shell adds `.dll` and derives the same entry
point, `sqlite3_sparqlite_init`. Write `py` where the tutorial writes `python3`. The tutorial was
run on Linux only; section 4.1 gives a PowerShell version of the one command that uses Unix shell
syntax.

If `.load` fails, see [Troubleshooting](#83-troubleshooting).

**From Python**, enable extension loading on the connection, load, and switch it off again:

<!-- verify: python -->
<!-- verify: mask \(SQLite [0-9.]+\) -->
```python
import sqlite3

con = sqlite3.connect(":memory:")
con.enable_load_extension(True)
con.load_extension("ext/target/release/libsparqlite")
con.enable_load_extension(False)
print(con.execute("SELECT sparql_version()").fetchone()[0])
```

```output
sparqlite 0.1.0 (SQLite 3.45.1)
```

Python 3.12 and newer also accept `con.load_extension(path, entrypoint="sqlite3_sparqlite_init")`
for a renamed file. Some Python builds cannot load extensions at all (macOS's system Python is
one); `enable_load_extension` then fails with an `AttributeError`.

**Other programs.** Anything that can load SQLite extensions can use sparqlite. The extension
registers its functions on one connection at a time, so every connection must load it. Two tools
that were tried with this build: `sqlite-utils query tutorial/tutorial.db "SELECT ..."
--load-extension ext/target/release/libsparqlite.so` and
`datasette tutorial/tutorial.db --load-extension ext/target/release/libsparqlite.so`.

Leave the shell with `.quit` (or Ctrl-D).

---

## 2. First steps in the sqlite3 shell

This section uses a throw-away in-memory database. Start a new shell and load the extension:

<!-- verify: session -->
```sh
sqlite3
```

```sql
.load ext/target/release/libsparqlite
```

### 2.1 Create the store and load a few triples

`sparql_init()` creates the tables that hold RDF data. It returns the storage schema version and
is safe to call again. (Loading data creates the tables automatically too, so you rarely need
it; it is shown here so you can see what it creates.)

```sql
SELECT sparql_init();
.tables
```

```output
1
rdf_graph        rdf_quad         rdf_stat_global  rdf_stat_pred
rdf_meta         rdf_stat_cs      rdf_stat_po      rdf_term
```

Now load some data. RDF data comes as *triples*: subject, predicate, object. Section 3 explains
the format; for now, read the text below as "alice has the name Alice, the age 34 and knows bob;
bob has the name Bob and the age 29":

```sql
SELECT sparql_load('
@prefix ex: <http://example.org/> .

ex:alice ex:name  "Alice" ;
         ex:age   34 ;
         ex:knows ex:bob .

ex:bob   ex:name  "Bob" ;
         ex:age   29 .
');
```

```output
5
```

`sparql_load(data)` parses Turtle text and returns the number of new triples it stored. Loading
the same triples again adds nothing (a store holds each triple once) and returns 0.

### 2.2 The first query

A SPARQL `SELECT` query describes a pattern; the answer is every way the pattern matches the
data. `sparql_query()` runs a query and returns the answer as one piece of text, here in the
standard SPARQL JSON results format (`'json'` is the default, so the second argument can be left
out):

```sql
SELECT sparql_query('
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age
  WHERE { ?person ex:name ?name ;
                  ex:age  ?age . }
  ORDER BY ?name
', 'json');
```

```output
{"head":{"vars":["name","age"]},"results":{"bindings":[{"name":{"type":"literal","value":"Alice"},"age":{"type":"literal","value":"34","datatype":"http://www.w3.org/2001/XMLSchema#integer"}},{"name":{"type":"literal","value":"Bob"},"age":{"type":"literal","value":"29","datatype":"http://www.w3.org/2001/XMLSchema#integer"}}]}}
```

JSON is what you would send to a web client. Inside SQL it is more useful to get the answer as
rows. That is what the **`sparql_rows`** table-valued function does: one row per solution, with
the first variable's value in column `v0`, the second in `v1`, and so on.

```sql
SELECT v0, v1 FROM sparql_rows('
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age
  WHERE { ?person ex:name ?name ;
                  ex:age  ?age . }
  ORDER BY ?name
');
```

```output
Alice|34
Bob|29
```

`sparql_rows` has more columns than `v0`, `v1`, .... Switch the shell to `.mode line`, which
prints one column per line, to see them for the first row:

```sql
.mode line
SELECT n, v0, v1, t0, t1, id0, id1, vars, json
FROM sparql_rows('
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age
  WHERE { ?person ex:name ?name ;
                  ex:age  ?age . }
  ORDER BY ?name
')
WHERE n = 1;
```

```output
    n = 1
   v0 = Alice
   v1 = 34
   t0 = "Alice"
   t1 = "34"^^<http://www.w3.org/2001/XMLSchema#integer>
  id0 = 4714258785567244379
  id1 = 550
 vars = ["name","age"]
 json = {"name":{"type":"literal","value":"Alice"},"age":{"type":"literal","value":"34","datatype":"http://www.w3.org/2001/XMLSchema#integer"}}
```

| Column | Holds |
|---|---|
| `n` | the solution's number, starting at 1 |
| `v0` … `v15` | the value of the 1st … 16th variable as a plain SQL value: text for names and IRIs, INTEGER for whole numbers, REAL for decimals (see [6.2](#62-sparql_rows-and-the-sql-type-of-each-value)) |
| `t0` … `t15` | the same value as an RDF term in N-Triples syntax, which keeps the details SQL types lose (`"Alice"` is a string, `34` is typed `xsd:integer`) |
| `id0` … `id15` | the term's internal id in the store (see 2.4) |
| `vars` | the variable names, as a JSON array |
| `json` | the whole solution in SPARQL JSON form |

Variables beyond the sixteenth are only in `json`. A variable that has no value in a solution
gives NULL in `v`, `t` and `id`.

### 2.3 Useful shell settings

`.mode line` is handy for wide rows. For tables, `.mode box` draws a grid with the column names
(`.mode table` does the same with plain ASCII characters, if your terminal font lacks the box
characters):

```sql
.mode box
SELECT v0 AS name, v1 AS age FROM sparql_rows('
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age WHERE { ?person ex:name ?name ; ex:age ?age . } ORDER BY ?name
');
```

```output
┌───────┬─────┐
│ name  │ age │
├───────┼─────┤
│ Alice │ 34  │
│ Bob   │ 29  │
└───────┴─────┘
```

Other settings worth knowing:

- `.headers on` prints column names in the default `list` mode and in `csv` mode (`box` and
  `table` always show them).
- `.timer on` prints how long each statement took (`real` is wall-clock seconds); `.timer off`
  stops it.
- `.mode list` is the default: values separated by `|`, no decoration. It is the best mode for
  long text such as JSON.

<!-- verify: ignore-timing -->
```sql
.timer on
SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }');
.timer off
```

```output
┌──────────┐
│ count(*) │
├──────────┤
│ 5        │
└──────────┘
Run Time: real 0.000 user 0.000000 sys 0.000061
```

The rest of this tutorial uses `.mode box`, with `.mode list` for long text.

### 2.4 Where the data lives

sparqlite keeps everything in ordinary tables of the database file, all named `rdf_*`. The
triples are in `rdf_quad`, one row per triple, as four integers: `g` (the graph; 0 is the
default graph, see section 3), `s`, `p` and `o`:

```sql
SELECT * FROM rdf_quad;
```

```output
┌───┬───────┬───────┬─────────────────────┐
│ g │   s   │   p   │          o          │
├───┼───────┼───────┼─────────────────────┤
│ 0 │ 16449 │ 16417 │ 470                 │
│ 0 │ 16385 │ 16417 │ 550                 │
│ 0 │ 16385 │ 16433 │ 16449               │
│ 0 │ 16385 │ 16401 │ 4714258785567244379 │
│ 0 │ 16449 │ 16401 │ 4787152681057648699 │
└───┴───────┴───────┴─────────────────────┘
```

Each integer is a *term id*, a number that stands for one name or value. Names (the IRIs in
angle brackets that section 3 explains) and longer texts are stored once, in the dictionary table
`rdf_term`, and their id refers to that row; small values such as `34` and `"Alice"` are packed
into the id itself. `sparql_term(id)` turns any id back into the term, written the same way as in
the `t` columns (this notation is called N-Triples):

```sql
SELECT sparql_term(s) AS s, sparql_term(p) AS p, sparql_term(o) AS o FROM rdf_quad;
```

```output
┌────────────────────────────┬────────────────────────────┬──────────────────────────────────────────────────┐
│             s              │             p              │                        o                         │
├────────────────────────────┼────────────────────────────┼──────────────────────────────────────────────────┤
│ <http://example.org/bob>   │ <http://example.org/age>   │ "29"^^<http://www.w3.org/2001/XMLSchema#integer> │
│ <http://example.org/alice> │ <http://example.org/age>   │ "34"^^<http://www.w3.org/2001/XMLSchema#integer> │
│ <http://example.org/alice> │ <http://example.org/knows> │ <http://example.org/bob>                         │
│ <http://example.org/alice> │ <http://example.org/name>  │ "Alice"                                          │
│ <http://example.org/bob>   │ <http://example.org/name>  │ "Bob"                                            │
└────────────────────────────┴────────────────────────────┴──────────────────────────────────────────────────┘
```

You can read these tables with plain SQL, as above. Do not write to them with plain SQL: change
data with `sparql_load` and `sparql_update` (lesson 5.15), which keep the dictionary, indexes
and statistics consistent. You do not need to know more about term ids for the SPARQL course;
section 6.5 explains how they are built and the functions that work with them.

---

## 3. RDF in five minutes

RDF (Resource Description Framework) describes things with **triples**:

```text
subject    predicate    object
ex:alice   ex:knows     ex:bob
ex:alice   ex:age       34
```

A triple is one fact: the subject has a property (the predicate) whose value is the object. A set
of triples is a **graph**: subjects and objects are nodes, each triple is an arrow labelled with
its predicate. There is no schema to declare first; any subject can have any predicate, any
number of times. If you think in tables, a triple is one cell of a very wide, very sparse table
(row = subject, column = predicate), and a multi-valued property just repeats the triple.

### 3.1 IRIs and prefixes

Things and predicates are named by **IRIs**, web-address-like global names, written in angle
brackets: `<http://example.org/alice>`. The IRI does not need to point to an actual web page; it
is just a name that cannot clash with anybody else's. Well-known vocabularies have their own
IRIs, for example `<http://www.w3.org/2000/01/rdf-schema#label>` ("a human-readable name") or
`<https://schema.org/birthDate>`.

Because IRIs are long, Turtle and SPARQL let you declare **prefixes**:

```turtle
@prefix ex:   <http://example.org/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
```

after which `ex:alice` means `<http://example.org/alice>` and `rdfs:label` means
`<http://www.w3.org/2000/01/rdf-schema#label>`. (Turtle writes `@prefix ... .`; SPARQL writes
`PREFIX ex: <...>` without the `@` and the final dot.) The word `a` is short for
`rdf:type`, "is an instance of": `ex:alice a ex:Person`.

### 3.2 Literals: strings, numbers, dates, languages

Objects can also be **literals**, plain values:

| Turtle | Meaning |
|---|---|
| `"Alice"` | a string |
| `"Bob"@en`, `"Roberto"@es` | strings in a given language (a *language tag*) |
| `34` | an integer, short for `"34"^^xsd:integer` |
| `1.82` | a decimal, short for `"1.82"^^xsd:decimal` |
| `4.2e1` | a double (floating-point number), short for `"4.2e1"^^xsd:double` |
| `true` | a boolean, short for `"true"^^xsd:boolean` |
| `"1997-04-02"^^xsd:date` | a date: the text plus a **datatype** IRI after `^^` |

`xsd:` is the XML Schema datatype vocabulary, `<http://www.w3.org/2001/XMLSchema#>`. A literal is
text plus a datatype, either written after `^^` or implied: a plain `"..."` is an `xsd:string`,
and `"..."@xx` is an `rdf:langString` with the language tag `xx`. The datatype is what lets SPARQL
compare `34` and `29` as numbers and dates as dates.

### 3.3 Blank nodes

Sometimes a thing has no name of its own, for example "Bob's chess club membership since 2019".
Turtle writes such an anonymous node in square brackets; inside the store it becomes a **blank
node**, a node with a local, meaningless label such as `_:b1`:

```turtle
ex:bob ex:member [ ex:club "Chess club" ; ex:since 2019 ] .
```

This is three triples: `ex:bob ex:member _:b1`, `_:b1 ex:club "Chess club"` and
`_:b1 ex:since 2019`. The label is not a real name, so you cannot use it to point at the node
later: in a query, `_:b1` acts like a variable (section 6.2 shows this). You reach a blank node
through the named things it hangs off, here `ex:bob ex:member ?m`.

### 3.4 Turtle in one example

Turtle is the most common text format for RDF. A `;` repeats the subject, a `,` repeats subject
and predicate, and `.` ends the statement. Load this into the session from section 2:

```sql
SELECT sparql_load('
@prefix ex:   <http://example.org/> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd:  <http://www.w3.org/2001/XMLSchema#> .

ex:bob a ex:Person ;
       rdfs:label "Bob"@en , "Roberto"@es ;
       ex:born    "1997-04-02"^^xsd:date ;
       ex:height  1.82 ;
       ex:member  [ ex:club "Chess club" ; ex:since 2019 ] .
') AS new_triples;
```

```output
┌─────────────┐
│ new_triples │
├─────────────┤
│ 8           │
└─────────────┘
```

and look at everything about `ex:bob`, using the `t` columns to see the exact RDF terms:

```sql
SELECT t0 AS predicate, t1 AS object FROM sparql_rows('
  SELECT ?p ?o WHERE { <http://example.org/bob> ?p ?o } ORDER BY ?p ?o
');
```

```output
┌───────────────────────────────────────────────────┬───────────────────────────────────────────────────────┐
│                     predicate                     │                        object                         │
├───────────────────────────────────────────────────┼───────────────────────────────────────────────────────┤
│ <http://example.org/age>                          │ "29"^^<http://www.w3.org/2001/XMLSchema#integer>      │
│ <http://example.org/born>                         │ "1997-04-02"^^<http://www.w3.org/2001/XMLSchema#date> │
│ <http://example.org/height>                       │ "1.82"^^<http://www.w3.org/2001/XMLSchema#decimal>    │
│ <http://example.org/member>                       │ _:b1                                                  │
│ <http://example.org/name>                         │ "Bob"                                                 │
│ <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> │ <http://example.org/Person>                           │
│ <http://www.w3.org/2000/01/rdf-schema#label>      │ "Bob"@en                                              │
│ <http://www.w3.org/2000/01/rdf-schema#label>      │ "Roberto"@es                                          │
└───────────────────────────────────────────────────┴───────────────────────────────────────────────────────┘
```

The language tags, the datatypes and the blank node `_:b...` are all there. (sparqlite chooses
blank node labels itself; they are only meaningful inside one database.)

### 3.5 Named graphs

A store can hold several graphs. Triples loaded as above go to the **default graph**. A **named
graph** is a set of triples with an IRI of its own, used to keep apart data from different
sources, versions or years. The TriG format is Turtle plus `GRAPH <name> { ... }` blocks:

```sql
SELECT sparql_load('
@prefix ex: <http://example.org/> .
GRAPH ex:diary-2025 { ex:alice ex:visited ex:paris . }
GRAPH ex:diary-2026 { ex:alice ex:visited ex:rome , ex:paris . }
', 'trig') AS new_quads;
```

```output
┌───────────┐
│ new_quads │
├───────────┤
│ 3         │
└───────────┘
```

Every stored row is really a *quad* (graph, subject, predicate, object), which is why the table
is called `rdf_quad`; the default graph is `g = 0`. SPARQL's `GRAPH` keyword (lesson 5.13) matches
the graph name:

```sql
SELECT v0 AS graph, v1 AS place FROM sparql_rows('
  PREFIX ex: <http://example.org/>
  SELECT ?g ?place WHERE { GRAPH ?g { ex:alice ex:visited ?place } } ORDER BY ?g ?place
');
```

```output
┌───────────────────────────────┬──────────────────────────┐
│             graph             │          place           │
├───────────────────────────────┼──────────────────────────┤
│ http://example.org/diary-2025 │ http://example.org/paris │
│ http://example.org/diary-2026 │ http://example.org/paris │
│ http://example.org/diary-2026 │ http://example.org/rome  │
└───────────────────────────────┴──────────────────────────┘
```

`sparql_load(data, format, base, graph)` can also put Turtle or N-Triples data into a named
graph: the fourth argument is the graph IRI.

That is all the RDF you need. You can `.quit` this shell; the in-memory database disappears.

---

## 4. The tutorial dataset

### 4.1 Build the database

`tutorial/make_tutorial_data.py` writes the data files into `tutorial/data/` and loads them into
a SQLite database. Pick one of two routes.

**Route 1: Python does everything.** This needs a Python whose `sqlite3` module can load
extensions (most Linux and Homebrew builds can):

<!-- verify: setup -->
<!-- verify: ignore-timing -->
```sh
python3 tutorial/make_tutorial_data.py --db tutorial/tutorial.db --ext ext/target/release/libsparqlite
```

```output
wrote languages.ttl         2126 triples (94 languages, 117 people, 40 organisations)
wrote survey.trig            287 quads   (FICTIONAL; 3 named graphs + their metadata)
wrote job_postings.csv       300 rows    (FICTIONAL), and job_postings.sql
loaded sparqlite 0.1.0 (SQLite 3.45.1) from ext/target/release/libsparqlite.so
  languages.ttl          2126 new quads  (0.01s)
  survey.trig             287 new quads  (0.00s)
summary: 2413 quads, 928 dictionary terms, 3 named graphs, 94 languages, 300 job postings
wrote tutorial/tutorial.db
```

**Route 2: the sqlite3 shell does the loading.** The script writes `load.sql`, a script for the
shell. `load.sql` uses `readfile()` with names relative to the shell's current directory, so run
it from inside `tutorial/data` (the parentheses run the `cd` in a sub-shell, so your own shell
stays in the project folder). The `rm -f` first removes a database left from an earlier run;
the paragraph after the output explains why:

<!-- verify: setup fresh -->
<!-- verify: ignore-timing -->
```sh
python3 tutorial/make_tutorial_data.py --emit-sql
rm -f tutorial/tutorial.db
(cd tutorial/data && sqlite3 ../tutorial.db -cmd '.load ../../ext/target/release/libsparqlite' '.read load.sql')
```

```output
wrote languages.ttl         2126 triples (94 languages, 117 people, 40 organisations)
wrote survey.trig            287 quads   (FICTIONAL; 3 named graphs + their metadata)
wrote job_postings.csv       300 rows    (FICTIONAL), and job_postings.sql
wrote load.sql        -- then: cd tutorial/data && sqlite3 ../tutorial.db -cmd '.load ../../ext/target/release/libsparqlite.so' '.read load.sql'
sparql_init: schema version 1
languages.ttl: 2126 new quads
survey.trig: 287 new quads
sparql_analyze: {"analyzed_quads":2413,"default_graph":{"characteristic_sets":87,"heavy_hitters":1,"objects":1173,"predicates":31,"subjects":299,"triples":2146},"ms":2,"named_graphs":{"characteristic_sets":5,"heavy_hitters":0,"objects":206,"predicates":10,"subjects":46,"triples":267}}
quads: 2413, dictionary terms: 928, named graphs: 3, job postings: 300
```

Both routes create the same `tutorial/tutorial.db`. Route 1 always writes a new file. `load.sql`
can also be run on a database that already holds the data: it then removes the old copy
(`removed previous copy: ... quads`) and loads it again. That works, but the reloaded terms get
new internal ids and the blank nodes new labels, so a few ids and labels in section 6 would
differ from this tutorial; hence the `rm -f`.

In Windows PowerShell, route 2 is (not tested; see the Windows note in 1.3):

```powershell
py tutorial/make_tutorial_data.py --emit-sql
Remove-Item tutorial/tutorial.db -ErrorAction SilentlyContinue
Push-Location tutorial/data
sqlite3 ../tutorial.db -cmd ".load ../../ext/target/release/sparqlite" ".read load.sql"
Pop-Location
```

The 928 "dictionary terms" of the summaries are the dictionary rows that the data added (section
6.5 explains the dictionary); `sparql_check()` in section 6.11 reports 963, because it also
counts 35 built-in rows. Run `python3 tutorial/make_tutorial_data.py --help` for all options, and
for a third route that loads the files by hand, one at a time.

### 4.2 What is in it

| Where | What | Real? |
|---|---|---|
| default graph, from `languages.ttl` | 94 computer languages (91 programming languages and the query languages SQL, XQuery and SPARQL), 117 people who designed them, 40 organisations; for each language its label, first year, designers, developers, the languages that influenced it, paradigms, typing discipline, file extensions, and some homepages and licences | **Real**, checked against the English Wikipedia infoboxes. A missing value means "not recorded here", not "none". |
| default graph, from `languages.ttl` | a small vocabulary: classes such as `ex:ProgrammingLanguage`, a paradigm class hierarchy (`ex:Functional` is a subclass of `ex:Declarative`, ...), and labels in English, French, German, Spanish and Japanese | Written for this tutorial |
| named graphs `<http://example.org/graph/survey-2025>` and `.../survey-2026`, from `survey.trig` | a developer survey: usage share, "loved" percentage and median salary per language | **Fictional** (made-up numbers) |
| named graph `<http://example.org/graph/editorial>` | editors' reviews (blank nodes) with ratings, review texts and timestamps | **Fictional** |
| default graph, from `survey.trig` | one description of each named graph (`ex:fictional true`, survey year, number of respondents, ...) | Fictional, and says so |
| SQL table `job_postings` | 300 job adverts: title, company, language, city, salary in USD, date | **Fictional** companies and salaries; real cities |

The prefixes used throughout:

```text
PREFIX ex:      <http://example.org/pl/>           # languages, classes and properties
PREFIX person:  <http://example.org/pl/person/>    # people, e.g. person:guido-van-rossum
PREFIX org:     <http://example.org/pl/org/>       # organisations, e.g. org:bell-labs
PREFIX rdf:     <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX rdfs:    <http://www.w3.org/2000/01/rdf-schema#>
PREFIX xsd:     <http://www.w3.org/2001/XMLSchema#>
PREFIX schema:  <https://schema.org/>
PREFIX dcterms: <http://purl.org/dc/terms/>
PREFIX skos:    <http://www.w3.org/2004/02/skos/core#>
```

A language in `languages.ttl` looks like this (Turtle):

```turtle
ex:Python
    a ex:ProgrammingLanguage ;
    rdfs:label "Python"@en ;
    schema:description "General-purpose language that emphasises readability."@en ;
    rdfs:comment "Named after the comedy group Monty Python."@en ;
    ex:firstAppeared 1991 ;
    ex:firstReleaseDate "1991-02-20"^^xsd:date ;
    ex:designer person:guido-van-rossum ;
    ex:developer org:psf ;
    ex:paradigm ex:ClassBased , ex:Procedural , ex:Functional ;
    ex:typing ex:DynamicTyping ;
    ex:typeStrength ex:StrongTyping ;
    ex:fileExtension ".py" , ".pyi" , ".pyw" ;
    schema:url <https://www.python.org/> ;
    dcterms:license <http://spdx.org/licenses/PSF-2.0> ;
    ex:influencedBy ex:ABC , ex:ALGOL68 , ex:C , ex:CPlusPlus , ex:CLU , ex:Haskell , ex:Icon ,
                    ex:Lisp , ex:Modula3 , ex:Perl , ex:StandardML .
```

Note the shapes you will meet in the queries: language names are language-tagged (`"Python"@en`),
while people and organisations have plain `schema:name` strings; years are integers, dates are
`xsd:date`; `ex:influencedBy` points from a language to an older language that influenced it.

### 4.3 Open it

From here on, all examples run in one shell session on the tutorial database. Start it and set
it up like this:

<!-- verify: session tutorial/tutorial.db -->
```sh
sqlite3 tutorial/tutorial.db
```

```sql
.load ext/target/release/libsparqlite
.mode box --wordwrap on
```

`--wordwrap on` makes box mode break values longer than 60 characters between words rather than
in the middle of one. If you leave the shell and come back, repeat these two lines (and, from
section 6.8 on, the `CREATE VIRTUAL TABLE temp.languages` statement of 6.7, because temporary
tables end with the session). Some examples change settings; the text says when, and how to
change them back.

### 4.4 Explore its shape

A good first question for any RDF dataset: which predicates are used, and how often? In SPARQL,
`?s ?p ?o` matches every triple, and `GROUP BY` / `COUNT` work much as in SQL (lesson 5.8):

```sql
SELECT v0 AS predicate, v1 AS triples FROM sparql_rows('
  SELECT ?p (COUNT(*) AS ?n)
  WHERE { ?s ?p ?o }
  GROUP BY ?p
  ORDER BY DESC(?n) ?p
');
```

```output
┌─────────────────────────────────────────────────┬─────────┐
│                    predicate                    │ triples │
├─────────────────────────────────────────────────┼─────────┤
│ http://www.w3.org/1999/02/22-rdf-syntax-ns#type │ 299     │
│ http://example.org/pl/influencedBy              │ 275     │
│ http://www.w3.org/2000/01/rdf-schema#label      │ 177     │
│ http://example.org/pl/paradigm                  │ 163     │
│ https://schema.org/description                  │ 157     │
│ https://schema.org/name                         │ 157     │
│ http://example.org/pl/designer                  │ 137     │
│ http://example.org/pl/fileExtension             │ 116     │
│ http://example.org/pl/firstAppeared             │ 93      │
│ https://schema.org/birthDate                    │ 69      │
│ http://example.org/pl/typing                    │ 68      │
│ http://www.w3.org/2000/01/rdf-schema#comment    │ 67      │
│ http://example.org/pl/developer                 │ 63      │
│ http://www.w3.org/2004/02/skos/core#altLabel    │ 51      │
│ http://example.org/pl/typeStrength              │ 44      │
│ https://schema.org/url                          │ 40      │
│ https://schema.org/deathDate                    │ 35      │
│ http://purl.org/dc/terms/license                │ 27      │
│ http://www.w3.org/2000/01/rdf-schema#subClassOf │ 16      │
│ http://example.org/pl/turingAwardYear           │ 15      │
│ http://example.org/pl/typeInference             │ 14      │
│ http://www.w3.org/2000/01/rdf-schema#domain     │ 12      │
│ http://www.w3.org/2000/01/rdf-schema#range      │ 12      │
│ http://example.org/pl/firstReleaseDate          │ 10      │
│ http://example.org/pl/version1Date              │ 10      │
│ http://purl.org/dc/terms/identifier             │ 8       │
│ http://example.org/pl/fictional                 │ 3       │
│ http://example.org/pl/marginOfError             │ 2       │
│ http://example.org/pl/respondents               │ 2       │
│ http://example.org/pl/surveyYear                │ 2       │
│ http://purl.org/dc/terms/issued                 │ 2       │
└─────────────────────────────────────────────────┴─────────┘
```

Which kinds of things are there? The predicate `a` (`rdf:type`) says what class a thing belongs
to:

```sql
SELECT v0 AS class, v1 AS things FROM sparql_rows('
  SELECT ?class (COUNT(*) AS ?n)
  WHERE { ?thing a ?class }
  GROUP BY ?class
  ORDER BY DESC(?n) ?class
');
```

```output
┌─────────────────────────────────────────────────────┬────────┐
│                        class                        │ things │
├─────────────────────────────────────────────────────┼────────┤
│ http://example.org/pl/Person                        │ 117    │
│ http://example.org/pl/ProgrammingLanguage           │ 91     │
│ http://example.org/pl/Organization                  │ 40     │
│ http://www.w3.org/2000/01/rdf-schema#Class          │ 19     │
│ http://www.w3.org/1999/02/22-rdf-syntax-ns#Property │ 12     │
│ http://purl.org/dc/terms/LicenseDocument            │ 8      │
│ http://example.org/pl/TypeDiscipline                │ 6      │
│ http://example.org/pl/QueryLanguage                 │ 3      │
│ http://example.org/pl/Survey                        │ 2      │
│ http://example.org/pl/EditorialGraph                │ 1      │
└─────────────────────────────────────────────────────┴────────┘
```

These queries only see the default graph. The named graphs need `GRAPH`:

```sql
SELECT v0 AS graph, v1 AS quads FROM sparql_rows('
  SELECT ?g (COUNT(*) AS ?n)
  WHERE { GRAPH ?g { ?s ?p ?o } }
  GROUP BY ?g
  ORDER BY ?g
');
```

```output
┌──────────────────────────────────────┬───────┐
│                graph                 │ quads │
├──────────────────────────────────────┼───────┤
│ http://example.org/graph/editorial   │ 98    │
│ http://example.org/graph/survey-2025 │ 84    │
│ http://example.org/graph/survey-2026 │ 85    │
└──────────────────────────────────────┴───────┘
```

And the relational table:

```sql
SELECT id, title, company, language, city, salary_usd, posted
FROM job_postings ORDER BY id LIMIT 5;
```

```output
┌────┬───────────────────────────────────┬─────────────────────┬────────────┬───────────┬────────────┬────────────┐
│ id │               title               │       company       │  language  │   city    │ salary_usd │   posted   │
├────┼───────────────────────────────────┼─────────────────────┼────────────┼───────────┼────────────┼────────────┤
│ 1  │ Senior Quant Developer (C++)      │ Pinecrest Payments  │ C++        │ Warsaw    │ 72000      │ 2025-10-02 │
│ 2  │ Senior Software Engineer (Java)   │ Oakhollow Insurance │ Java       │ London    │ 121000     │ 2025-10-06 │
│ 3  │ Senior Data Engineer (Scala)      │ Saltmarsh Biotech   │ Scala      │ Tokyo     │ 96000      │ 2025-10-12 │
│ 4  │ Full-Stack Developer (JavaScript) │ Quarry Lane Data    │ JavaScript │ São Paulo │ 35500      │ 2025-10-12 │
│ 5  │ Senior Game Developer (C#)        │ Redwood Arc Studios │ C#         │ London    │ 127500     │ 2025-10-12 │
└────┴───────────────────────────────────┴─────────────────────┴────────────┴───────────┴────────────┴────────────┘
```

---

## 5. A SPARQL course

Each lesson explains one idea, runs a query on the tutorial database and shows the result, and
ends with an exercise; click "Solution" to see an answer. Write your own attempt first: the
shell is the best teacher.

All queries are wrapped the same way: `sparql_rows` runs the SPARQL text, and the outer SQL
`SELECT` names its columns (`v0` is the first variable of the SPARQL `SELECT`, `v1` the second,
...), so the shell can draw a table:

<!-- verify: skip -->
```sql
SELECT v0 AS first_variable, v1 AS second_variable FROM sparql_rows('
  ... SPARQL query ...
');
```

Inside the SQL string, a single quote must be doubled (`''`), as in any SQL string. SPARQL lets
you use double quotes for its own strings, so you rarely need single quotes inside a query.

### 5.1 SELECT, triple patterns and PREFIX

A SPARQL query is a pattern of triples in which some positions are **variables** (names starting
with `?`). The engine finds every way to give the variables values so that all the triples of the
pattern are in the data. Each way is one **solution**, one row of the result.

Who designed Python?

```sql
SELECT v0 AS designer FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?designer
  WHERE { ex:Python ex:designer ?designer }
');
```

```output
┌───────────────────────────────────────────────┐
│                   designer                    │
├───────────────────────────────────────────────┤
│ http://example.org/pl/person/guido-van-rossum │
└───────────────────────────────────────────────┘
```

The answer is an IRI, the name of a node in the graph. To get something readable, follow one more
arrow: the person's `schema:name`. Patterns in the same `{ }` group are separated by `.`, and a
variable that appears in two patterns must have the same value in both. That is a join, exactly
like joining two tables on a shared column:

```sql
SELECT v0 AS designer, v1 AS name FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?designer ?name
  WHERE {
    ex:Python ex:designer ?designer .
    ?designer schema:name ?name .
  }
');
```

```output
┌───────────────────────────────────────────────┬──────────────────┐
│                   designer                    │       name       │
├───────────────────────────────────────────────┼──────────────────┤
│ http://example.org/pl/person/guido-van-rossum │ Guido van Rossum │
└───────────────────────────────────────────────┴──────────────────┘
```

Things to notice:

- `PREFIX ex: <http://example.org/pl/>` lets you write `ex:Python` instead of
  `<http://example.org/pl/Python>`. Each query declares the prefixes it uses.
- `SELECT ?designer ?name` lists the variables you want back, like a SQL column list. `SELECT *`
  returns every variable of the pattern.
- Keywords are case-insensitive (`select`, `where`); variable names and IRIs are not. The one
  exception among keywords is `a` (lesson 5.2), which must be lower case.

**Exercise 5.1.** In which year did Haskell first appear? The predicate is `ex:firstAppeared`.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS year FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?year
  WHERE { ex:Haskell ex:firstAppeared ?year }
');
```

```output
┌──────┐
│ year │
├──────┤
│ 1990 │
└──────┘
```

</details>

### 5.2 Joins, `;` and `,` shorthand, and `a`

Patterns can be chained as far as you like. Which languages were designed by someone who
received the ACM Turing Award?

```sql
SELECT v0 AS language, v1 AS designer, v2 AS award FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?designer ?award
  WHERE {
    ?lang   ex:designer        ?person ;
            rdfs:label         ?language .
    ?person ex:turingAwardYear ?award ;
            schema:name        ?designer .
  }
  ORDER BY ?award ?language
  LIMIT 8
');
```

```output
┌──────────┬────────────────────┬───────┐
│ language │      designer      │ award │
├──────────┼────────────────────┼───────┤
│ ALGOL 58 │ Alan Perlis        │ 1966  │
│ ALGOL 60 │ Alan Perlis        │ 1966  │
│ ALGOL 60 │ John McCarthy      │ 1971  │
│ Lisp     │ John McCarthy      │ 1971  │
│ ALGOL 58 │ John Backus        │ 1977  │
│ ALGOL 60 │ John Backus        │ 1977  │
│ Fortran  │ John Backus        │ 1977  │
│ APL      │ Kenneth E. Iverson │ 1979  │
└──────────┴────────────────────┴───────┘
```

(`ORDER BY` and `LIMIT` work as in SQL; lesson 5.7 covers them.) The `;` means "same subject,
next predicate", so `?lang ex:designer ?person ; rdfs:label ?language .` is short for two
patterns with the subject `?lang`. A `,` goes one step further and repeats subject *and*
predicate: these are the languages that are both functional and class-based:

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    ?lang ex:paradigm ex:Functional , ex:ClassBased ;
          rdfs:label  ?language .
  }
  ORDER BY ?language
');
```

```output
┌─────────────┐
│  language   │
├─────────────┤
│ C#          │
│ Common Lisp │
│ D           │
│ Dart        │
│ Groovy      │
│ Kotlin      │
│ PHP         │
│ Perl        │
│ Python      │
│ Raku        │
│ Ruby        │
│ Scala       │
│ Swift       │
└─────────────┘
```

The keyword `a` stands for `rdf:type`, the "is a" predicate. The query languages in the dataset:

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE { ?lang a ex:QueryLanguage ; rdfs:label ?language }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ SPARQL   │
│ SQL      │
│ XQuery   │
└──────────┘
```

**Exercise 5.2.** Which languages did Bell Labs develop? The organisation is `org:bell-labs`
(declare `PREFIX org: <http://example.org/pl/org/>`) and the predicate is `ex:developer`. Show
the languages' names.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX org:  <http://example.org/pl/org/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE { ?lang ex:developer org:bell-labs ; rdfs:label ?language }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ AWK      │
│ B        │
│ C        │
│ C++      │
│ Limbo    │
│ S        │
│ SNOBOL   │
└──────────┘
```

</details>

### 5.3 FILTER

`FILTER (condition)` keeps only the solutions for which the condition is true, like SQL's
`WHERE`. A `FILTER` applies to the whole `{ }` group it is written in, wherever it stands in the
group (lesson 5.4 shows why that matters). Comparisons (`=`, `!=`, `<`, `<=`, `>`, `>=`) compare
numbers as numbers, strings as strings and dates as dates; `&&` is "and", `||` is "or", `!` is
"not". Languages that appeared in the 1950s or that are called "Go":

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?year
  WHERE {
    ?lang ex:firstAppeared ?year ;
          rdfs:label ?language .
    FILTER ((?year >= 1950 && ?year < 1960) || ?language = "Go"@en)
  }
  ORDER BY ?year
');
```

```output
┌────────────┬──────┐
│  language  │ year │
├────────────┼──────┤
│ FLOW-MATIC │ 1955 │
│ Fortran    │ 1957 │
│ ALGOL 58   │ 1958 │
│ Go         │ 2009 │
└────────────┴──────┘
```

Why `"Go"@en` and not `"Go"`? The labels are language-tagged, and in RDF the string `"Go"` and
the English word `"Go"@en` are different terms, so `?language = "Go"` is never true. `STR()`
strips the tag: `STR(?language) = "Go"` works too.

String tests: `STRSTARTS`, `STRENDS`, `CONTAINS`, `REGEX` (regular expressions; the optional third
argument `"i"` ignores case), plus `STRLEN`, `UCASE` and `LCASE`. They work on language-tagged
strings too. Languages whose name starts with "C" and contains "+" or "#":

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    ?lang rdfs:label ?language .
    FILTER (STRSTARTS(?language, "C") && (CONTAINS(?language, "+") || CONTAINS(?language, "#")))
  }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ C#       │
│ C++      │
└──────────┘
```

Languages whose English description mentions Apple, in any capitalisation:

```sql
SELECT v0 AS language, v1 AS description FROM sparql_rows('
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?description
  WHERE {
    ?lang rdfs:label ?language ;
          schema:description ?description .
    FILTER (REGEX(?description, "apple", "i") && LANG(?description) = "en")
  }
  ORDER BY ?language
');
```

```output
┌─────────────┬───────────────────────────────────────────────────────────┐
│  language   │                        description                        │
├─────────────┼───────────────────────────────────────────────────────────┤
│ Dylan       │ Multi-paradigm language created at Apple, combining Lisp  │
│             │ ideas with an ALGOL-like syntax.                          │
├─────────────┼───────────────────────────────────────────────────────────┤
│ Objective-C │ C with Smalltalk-style messaging; long the main language  │
│             │ for Apple platforms.                                      │
├─────────────┼───────────────────────────────────────────────────────────┤
│ Swift       │ Apple's general-purpose language for its platforms and    │
│             │ beyond.                                                   │
└─────────────┴───────────────────────────────────────────────────────────┘
```

`LANG(?x)` returns a literal's language tag (`""` if it has none). Some things in the dataset
have labels in several languages; the class `ex:ProgrammingLanguage`, for example:

```sql
SELECT t0 AS label, v1 AS lang FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?label (LANG(?label) AS ?lang)
  WHERE { ex:ProgrammingLanguage rdfs:label ?label }
  ORDER BY ?lang
');
```

```output
┌───────────────────────────────┬──────┐
│             label             │ lang │
├───────────────────────────────┼──────┤
│ "Programmiersprache"@de       │ de   │
│ "programming language"@en     │ en   │
│ "lenguaje de programación"@es │ es   │
│ "langage de programmation"@fr │ fr   │
│ "プログラミング言語"@ja                │ ja   │
└───────────────────────────────┴──────┘
```

(`(LANG(?label) AS ?lang)` computes a new column; lesson 5.9 explains such expressions. The shell
counts characters, not screen columns, so the double-width Japanese text pushes its row out of
line.) Language
tags have sub-tags, such as `en-GB` for British English, and the store keeps tags in lower case.
To ask "is this English?", use `LANGMATCHES(LANG(?x), "en")`, which also accepts `en-gb`; a plain
`LANG(?x) = "en"` does not. The paradigm `ex:ObjectOriented` has one British alternative label:

```sql
SELECT t0 AS alt_label,
       v1 AS lang_is_en,
       v2 AS langmatches_en
FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
  SELECT ?alt (LANG(?alt) = "en" AS ?eq) (LANGMATCHES(LANG(?alt), "en") AS ?m)
  WHERE { ex:ObjectOriented skos:altLabel ?alt }
');
```

```output
┌───────────────────────────────────────┬────────────┬────────────────┐
│               alt_label               │ lang_is_en │ langmatches_en │
├───────────────────────────────────────┼────────────┼────────────────┤
│ "object-orientated programming"@en-gb │ 0          │ 1              │
└───────────────────────────────────────┴────────────┴────────────────┘
```

Finally, `isIRI`, `isLiteral`, `isBlank` and `isNumeric` test what kind of term a value is. The
literal (non-IRI) values recorded for Lua:

```sql
SELECT v0 AS predicate, t1 AS value FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?p ?value
  WHERE { ex:Lua ?p ?value . FILTER (isLiteral(?value)) }
  ORDER BY ?p ?value
');
```

```output
┌──────────────────────────────────────────────┬──────────────────────────────────────────────────────────────┐
│                  predicate                   │                            value                             │
├──────────────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ http://example.org/pl/fileExtension          │ ".lua"                                                       │
├──────────────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ http://example.org/pl/firstAppeared          │ "1993"^^<http://www.w3.org/2001/XMLSchema#integer>           │
├──────────────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ http://www.w3.org/2000/01/rdf-schema#comment │ "'Lua' means 'moon' in Portuguese."@en                       │
├──────────────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ http://www.w3.org/2000/01/rdf-schema#label   │ "Lua"@en                                                     │
├──────────────────────────────────────────────┼──────────────────────────────────────────────────────────────┤
│ https://schema.org/description               │ "Lightweight, embeddable scripting language from PUC-Rio in  │
│                                              │ Brazil."@en                                                  │
└──────────────────────────────────────────────┴──────────────────────────────────────────────────────────────┘
```

**Exercise 5.3.** Which programming languages have a name of at most two characters?

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    ?lang a ex:ProgrammingLanguage ;
          rdfs:label ?language .
    FILTER (STRLEN(?language) <= 2)
  }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ B        │
│ C        │
│ C#       │
│ D        │
│ F#       │
│ Go       │
│ J        │
│ K        │
│ ML       │
│ R        │
│ S        │
└──────────┘
```

</details>

### 5.4 OPTIONAL and BOUND

A plain pattern drops every solution for which part of it has no match. Only 40 of the 94
languages have a homepage (`schema:url`), so requiring one would lose the others. `OPTIONAL { }`
works like SQL's `LEFT JOIN`: if the optional part matches, its variables get values; if not,
the solution is kept and those variables stay **unbound** (NULL in SQL):

```sql
SELECT v0 AS language, v1 AS year, v2 AS homepage FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?year ?homepage
  WHERE {
    ?lang ex:firstAppeared ?year ;
          rdfs:label ?language .
    OPTIONAL { ?lang schema:url ?homepage }
    FILTER (?year >= 1990 && ?year < 1995)
  }
  ORDER BY ?year ?language
');
```

```output
┌──────────┬──────┬────────────────────────────┐
│ language │ year │          homepage          │
├──────────┼──────┼────────────────────────────┤
│ Haskell  │ 1990 │ https://www.haskell.org/   │
│ J        │ 1990 │ https://www.jsoftware.com/ │
│ Python   │ 1991 │ https://www.python.org/    │
│ Dylan    │ 1992 │                            │
│ K        │ 1993 │                            │
│ Lua      │ 1993 │ https://www.lua.org/       │
│ R        │ 1993 │ https://www.r-project.org/ │
└──────────┴──────┴────────────────────────────┘
```

`BOUND(?x)` is true when `?x` has a value. Combined with `OPTIONAL` it finds things that *lack*
something, here the languages from 2010 on without a recorded homepage:

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?year
  WHERE {
    ?lang ex:firstAppeared ?year ;
          rdfs:label ?language .
    OPTIONAL { ?lang schema:url ?homepage }
    FILTER (?year >= 2010 && !BOUND(?homepage))
  }
');
```

```output
┌──────────┬──────┐
│ language │ year │
├──────────┼──────┤
│ Mojo     │ 2023 │
└──────────┴──────┘
```

(Lesson 5.6 shows two more direct ways to say "has no ...".)

**Unbound variables in a FILTER.** A test on a variable that has no value is not false but an
*error*, and `FILTER` drops every solution whose condition is an error. It works like `NULL` in
SQL, where `NULL <> 'x'` is not true either. Here are two attempts at "every language with a
year, except the one whose homepage is python.org", counted with `sparql_count()`, a SQL
function that returns the number of solutions of a query (93 languages have a year):

```sql
SELECT
  sparql_count('
    PREFIX ex:     <http://example.org/pl/>
    PREFIX schema: <https://schema.org/>
    SELECT ?lang WHERE {
      ?lang ex:firstAppeared ?year .
      OPTIONAL { ?lang schema:url ?homepage }
      FILTER (?homepage != <https://www.python.org/>)
    }') AS plain_test,
  sparql_count('
    PREFIX ex:     <http://example.org/pl/>
    PREFIX schema: <https://schema.org/>
    SELECT ?lang WHERE {
      ?lang ex:firstAppeared ?year .
      OPTIONAL { ?lang schema:url ?homepage }
      FILTER (!BOUND(?homepage) || ?homepage != <https://www.python.org/>)
    }') AS with_bound;
```

```output
┌────────────┬────────────┐
│ plain_test │ with_bound │
├────────────┼────────────┤
│ 38         │ 92         │
└────────────┴────────────┘
```

The plain test also throws away every language without a homepage. In the second version,
`!BOUND(?homepage)` is true for those, and `||` is true as soon as one side is true, even if the
other side is an error.

**Where the FILTER goes.** A `FILTER` written directly in a group applies to the whole group. A
`FILTER` inside the braces of an `OPTIONAL` only decides whether the optional part matches. The
languages of 1990-1994 again, this time showing only homepages that end in `.org/`. With the
filter inside the `OPTIONAL`, all seven languages stay, and the other homepage (J's) is left out:

```sql
SELECT v0 AS language, v1 AS year, v2 AS org_homepage FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?year ?homepage
  WHERE {
    ?lang ex:firstAppeared ?year ;
          rdfs:label ?language .
    OPTIONAL { ?lang schema:url ?homepage
               FILTER (STRENDS(STR(?homepage), ".org/")) }
    FILTER (?year >= 1990 && ?year < 1995)
  }
  ORDER BY ?year ?language
');
```

```output
┌──────────┬──────┬────────────────────────────┐
│ language │ year │        org_homepage        │
├──────────┼──────┼────────────────────────────┤
│ Haskell  │ 1990 │ https://www.haskell.org/   │
│ J        │ 1990 │                            │
│ Python   │ 1991 │ https://www.python.org/    │
│ Dylan    │ 1992 │                            │
│ K        │ 1993 │                            │
│ Lua      │ 1993 │ https://www.lua.org/       │
│ R        │ 1993 │ https://www.r-project.org/ │
└──────────┴──────┴────────────────────────────┘
```

With the same test outside the `OPTIONAL`, it filters the languages themselves: J goes because
its homepage does not end in `.org/`, and Dylan and K go because they have no homepage, which
makes the test an error:

```sql
SELECT v0 AS language, v1 AS year, v2 AS org_homepage FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?year ?homepage
  WHERE {
    ?lang ex:firstAppeared ?year ;
          rdfs:label ?language .
    OPTIONAL { ?lang schema:url ?homepage }
    FILTER (?year >= 1990 && ?year < 1995)
    FILTER (STRENDS(STR(?homepage), ".org/"))
  }
  ORDER BY ?year ?language
');
```

```output
┌──────────┬──────┬────────────────────────────┐
│ language │ year │        org_homepage        │
├──────────┼──────┼────────────────────────────┤
│ Haskell  │ 1990 │ https://www.haskell.org/   │
│ Python   │ 1991 │ https://www.python.org/    │
│ Lua      │ 1993 │ https://www.lua.org/       │
│ R        │ 1993 │ https://www.r-project.org/ │
└──────────┴──────┴────────────────────────────┘
```

**Exercise 5.4.** List the people born in 1938, 1939 or 1940 (`schema:birthDate`, an
`xsd:date`) with their date of death (`schema:deathDate`) where one is recorded. Compare dates
with typed literals such as `"1938-01-01"^^xsd:date` (declare
`PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>`).

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS name, v1 AS born, v2 AS died FROM sparql_rows('
  PREFIX schema: <https://schema.org/>
  PREFIX xsd:    <http://www.w3.org/2001/XMLSchema#>
  SELECT ?name ?born ?died
  WHERE {
    ?person schema:name ?name ;
            schema:birthDate ?born .
    OPTIONAL { ?person schema:deathDate ?died }
    FILTER (?born >= "1938-01-01"^^xsd:date && ?born < "1941-01-01"^^xsd:date)
  }
  ORDER BY ?born
');
```

```output
┌──────────────────┬────────────┬────────────┐
│       name       │    born    │    died    │
├──────────────────┼────────────┼────────────┤
│ Charles H. Moore │ 1938-09-09 │            │
│ Cleve Moler      │ 1939-08-17 │ 2026-05-20 │
│ Charles Geschke  │ 1939-09-11 │ 2021-04-16 │
│ Barbara Liskov   │ 1939-11-07 │            │
│ Jean Ichbiah     │ 1940-03-25 │ 2007-01-26 │
│ Alan Kay         │ 1940-05-17 │            │
│ Martin Richards  │ 1940-07-21 │            │
│ John Warnock     │ 1940-10-06 │ 2023-08-19 │
└──────────────────┴────────────┴────────────┘
```

An empty `died` cell means that no date of death is recorded in the dataset, nothing more.
Dates come back as ISO 8601 text, so SQLite's own date functions work on them.

</details>

### 5.5 UNION

`{ A } UNION { B }` returns the solutions of A plus those of B, like SQL's `UNION ALL`. Everyone
credited for Java, as designer or as developer:

```sql
SELECT v0 AS name, v1 AS who FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?name ?who
  WHERE {
    { ex:Java ex:designer ?who } UNION { ex:Java ex:developer ?who }
    ?who schema:name ?name .
  }
  ORDER BY ?name
');
```

```output
┌──────────────────┬────────────────────────────────────────────┐
│       name       │                    who                     │
├──────────────────┼────────────────────────────────────────────┤
│ James Gosling    │ http://example.org/pl/person/james-gosling │
│ Oracle           │ http://example.org/pl/org/oracle           │
│ Sun Microsystems │ http://example.org/pl/org/sun-microsystems │
└──────────────────┴────────────────────────────────────────────┘
```

The two branches may bind different variables; a variable that only one branch binds is unbound
in the other branch's solutions. The other patterns of the group, before or after the `UNION`,
are joined with the solutions of both branches.

**Exercise 5.5.** Find the languages that use gradual typing (`ex:typing ex:GradualTyping`) or
logic programming (`ex:paradigm ex:Logic`), with a `UNION`.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    { ?lang ex:typing ex:GradualTyping } UNION { ?lang ex:paradigm ex:Logic }
    ?lang rdfs:label ?language .
  }
  ORDER BY ?language
');
```

```output
┌────────────┐
│  language  │
├────────────┤
│ Dylan      │
│ Hack       │
│ Mercury    │
│ Prolog     │
│ Raku       │
│ TypeScript │
└────────────┘
```

A language that matched both branches would appear twice; `SELECT DISTINCT` (lesson 5.7) removes
such duplicates.

</details>

### 5.6 MINUS and FILTER NOT EXISTS

Two ways to say "without": `FILTER NOT EXISTS { pattern }` keeps a solution if the pattern,
with the solution's values filled in, has **no** match; `MINUS { pattern }` removes the solutions
that are **compatible** with some solution of the pattern. The programming languages without a
recorded designer, both ways:

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    ?lang a ex:ProgrammingLanguage ; rdfs:label ?language .
    FILTER NOT EXISTS { ?lang ex:designer ?someone }
  }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ Caml     │
│ Carbon   │
│ Cyclone  │
│ Dylan    │
│ Hack     │
│ Mesa     │
│ PL/I     │
│ Racket   │
└──────────┘
```

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE {
    ?lang a ex:ProgrammingLanguage ; rdfs:label ?language .
    MINUS { ?lang ex:designer ?someone }
  }
  ORDER BY ?language
');
```

```output
┌──────────┐
│ language │
├──────────┤
│ Caml     │
│ Carbon   │
│ Cyclone  │
│ Dylan    │
│ Hack     │
│ Mesa     │
│ PL/I     │
│ Racket   │
└──────────┘
```

They agree here because the inner pattern shares the variable `?lang` with the outer one. They
differ in two situations. The first is when nothing is shared. `MINUS` only removes solutions
that share at least one variable with the removed pattern, so a pattern without shared variables
removes nothing. `NOT EXISTS` simply asks "does this pattern match?", which for a pattern without
shared variables is the same question for every solution. Here are both versions, counted with
`sparql_count()` (lesson 5.4):

```sql
SELECT
  sparql_count('PREFIX ex: <http://example.org/pl/>
    SELECT ?lang WHERE { ?lang a ex:ProgrammingLanguage
                         MINUS { ex:Python ex:designer ?d } }') AS with_minus,
  sparql_count('PREFIX ex: <http://example.org/pl/>
    SELECT ?lang WHERE { ?lang a ex:ProgrammingLanguage
                         FILTER NOT EXISTS { ex:Python ex:designer ?d } }') AS with_not_exists;
```

```output
┌────────────┬─────────────────┐
│ with_minus │ with_not_exists │
├────────────┼─────────────────┤
│ 91         │ 0               │
└────────────┴─────────────────┘
```

Python has a designer, so `NOT EXISTS` is false for every language and removes all 91; `MINUS`
finds no shared variable and removes none.

The second situation is a `FILTER` inside the inner pattern that uses an outer variable.
`NOT EXISTS` checks its pattern with the solution's values filled in, so the filter can see
them. `MINUS` evaluates its pattern on its own, where the outer variables have no value. The
oldest language is the one for which no other language has an earlier year:

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?year
  WHERE {
    ?lang ex:firstAppeared ?year ; rdfs:label ?language .
    FILTER NOT EXISTS { ?other ex:firstAppeared ?earlier . FILTER (?earlier < ?year) }
  }
');
```

```output
┌────────────┬──────┐
│  language  │ year │
├────────────┼──────┤
│ FLOW-MATIC │ 1955 │
└────────────┴──────┘
```

Written with `MINUS`, the same query keeps all 93 languages that have a year: inside `MINUS`,
`?year` is unbound, so `?earlier < ?year` is an error, the inner pattern has no solutions, and
nothing is removed:

```sql
SELECT sparql_count('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?lang
  WHERE {
    ?lang ex:firstAppeared ?year .
    MINUS { ?other ex:firstAppeared ?earlier . FILTER (?earlier < ?year) }
  }') AS with_minus;
```

```output
┌────────────┐
│ with_minus │
├────────────┤
│ 93         │
└────────────┘
```

Rule of thumb: use `FILTER NOT EXISTS` when you mean "there is no ...", and `MINUS` when you mean
"take away these solutions". (`FILTER EXISTS` is the positive form.)

**Exercise 5.6.** Which languages influenced at least one other language in the dataset but were
not themselves influenced by any language in it? (Use `DISTINCT` after `SELECT` to list each
once; it is explained in the next lesson.)

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT DISTINCT ?language
  WHERE {
    ?newer ex:influencedBy ?root .
    ?root rdfs:label ?language .
    FILTER NOT EXISTS { ?root ex:influencedBy ?older }
  }
  ORDER BY ?language
');
```

```output
┌──────────────────┐
│     language     │
├──────────────────┤
│ APL              │
│ FLOW-MATIC       │
│ Fortran          │
│ Lisp             │
│ MATLAB           │
│ Mesa             │
│ Prolog           │
│ S                │
│ SNOBOL           │
│ SQL              │
│ Wolfram Language │
└──────────────────┘
```

</details>

### 5.7 DISTINCT, ORDER BY, LIMIT and OFFSET

These work as in SQL. `ORDER BY ?x` sorts ascending, `ORDER BY DESC(?x)` descending; several keys
are separated by spaces. Unbound values sort first, and strings sort by Unicode code point, so
upper-case letters come before lower-case ones ("PHP" before "Perl" in lesson 5.2). Without
`ORDER BY`, the order of the solutions is not defined: it can change when the data or the engine
changes, so sort whenever order matters, and add a second key to break ties. `LIMIT n` keeps the
first n solutions and `OFFSET n` skips n, which together page through a result. The five most
recent languages:

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?year
  WHERE { ?lang ex:firstAppeared ?year ; rdfs:label ?language }
  ORDER BY DESC(?year) ?language
  LIMIT 5
');
```

```output
┌──────────┬──────┐
│ language │ year │
├──────────┼──────┤
│ Mojo     │ 2023 │
│ Carbon   │ 2022 │
│ Gleam    │ 2016 │
│ Zig      │ 2016 │
│ Raku     │ 2015 │
└──────────┴──────┘
```

`SELECT DISTINCT` removes duplicate solutions. The paradigms of the languages that appeared
before 1960; without `DISTINCT`, "procedural programming" would appear once per language:

```sql
SELECT v0 AS paradigm FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT DISTINCT ?paradigm
  WHERE {
    ?lang ex:firstAppeared ?year ;
          ex:paradigm ?p .
    ?p rdfs:label ?paradigm .
    FILTER (?year < 1960 && LANG(?paradigm) = "en")
  }
  ORDER BY ?paradigm
');
```

```output
┌────────────────────────┐
│        paradigm        │
├────────────────────────┤
│ array programming      │
│ imperative programming │
│ procedural programming │
└────────────────────────┘
```

**Exercise 5.7.** Page through the languages by year, five per page: show page 3 (the 11th to
15th oldest languages with a known year, ties broken by name).

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?year
  WHERE { ?lang ex:firstAppeared ?year ; rdfs:label ?language }
  ORDER BY ?year ?language
  LIMIT 5 OFFSET 10
');
```

```output
┌──────────┬──────┐
│ language │ year │
├──────────┼──────┤
│ PL/I     │ 1964 │
│ ALGOL W  │ 1966 │
│ APL      │ 1966 │
│ ISWIM    │ 1966 │
│ BCPL     │ 1967 │
└──────────┴──────┘
```

</details>

### 5.8 Aggregates: COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT, SAMPLE

`GROUP BY` splits the solutions into groups, and aggregate functions summarise each group, as in
SQL. In the `SELECT` list, an aggregate (like any expression) must be given a name with
`(... AS ?name)`; in `HAVING` and `ORDER BY` it can be used directly. Per paradigm: how many
languages, the earliest and latest year, and the average year:

```sql
SELECT v0 AS paradigm, v1 AS languages, v2 AS first, v3 AS last, v4 AS average
FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?paradigm (COUNT(?lang) AS ?n) (MIN(?year) AS ?first)
         (MAX(?year) AS ?last) (AVG(?year) AS ?avg)
  WHERE {
    ?lang ex:paradigm ?p ;
          ex:firstAppeared ?year .
    ?p rdfs:label ?paradigm .
    FILTER (LANG(?paradigm) = "en")
  }
  GROUP BY ?paradigm
  ORDER BY DESC(?n) ?paradigm
');
```

```output
┌─────────────────────────────────────────┬───────────┬───────┬──────┬──────────────────┐
│                paradigm                 │ languages │ first │ last │     average      │
├─────────────────────────────────────────┼───────────┼───────┼──────┼──────────────────┤
│ procedural programming                  │ 49        │ 1957  │ 2022 │ 1983.79591836735 │
│ functional programming                  │ 42        │ 1960  │ 2016 │ 1995.28571428571 │
│ class-based programming                 │ 22        │ 1962  │ 2015 │ 1995.86363636364 │
│ object-oriented programming             │ 12        │ 1975  │ 2022 │ 1994.5           │
│ concurrent programming                  │ 10        │ 1980  │ 2016 │ 2002.6           │
│ array programming                       │ 7         │ 1957  │ 2012 │ 1985.57142857143 │
│ imperative programming                  │ 4         │ 1955  │ 2023 │ 1979.0           │
│ prototype-based programming             │ 4         │ 1987  │ 2009 │ 1996.0           │
│ declarative programming                 │ 3         │ 1973  │ 2008 │ 1996.0           │
│ purely functional programming           │ 3         │ 1985  │ 2012 │ 1995.66666666667 │
│ concatenative (stack-based) programming │ 2         │ 1970  │ 1982 │ 1976.0           │
│ logic programming                       │ 2         │ 1972  │ 1995 │ 1983.5           │
└─────────────────────────────────────────┴───────────┴───────┴──────┴──────────────────┘
```

Unlike SQLite, SPARQL allows no other variables in the `SELECT` list of a grouped query: each one
must be a `GROUP BY` key or appear inside an aggregate. Use `SAMPLE(?x)` (below) when any one
value will do. Breaking the rule gives a syntax error whose real reason, "The SELECT contains a
variable that is unbound", is buried in a long list of what the parser expected:

```sql
SELECT v0 FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?designer ?lang (COUNT(?lang) AS ?n)
  WHERE { ?lang ex:designer ?person . ?person schema:name ?designer }
  GROUP BY ?designer
');
```

```output
Runtime error: sparql: syntax error: error at 7:1: expected one of "$", "(", ":", "<", "?", STRLANGDIR, The SELECT contains a variable that is unbound, ['A' ..= 'Z' | 'a' ..= 'z' | '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00F6}'
| '\u{00F8}'..='\u{02FF}' | '\u{0370}'..='\u{037D}' | '\u{037F}'..='\u{1FFF}'
| '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}'
| '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}'], [_], hasLangDir
```

Here `?lang` is neither a key nor aggregated. (Section 6.14 has more on reading such messages.)

`HAVING` filters groups after aggregation. `SAMPLE(?x)` picks any one value from the group, and
`GROUP_CONCAT(?x; separator=" ")` joins all of them into one string, in no defined order. The
people who designed at least three languages:

```sql
SELECT v0 AS designer, v1 AS languages, v2 AS names, v3 AS one_of_them FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?designer (COUNT(?lang) AS ?n) (GROUP_CONCAT(?language; separator=", ") AS ?names)
         (SAMPLE(?language) AS ?one)
  WHERE {
    ?lang ex:designer ?person ; rdfs:label ?language .
    ?person schema:name ?designer .
  }
  GROUP BY ?designer
  HAVING (COUNT(?lang) >= 3)
  ORDER BY DESC(?n) ?designer
');
```

```output
┌──────────────────┬───────────┬──────────────────────────────────────────────────┬───────────────┐
│     designer     │ languages │                      names                       │  one_of_them  │
├──────────────────┼───────────┼──────────────────────────────────────────────────┼───────────────┤
│ Niklaus Wirth    │ 5         │ ALGOL W, Pascal, Modula-2, Object Pascal, Oberon │ ALGOL W       │
│ Anders Hejlsberg │ 3         │ Object Pascal, C#, TypeScript                    │ Object Pascal │
│ John Backus      │ 3         │ Fortran, ALGOL 58, ALGOL 60                      │ Fortran       │
└──────────────────┴───────────┴──────────────────────────────────────────────────┴───────────────┘
```

Without `GROUP BY`, the whole result is one group. `COUNT(DISTINCT ?x)` counts different values.
Be careful with solutions in which the aggregated variable has no value. This query combines the
275 influence edges with the two fictional surveys (described in the default graph), so `?older`
is unbound in two solutions and `?respondents` in 275:

```sql
SELECT v0 AS solutions, v1 AS edges, v2 AS influencers, v3 AS respondents
FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT (COUNT(*) AS ?all) (COUNT(?older) AS ?edges)
         (COUNT(DISTINCT ?older) AS ?influencers) (SUM(?respondents) AS ?total)
  WHERE {
    { ?lang ex:influencedBy ?older }
    UNION
    { ?survey ex:respondents ?respondents }
  }
');
```

```output
┌───────────┬───────┬─────────────┬─────────────┐
│ solutions │ edges │ influencers │ respondents │
├───────────┼───────┼─────────────┼─────────────┤
│ 277       │ 275   │ 72          │             │
└───────────┴───────┴─────────────┴─────────────┘
```

`COUNT(*)` counts all solutions, and `COUNT(?older)` skips the ones where `?older` is unbound.
But `SUM`, `AVG`, `MIN`, `MAX` and `GROUP_CONCAT` have **no value at all** for a group in which
the variable is unbound in some solution. `SUM` and `AVG` also give no value if one of the values
is not a number; `MIN` and `MAX` compare strings, dates and numbers alike (in the order `ORDER BY`
uses), and `GROUP_CONCAT` uses the text of each value. This is a strict reading of the SPARQL
rules; some other SPARQL engines skip the missing values instead. Filter the solutions, or give
missing values a default with `COALESCE` (lesson 5.9): `SUM(COALESCE(?respondents, 0))` is
100090.

**Exercise 5.8.** How many languages first appeared in each decade? Hint: you can group by an
expression, `GROUP BY (FLOOR(?year / 10) * 10 AS ?decade)`, and select `?decade`.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS decade, v1 AS languages FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?decade (COUNT(?lang) AS ?n)
  WHERE { ?lang ex:firstAppeared ?year }
  GROUP BY (FLOOR(?year / 10) * 10 AS ?decade)
  ORDER BY ?decade
');
```

```output
┌────────┬───────────┐
│ decade │ languages │
├────────┼───────────┤
│ 1950.0 │ 3         │
│ 1960.0 │ 15        │
│ 1970.0 │ 14        │
│ 1980.0 │ 18        │
│ 1990.0 │ 15        │
│ 2000.0 │ 13        │
│ 2010.0 │ 13        │
│ 2020.0 │ 2         │
└────────┴───────────┘
```

The decades come out as `1950.0`: in SPARQL, dividing two integers gives a decimal, and
`FLOOR` keeps it a decimal, which sparqlite hands to SQL as a REAL. Lesson 5.9 shows how to turn
it back into an integer.

</details>

### 5.9 BIND, expressions, IF, COALESCE and datatypes

`BIND (expression AS ?var)` computes a new value inside the pattern, and `(expression AS ?var)` in
the `SELECT` list does the same for the output. The people with both dates recorded who lived
longest (`YEAR()` takes the year out of a date):

```sql
SELECT v0 AS name, v1 AS born, v2 AS died, v3 AS years FROM sparql_rows('
  PREFIX schema: <https://schema.org/>
  SELECT ?name ?born ?died ?years
  WHERE {
    ?person schema:name ?name ;
            schema:birthDate ?born ;
            schema:deathDate ?died .
    BIND (YEAR(?died) - YEAR(?born) AS ?years)
  }
  ORDER BY DESC(?years) ?name
  LIMIT 5
');
```

```output
┌─────────────────┬────────────┬────────────┬───────┐
│      name       │    born    │    died    │ years │
├─────────────────┼────────────┼────────────┼───────┤
│ Thomas E. Kurtz │ 1928-02-22 │ 2024-11-12 │ 96    │
│ David J. Farber │ 1934-04-17 │ 2026-02-07 │ 92    │
│ Tony Hoare      │ 1934-01-11 │ 2026-03-05 │ 92    │
│ Niklaus Wirth   │ 1934-02-15 │ 2024-01-01 │ 90    │
│ Jean E. Sammet  │ 1928-03-23 │ 2017-05-20 │ 89    │
└─────────────────┴────────────┴────────────┴───────┘
```

(That is the difference between the years, not the exact age.) Unlike `FILTER`, the position of
`BIND` matters: it sees only the patterns written before it in its group, so write it after
them (placed first, `?years` would stay unbound in every row). And it must introduce a new
variable; binding a variable the patterns before it already bind is a syntax error.

`IF(condition, then, else)` picks
one of two values, and `COALESCE(a, b, ...)` returns the first argument that has a value, which
is how you give an `OPTIONAL` variable a default. `STR()` turns an IRI or literal into a plain
string, and `STRAFTER(text, prefix)` returns the part after `prefix`:

```sql
SELECT v0 AS id, v1 AS year, v2 AS era, v3 AS homepage FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT (STRAFTER(STR(?lang), "/pl/") AS ?id) ?year
         (IF(?year < 1970, "classic", "modern") AS ?era)
         (COALESCE(STR(?url), "(none recorded)") AS ?home)
  WHERE {
    VALUES ?lang { ex:Lisp ex:Python ex:Gleam }
    ?lang ex:firstAppeared ?year .
    OPTIONAL { ?lang schema:url ?url }
  }
  ORDER BY ?year
');
```

```output
┌────────┬──────┬─────────┬─────────────────────────┐
│   id   │ year │   era   │        homepage         │
├────────┼──────┼─────────┼─────────────────────────┤
│ Lisp   │ 1960 │ classic │ (none recorded)         │
│ Python │ 1991 │ modern  │ https://www.python.org/ │
│ Gleam  │ 2016 │ modern  │ https://gleam.run/      │
└────────┴──────┴─────────┴─────────────────────────┘
```

(`VALUES` lists the starting values for `?lang`; it is the subject of lesson 5.10.) SPARQL has
many more functions; a sample of the string and numeric ones:

```sql
SELECT v0 AS concat, v1 AS ucase, v2 AS substr, v3 AS replace,
       v4 AS round, v5 AS abs, v6 AS division
FROM sparql_rows('
  SELECT (CONCAT("Py", "thon") AS ?a) (UCASE("go") AS ?b) (SUBSTR("JavaScript", 5) AS ?c)
         (REPLACE("JavaScript", "Script", "") AS ?d) (ROUND(2.5) AS ?e) (ABS(-3) AS ?f)
         (7 / 2 AS ?g)
  WHERE {}
');
```

```output
┌────────┬───────┬────────┬─────────┬───────┬─────┬──────────┐
│ concat │ ucase │ substr │ replace │ round │ abs │ division │
├────────┼───────┼────────┼─────────┼───────┼─────┼──────────┤
│ Python │ GO    │ Script │ Java    │ 3.0   │ 3   │ 3.5      │
└────────┴───────┴────────┴─────────┴───────┴─────┴──────────┘
```

Arithmetic follows the types of its operands: an integer combined with a decimal gives a
decimal, anything combined with a double gives a double, and `/` on two integers gives a decimal,
hence `3.5` (`ROUND` keeps the decimal type, hence `3.0`).

Every literal has a **datatype**, and functions and comparisons respect it. `DATATYPE(?x)` returns
it; a cast such as `xsd:integer(?x)` converts a value and fails (leaves the variable unbound) when
it cannot:

```sql
SELECT t0 AS term, v1 AS datatype, v2 AS as_integer FROM sparql_rows('
  PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
  SELECT ?value (DATATYPE(?value) AS ?type) (xsd:integer(?value) AS ?int)
  WHERE {
    VALUES ?value { 42 4.2 "17" "seventeen" "2026-10-15"^^xsd:date "x"@en }
  }
');
```

```output
┌───────────────────────────────────────────────────────┬───────────────────────────────────────────────────────┬────────────┐
│                         term                          │                       datatype                        │ as_integer │
├───────────────────────────────────────────────────────┼───────────────────────────────────────────────────────┼────────────┤
│ "42"^^<http://www.w3.org/2001/XMLSchema#integer>      │ http://www.w3.org/2001/XMLSchema#integer              │ 42         │
│ "4.2"^^<http://www.w3.org/2001/XMLSchema#decimal>     │ http://www.w3.org/2001/XMLSchema#decimal              │ 4          │
│ "17"                                                  │ http://www.w3.org/2001/XMLSchema#string               │ 17         │
│ "seventeen"                                           │ http://www.w3.org/2001/XMLSchema#string               │            │
│ "2026-10-15"^^<http://www.w3.org/2001/XMLSchema#date> │ http://www.w3.org/2001/XMLSchema#date                 │            │
│ "x"@en                                                │ http://www.w3.org/1999/02/22-rdf-syntax-ns#langString │            │
└───────────────────────────────────────────────────────┴───────────────────────────────────────────────────────┴────────────┘
```

Notes: `4.2` cast to an integer is `4`; a simple string like `"17"` has the datatype
`xsd:string`; a language-tagged string has `rdf:langString`. Other casts: `xsd:decimal()`,
`xsd:double()`, `xsd:string()`, `xsd:boolean()`, `xsd:date()`, `xsd:dateTime()`. `STRDT("17",
xsd:integer)` builds a typed literal from a string and `STRLANG("chat", "fr")` a tagged one. And
`xsd:integer(FLOOR(?year / 10) * 10)` gives the decades of exercise 5.8 as integers.

**Exercise 5.9.** For each Turing Award winner with a recorded birth date, show the year of the
award and the difference between that year and the year of birth, youngest first.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS name, v1 AS award, v2 AS age FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?name ?award (?award - YEAR(?born) AS ?age)
  WHERE {
    ?person schema:name ?name ;
            ex:turingAwardYear ?award ;
            schema:birthDate ?born .
  }
  ORDER BY ?age ?name
');
```

```output
┌────────────────────┬───────┬─────┐
│        name        │ award │ age │
├────────────────────┼───────┼─────┤
│ Ken Thompson       │ 1983  │ 40  │
│ Dennis Ritchie     │ 1983  │ 42  │
│ Alan Perlis        │ 1966  │ 44  │
│ John McCarthy      │ 1971  │ 44  │
│ Tony Hoare         │ 1980  │ 46  │
│ Niklaus Wirth      │ 1984  │ 50  │
│ John Backus        │ 1977  │ 53  │
│ Robin Milner       │ 1991  │ 57  │
│ Kenneth E. Iverson │ 1979  │ 59  │
│ Alan Kay           │ 2003  │ 63  │
│ Barbara Liskov     │ 2008  │ 69  │
│ Ole-Johan Dahl     │ 2001  │ 70  │
│ Kristen Nygaard    │ 2001  │ 75  │
│ Peter Naur         │ 2005  │ 77  │
│ Alfred Aho         │ 2020  │ 79  │
└────────────────────┴───────┴─────┘
```

</details>

### 5.10 VALUES

`VALUES` supplies a small table of values inline, like a SQL `VALUES` list joined into the query.
With one variable the values go in braces; with several, each row goes in parentheses (`UNDEF`
leaves a cell unbound):

```sql
SELECT v0 AS language, v1 AS nickname, v2 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?nickname ?year
  WHERE {
    VALUES (?lang ?nickname) {
      (ex:Python  "batteries included")
      (ex:Rust    "fearless concurrency")
      (ex:Haskell UNDEF)
    }
    ?lang rdfs:label ?language ;
          ex:firstAppeared ?year .
  }
  ORDER BY ?year
');
```

```output
┌──────────┬──────────────────────┬──────┐
│ language │       nickname       │ year │
├──────────┼──────────────────────┼──────┤
│ Haskell  │                      │ 1990 │
│ Python   │ batteries included   │ 1991 │
│ Rust     │ fearless concurrency │ 2012 │
└──────────┴──────────────────────┴──────┘
```

`VALUES` is the natural way to pass a list of things into a query, for example a list of ids
built by your application.

**Exercise 5.10.** Using `VALUES`, list the designers (by name) of C, Pascal and Smalltalk,
together with the language.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language, v1 AS designer FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?language ?designer
  WHERE {
    VALUES ?lang { ex:C ex:Pascal ex:Smalltalk }
    ?lang rdfs:label ?language ;
          ex:designer ?person .
    ?person schema:name ?designer .
  }
  ORDER BY ?language ?designer
');
```

```output
┌───────────┬────────────────┐
│ language  │    designer    │
├───────────┼────────────────┤
│ C         │ Dennis Ritchie │
│ Pascal    │ Niklaus Wirth  │
│ Smalltalk │ Adele Goldberg │
│ Smalltalk │ Alan Kay       │
│ Smalltalk │ Dan Ingalls    │
└───────────┴────────────────┘
```

</details>

### 5.11 Sub-queries

A complete `SELECT` can be nested inside `{ }` and its results joined with the rest of the
pattern, like a derived table in SQL. Only the variables the inner `SELECT` projects are visible
outside. A classic use is "the row with the minimum per group": first compute each paradigm's
earliest year, then find the languages of that paradigm and year:

```sql
SELECT v0 AS paradigm, v1 AS earliest_language, v2 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?paradigm ?language ?first
  WHERE {
    {
      SELECT ?p (MIN(?year) AS ?first)
      WHERE { ?l ex:paradigm ?p ; ex:firstAppeared ?year }
      GROUP BY ?p
    }
    ?lang ex:paradigm ?p ;
          ex:firstAppeared ?first ;
          rdfs:label ?language .
    ?p rdfs:label ?paradigm .
    FILTER (LANG(?paradigm) = "en")
  }
  ORDER BY ?first ?paradigm
');
```

```output
┌─────────────────────────────────────────┬───────────────────┬──────┐
│                paradigm                 │ earliest_language │ year │
├─────────────────────────────────────────┼───────────────────┼──────┤
│ imperative programming                  │ FLOW-MATIC        │ 1955 │
│ array programming                       │ Fortran           │ 1957 │
│ procedural programming                  │ Fortran           │ 1957 │
│ functional programming                  │ Lisp              │ 1960 │
│ class-based programming                 │ Simula            │ 1962 │
│ concatenative (stack-based) programming │ Forth             │ 1970 │
│ logic programming                       │ Prolog            │ 1972 │
│ declarative programming                 │ SQL               │ 1973 │
│ object-oriented programming             │ CLU               │ 1975 │
│ concurrent programming                  │ Ada               │ 1980 │
│ purely functional programming           │ Miranda           │ 1985 │
│ prototype-based programming             │ Self              │ 1987 │
└─────────────────────────────────────────┴───────────────────┴──────┘
```

The inner query runs first, conceptually; its `ORDER BY` / `LIMIT` apply to it alone, so
sub-queries are also how you join against "the top 3 of something".

**Exercise 5.11.** Which language has the most file extensions (`ex:fileExtension`), and how many?
Use a sub-query that counts per language, sorted, with `LIMIT 1`.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language, v1 AS extensions FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?n
  WHERE {
    {
      SELECT ?lang (COUNT(?ext) AS ?n)
      WHERE { ?lang ex:fileExtension ?ext }
      GROUP BY ?lang
      ORDER BY DESC(?n)
      LIMIT 1
    }
    ?lang rdfs:label ?language .
  }
');
```

```output
┌──────────┬────────────┐
│ language │ extensions │
├──────────┼────────────┤
│ C++      │ 6          │
└──────────┴────────────┘
```

With `LIMIT 1`, a tie for first place would show only one of the winners; comparing with
`MAX` in a second sub-query avoids that.

</details>

### 5.12 Property paths

So far each pattern followed exactly one arrow. A **property path** describes a route through
the graph in one pattern:

| Path | Meaning |
|---|---|
| `p1/p2` | follow `p1`, then `p2` |
| `p1\|p2` | follow `p1` or `p2` |
| `^p` | follow `p` backwards |
| `p*` | follow `p` zero or more times |
| `p+` | follow `p` one or more times |
| `p?` | follow `p` zero times or once |
| `!p`, `!(p1\|p2)` | follow any one predicate except `p` (or except `p1` and `p2`) |

`/` saves intermediate variables. The names of everyone behind Java, as designer or developer,
in one pattern:

```sql
SELECT v0 AS name FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?name
  WHERE { ex:Java (ex:designer|ex:developer)/schema:name ?name }
  ORDER BY ?name
');
```

```output
┌──────────────────┐
│       name       │
├──────────────────┤
│ James Gosling    │
│ Oracle           │
│ Sun Microsystems │
└──────────────────┘
```

`ex:influencedBy` points from a language to an older one. `^ex:influencedBy` walks the other way,
from a language to the languages it influenced:

```sql
SELECT v0 AS influenced_by_c FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language
  WHERE { ex:C ^ex:influencedBy/rdfs:label ?language }
  ORDER BY ?language
');
```

```output
┌─────────────────┐
│ influenced_by_c │
├─────────────────┤
│ AWK             │
│ C++             │
│ Cyclone         │
│ D               │
│ Go              │
│ Julia           │
│ Limbo           │
│ Objective-C     │
│ PHP             │
│ Perl            │
│ Python          │
│ Zig             │
└─────────────────┘
```

`+` follows the arrow one or more times, which gives the transitive closure:
`ex:Rust ex:influencedBy+ ?x` finds the languages that influenced Rust, the languages that
influenced those, and so on back to the 1950s. Counted for every language, this ranks the most
influential ones by the number of languages that descend from them:

```sql
SELECT v0 AS language, v1 AS influenced FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language (COUNT(DISTINCT ?descendant) AS ?n)
  WHERE {
    ?descendant ex:influencedBy+ ?ancestor .
    ?ancestor rdfs:label ?language .
  }
  GROUP BY ?language
  ORDER BY DESC(?n) ?language
  LIMIT 5
');
```

```output
┌──────────┬────────────┐
│ language │ influenced │
├──────────┼────────────┤
│ Fortran  │ 74         │
│ ALGOL 58 │ 73         │
│ ALGOL 60 │ 72         │
│ Lisp     │ 54         │
│ Simula   │ 47         │
└──────────┴────────────┘
```

Fortran comes first: ALGOL 58 was influenced by it, and almost everything since descends from
ALGOL. Paths also answer yes/no questions about reachability:

```sql
SELECT v0 AS direct, v1 AS indirect FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT
    (EXISTS { ex:Rust ex:influencedBy  ex:ALGOL60 } AS ?d)
    (EXISTS { ex:Rust ex:influencedBy+ ex:ALGOL60 } AS ?i)
  WHERE {}
');
```

```output
┌────────┬──────────┐
│ direct │ indirect │
├────────┼──────────┤
│ 0      │ 1        │
└────────┴──────────┘
```

`?` follows the arrow at most once, so a pattern with `?` also matches the starting point
itself. Rust and the languages that directly influenced it:

```sql
SELECT count(*) AS direct, (SELECT count(*) FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?x WHERE { ex:Rust ex:influencedBy? ?x }')) AS rust_and_direct
FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?x WHERE { ex:Rust ex:influencedBy ?x }');
```

```output
┌────────┬─────────────────┐
│ direct │ rust_and_direct │
├────────┼─────────────────┤
│ 11     │ 12              │
└────────┴─────────────────┘
```

`*` is the same as `+` but also includes the starting point itself (zero steps). The classic use
is a class hierarchy. The paradigms form one: `ex:PurelyFunctional` is a subclass of
`ex:Functional`, which is a subclass of `ex:Declarative`, which is a subclass of `ex:Paradigm`:

```sql
SELECT v0 AS class FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?class WHERE { ex:PurelyFunctional rdfs:subClassOf* ?class }
');
```

```output
┌────────────────────────────────────────┐
│                 class                  │
├────────────────────────────────────────┤
│ http://example.org/pl/PurelyFunctional │
│ http://example.org/pl/Functional       │
│ http://example.org/pl/Declarative      │
│ http://example.org/pl/Paradigm         │
└────────────────────────────────────────┘
```

So "languages with a declarative paradigm" should include the functional, purely functional and
logic languages too. `ex:paradigm/rdfs:subClassOf*` follows a language's paradigm and then any
number of steps up the hierarchy:

```sql
SELECT v0 AS directly_declarative, v1 AS declarative_or_below FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT
    (COUNT(DISTINCT ?direct) AS ?d)
    (COUNT(DISTINCT ?any) AS ?a)
  WHERE {
    { ?direct ex:paradigm ex:Declarative }
    UNION
    { ?any ex:paradigm/rdfs:subClassOf* ex:Declarative }
  }
');
```

```output
┌──────────────────────┬──────────────────────┐
│ directly_declarative │ declarative_or_below │
├──────────────────────┼──────────────────────┤
│ 3                    │ 48                   │
└──────────────────────┴──────────────────────┘
```

Why `COUNT(DISTINCT ...)`? The operators `+` and `*` never return the same end point twice for
one starting point, but `/` and `|` can. Mercury (functional and logic) and XQuery (declarative
and functional) each reach `ex:Declarative` along two routes, so without `DISTINCT` that pattern
has 50 solutions for 48 languages.

The same idea, `a/rdfs:subClassOf*`, finds instances of a class and of all its subclasses: `?x
a/rdfs:subClassOf* ex:ComputerLanguage` matches all 94 languages, programming and query languages
alike.

**Careful with `*` and `?`.** "Zero steps" means "the node itself", which has two surprising
effects. With variables at both ends, every node of the graph matches itself, literals included;
and a fixed starting point matches itself even when it does not occur in the data, so a typo
still returns a row. The function `sparql_value()` used below returns the first value of the
first solution of a query (section 6.4):

```sql
SELECT
  sparql_count('PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
    SELECT * WHERE { ?a rdfs:subClassOf ?b }') AS one_step,
  sparql_count('PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
    SELECT * WHERE { ?a rdfs:subClassOf* ?b }') AS zero_or_more_steps,
  sparql_value('PREFIX ex:   <http://example.org/pl/>
                PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
    SELECT ?c WHERE { ex:NoSuchThing rdfs:subClassOf* ?c }') AS typo;
```

```output
┌──────────┬────────────────────┬───────────────────────────────────┐
│ one_step │ zero_or_more_steps │               typo                │
├──────────┼────────────────────┼───────────────────────────────────┤
│ 16       │ 1236               │ http://example.org/pl/NoSuchThing │
└──────────┴────────────────────┴───────────────────────────────────┘
```

The 16 `rdfs:subClassOf` triples give 26 pairs at one or more steps, plus 1,210 pairs of a node
with itself, one for every subject and object in the default graph. So give a `*` or `?` path a
constant, or a variable that other patterns already bind, at one end, and check the spelling of
constants.

**Exercise 5.12.** How many languages in the dataset were influenced, directly or indirectly, by
Lisp? And which of them appeared before 1970?

<details>
<summary>Solution</summary>

```sql
SELECT sparql_count('
  PREFIX ex: <http://example.org/pl/>
  SELECT DISTINCT ?lang WHERE { ?lang ex:influencedBy+ ex:Lisp }
') AS influenced_by_lisp;
```

```output
┌────────────────────┐
│ influenced_by_lisp │
├────────────────────┤
│ 54                 │
└────────────────────┘
```

```sql
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT DISTINCT ?language ?year
  WHERE {
    ?lang ex:influencedBy+ ex:Lisp ;
          ex:firstAppeared ?year ;
          rdfs:label ?language .
    FILTER (?year < 1970)
  }
  ORDER BY ?year
');
```

```output
┌──────────┬──────┐
│ language │ year │
├──────────┼──────┤
│ ISWIM    │ 1966 │
│ Logo     │ 1967 │
└──────────┴──────┘
```

</details>

### 5.13 Named graphs: GRAPH, FROM and FROM NAMED

Everything so far matched the **default graph**. The fictional surveys and reviews live in named
graphs, and a pattern only looks into them inside `GRAPH`. `GRAPH <iri> { ... }` matches inside
one graph; `GRAPH ?g { ... }` matches in every named graph and binds `?g` to the graph's name.
Patterns outside `GRAPH` still match the default graph, so you can mix the two. The top languages
of the (fictional) 2026 survey, with their names from the default graph:

```sql
SELECT v0 AS language, v1 AS usage_share, v2 AS loved_percent FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?share ?loved
  WHERE {
    GRAPH <http://example.org/graph/survey-2026> {
      ?lang ex:usageShare ?share ;
            ex:lovedPercent ?loved .
    }
    ?lang rdfs:label ?language .
  }
  ORDER BY DESC(?share)
  LIMIT 5
');
```

```output
┌────────────┬─────────────┬───────────────┐
│  language  │ usage_share │ loved_percent │
├────────────┼─────────────┼───────────────┤
│ JavaScript │ 0.609       │ 56.9          │
│ Python     │ 0.561       │ 67.4          │
│ TypeScript │ 0.437       │ 72.8          │
│ Java       │ 0.288       │ 50.5          │
│ C#         │ 0.266       │ 61.1          │
└────────────┴─────────────┴───────────────┘
```

The default graph also describes each survey graph (its year and number of respondents), so a
`GRAPH ?g` pattern can be joined with facts *about* `?g`:

```sql
SELECT v0 AS year, v1 AS respondents, v2 AS rust_share FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?year ?respondents ?share
  WHERE {
    ?g ex:surveyYear ?year ;
       ex:respondents ?respondents .
    GRAPH ?g { ex:Rust ex:usageShare ?share }
  }
  ORDER BY ?year
');
```

```output
┌──────┬─────────────┬────────────┐
│ year │ respondents │ rust_share │
├──────┼─────────────┼────────────┤
│ 2025 │ 48213       │ 0.127      │
│ 2026 │ 51877       │ 0.145      │
└──────┴─────────────┴────────────┘
```

The editorial graph holds reviews as blank nodes. Average rating per language:

```sql
SELECT v0 AS language, v1 AS reviews, v2 AS average_rating FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language (COUNT(?review) AS ?n) (AVG(?rating) AS ?avg)
  WHERE {
    GRAPH <http://example.org/graph/editorial> {
      ?review a ex:Review ;
              ex:about ?lang ;
              ex:rating ?rating .
    }
    ?lang rdfs:label ?language .
  }
  GROUP BY ?language
  ORDER BY DESC(?avg) ?language
  LIMIT 5
');
```

```output
┌──────────┬─────────┬────────────────┐
│ language │ reviews │ average_rating │
├──────────┼─────────┼────────────────┤
│ SQL      │ 1       │ 5.0            │
│ Python   │ 2       │ 4.5            │
│ Rust     │ 2       │ 4.5            │
│ Go       │ 1       │ 4.0            │
│ Haskell  │ 1       │ 4.0            │
└──────────┴─────────┴────────────────┘
```

**FROM and FROM NAMED** change the dataset a query runs against. `FROM <g>` makes graph `<g>`
the query's default graph (several `FROM`s are merged), so the patterns outside `GRAPH` no longer
see the store's own default graph; there are no labels in the survey graph, hence the IRIs:

```sql
SELECT v0 AS language, v1 AS usage_share FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?lang ?share
  FROM <http://example.org/graph/survey-2026>
  WHERE { ?lang ex:usageShare ?share }
  ORDER BY DESC(?share)
  LIMIT 3
');
```

```output
┌──────────────────────────────────┬─────────────┐
│             language             │ usage_share │
├──────────────────────────────────┼─────────────┤
│ http://example.org/pl/JavaScript │ 0.609       │
│ http://example.org/pl/Python     │ 0.561       │
│ http://example.org/pl/TypeScript │ 0.437       │
└──────────────────────────────────┴─────────────┘
```

`FROM NAMED <g>` limits which named graphs `GRAPH ?g` can see:

```sql
SELECT v0 AS graph, v1 AS quads FROM sparql_rows('
  SELECT ?g (COUNT(*) AS ?n)
  FROM NAMED <http://example.org/graph/survey-2025>
  WHERE { GRAPH ?g { ?s ?p ?o } }
  GROUP BY ?g
');
```

```output
┌──────────────────────────────────────┬───────┐
│                graph                 │ quads │
├──────────────────────────────────────┼───────┤
│ http://example.org/graph/survey-2025 │ 84    │
└──────────────────────────────────────┴───────┘
```

A classic trap: as soon as a query has a `FROM` or a `FROM NAMED`, its dataset contains only what
they list. With `FROM NAMED` but no `FROM`, the default graph is empty, so patterns outside
`GRAPH` find nothing; with `FROM` but no `FROM NAMED`, there are no named graphs for `GRAPH` to
find. Use both when you need both:

```sql
SELECT
  sparql_count('SELECT * FROM NAMED <http://example.org/graph/survey-2025>
                WHERE { ?s ?p ?o }') AS outside_graph_with_from_named,
  sparql_count('SELECT * FROM <http://example.org/graph/survey-2025>
                WHERE { GRAPH ?g { ?s ?p ?o } }') AS inside_graph_with_from;
```

```output
┌───────────────────────────────┬────────────────────────┐
│ outside_graph_with_from_named │ inside_graph_with_from │
├───────────────────────────────┼────────────────────────┤
│ 0                             │ 0                      │
└───────────────────────────────┴────────────────────────┘
```

**The union default graph.** Some applications want every pattern to see all data, whatever
graph it is in. The setting `union_default_graph` makes the default graph of every query the
union of the store's default graph and all named graphs. Set it for one call in the options of
`sparql_rows` (a JSON object, the second argument), or for the connection with
`sparql_config()`:

```sql
SELECT
  (SELECT v0 FROM sparql_rows('
     SELECT (COUNT(*) AS ?n) WHERE { ?s <http://example.org/pl/usageShare> ?o }'))
    AS normal,
  (SELECT v0 FROM sparql_rows('
     SELECT (COUNT(*) AS ?n) WHERE { ?s <http://example.org/pl/usageShare> ?o }',
     '{"union_default_graph": 1}'))
    AS with_union;
```

```output
┌────────┬────────────┐
│ normal │ with_union │
├────────┼────────────┤
│ 0      │ 57         │
└────────┴────────────┘
```

```sql
SELECT sparql_config('union_default_graph', 1);
SELECT sparql_value('SELECT (COUNT(*) AS ?n) WHERE { ?s ?p ?o }') AS triples_seen;
SELECT sparql_config('union_default_graph', 0);
```

```output
┌─────────────────────────────────────────┐
│ sparql_config('union_default_graph', 1) │
├─────────────────────────────────────────┤
│ 1                                       │
└─────────────────────────────────────────┘
┌──────────────┐
│ triples_seen │
├──────────────┤
│ 2413         │
└──────────────┘
┌─────────────────────────────────────────┐
│ sparql_config('union_default_graph', 0) │
├─────────────────────────────────────────┤
│ 0                                       │
└─────────────────────────────────────────┘
```

(`sparql_value`, from lesson 5.12, returns the first value of the first solution.) Remember to
switch the setting back, as the last line does: it stays for the rest of the session.

**Exercise 5.13.** Which languages have a lower usage share in the 2026 survey than in 2025?
Show both shares.

<details>
<summary>Solution</summary>

```sql
SELECT v0 AS language, v1 AS share_2025, v2 AS share_2026 FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?language ?s25 ?s26
  WHERE {
    GRAPH <http://example.org/graph/survey-2025> { ?lang ex:usageShare ?s25 }
    GRAPH <http://example.org/graph/survey-2026> { ?lang ex:usageShare ?s26 }
    ?lang rdfs:label ?language .
    FILTER (?s26 < ?s25)
  }
  ORDER BY ?language
');
```

```output
┌────────────┬────────────┬────────────┐
│  language  │ share_2025 │ share_2026 │
├────────────┼────────────┼────────────┤
│ C          │ 0.193      │ 0.187      │
│ C#         │ 0.271      │ 0.266      │
│ C++        │ 0.224      │ 0.218      │
│ Clojure    │ 0.011      │ 0.01       │
│ Dart       │ 0.061      │ 0.057      │
│ Erlang     │ 0.01       │ 0.009      │
│ Fortran    │ 0.009      │ 0.008      │
│ Haskell    │ 0.019      │ 0.018      │
│ Java       │ 0.296      │ 0.288      │
│ JavaScript │ 0.618      │ 0.609      │
│ Kotlin     │ 0.096      │ 0.094      │
│ PHP        │ 0.181      │ 0.169      │
│ Perl       │ 0.024      │ 0.021      │
│ R          │ 0.044      │ 0.041      │
│ Ruby       │ 0.058      │ 0.053      │
│ Scala      │ 0.027      │ 0.024      │
│ Swift      │ 0.052      │ 0.049      │
└────────────┴────────────┴────────────┘
```

</details>

### 5.14 ASK, CONSTRUCT and DESCRIBE

Besides `SELECT` there are three other query forms.

**ASK** answers yes or no: is there at least one solution? From SQL, `sparql_ask()` returns 1 or
0, while `sparql_query()` returns the standard JSON form:

```sql
.mode list
SELECT sparql_ask('PREFIX ex: <http://example.org/pl/>
                   ASK { ex:Rust ex:influencedBy+ ex:ALGOL60 }');
SELECT sparql_query('PREFIX ex: <http://example.org/pl/>
                     ASK { ex:Rust ex:influencedBy ex:Python }');
```

```output
1
{"head":{},"boolean":false}
```

**CONSTRUCT** builds new triples from a template, filled in once per solution. The result is a
graph, not a table, so `sparql_query()` returns it in an RDF format: N-Triples (one triple per
line) by default, or Turtle, or N-Quads:

```sql
SELECT sparql_query('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  CONSTRUCT { ?person ex:designed ?lang }
  WHERE {
    ?lang ex:designer ?person .
    ?person schema:name "Niklaus Wirth" .
  }
');
```

```output
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/ALGOLW> .
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/Pascal> .
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/Modula2> .
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/ObjectPascal> .
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/Oberon> .
```

```sql
SELECT sparql_query('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  CONSTRUCT { ?person ex:designed ?lang }
  WHERE {
    ?lang ex:designer ?person .
    ?person schema:name "Niklaus Wirth" .
  }
', 'turtle');
```

```output
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
<http://example.org/pl/person/niklaus-wirth> <http://example.org/pl/designed> <http://example.org/pl/ALGOLW> , <http://example.org/pl/Pascal> , <http://example.org/pl/Modula2> , <http://example.org/pl/ObjectPascal> , <http://example.org/pl/Oberon> .
```

(The Turtle writer declares the `rdf:`, `rdfs:` and `xsd:` prefixes and writes other IRIs in
full.) CONSTRUCT is how you reshape or export data: the output of one store can be loaded into
another with `sparql_load()`.

**DESCRIBE** returns "what the store knows about" a resource. In sparqlite that is every triple
of the default graph with the resource as subject, plus, recursively, the triples of any blank
nodes those triples point to (a *concise bounded description*):

```sql
SELECT sparql_query('DESCRIBE <http://example.org/pl/person/andrew-kelley>', 'turtle');
.mode box --wordwrap on
```

```output
@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
<http://example.org/pl/person/andrew-kelley> a <http://example.org/pl/Person> ;
	<https://schema.org/name> "Andrew Kelley" .
```

Only the query's default graph is described. To describe things that live in a named graph, such
as the editorial reviews, make that graph the default graph with `FROM` (lesson 5.13):

```sql
SELECT
  sparql_count('PREFIX ex: <http://example.org/pl/>
    DESCRIBE ?review
    WHERE { GRAPH <http://example.org/graph/editorial> { ?review ex:about ex:Rust } }')
    AS with_graph,
  sparql_count('PREFIX ex: <http://example.org/pl/>
    DESCRIBE ?review
    FROM <http://example.org/graph/editorial>
    WHERE { ?review ex:about ex:Rust }') AS with_from;
```

```output
┌────────────┬───────────┐
│ with_graph │ with_from │
├────────────┼───────────┤
│ 0          │ 12        │
└────────────┴───────────┘
```

The two reviews of Rust have six triples each, all in the editorial graph.

Through `sparql_rows`, CONSTRUCT and DESCRIBE give one row per triple, with the subject,
predicate and object in `v0`, `v1` and `v2`; an ASK query gives one row whose `v0` is 1 or 0:

```sql
SELECT v0 AS subject, v1 AS predicate, v2 AS object FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  CONSTRUCT { ?person ex:designed ?lang }
  WHERE { ?lang ex:designer ?person . ?person schema:name "Anders Hejlsberg" . }
');
```

```output
┌───────────────────────────────────────────────┬────────────────────────────────┬────────────────────────────────────┐
│                    subject                    │           predicate            │               object               │
├───────────────────────────────────────────────┼────────────────────────────────┼────────────────────────────────────┤
│ http://example.org/pl/person/anders-hejlsberg │ http://example.org/pl/designed │ http://example.org/pl/ObjectPascal │
│ http://example.org/pl/person/anders-hejlsberg │ http://example.org/pl/designed │ http://example.org/pl/CSharp       │
│ http://example.org/pl/person/anders-hejlsberg │ http://example.org/pl/designed │ http://example.org/pl/TypeScript   │
└───────────────────────────────────────────────┴────────────────────────────────┴────────────────────────────────────┘
```

**Exercise 5.14.** Write a CONSTRUCT query that produces, for every language that Smalltalk
influenced directly, a triple `<that language> ex:inspiredBy ex:Smalltalk`, and print it as
N-Triples. How many triples are there? (Use `sparql_rows` and `count(*)`.)

<details>
<summary>Solution</summary>

```sql
.mode list
SELECT sparql_query('
  PREFIX ex: <http://example.org/pl/>
  CONSTRUCT { ?lang ex:inspiredBy ex:Smalltalk }
  WHERE { ?lang ex:influencedBy ex:Smalltalk }
');
.mode box --wordwrap on
SELECT count(*) AS triples FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  CONSTRUCT { ?lang ex:inspiredBy ex:Smalltalk }
  WHERE { ?lang ex:influencedBy ex:Smalltalk }
');
```

```output
<http://example.org/pl/ObjectiveC> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/ObjectPascal> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Erlang> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Self> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Ruby> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Java> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Groovy> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Scala> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Dart> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .
<http://example.org/pl/Raku> <http://example.org/pl/inspiredBy> <http://example.org/pl/Smalltalk> .

┌─────────┐
│ triples │
├─────────┤
│ 10      │
└─────────┘
```

</details>

### 5.15 SPARQL Update

SPARQL Update changes the data. From SQL, run it with `sparql_update()`, which returns the number
of triples actually added plus the number actually removed (inserting a triple that is already
there counts 0). Because this lesson changes the tutorial database,
it starts a SQLite transaction first and undoes everything at the end with `ROLLBACK` (more about
that in 5.16). Start it now:

```sql
BEGIN;
```

**INSERT DATA** adds triples, written as in Turtle (but with SPARQL's `PREFIX`):

```sql
SELECT sparql_update('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  INSERT DATA {
    ex:Toylang a ex:ProgrammingLanguage ;
               rdfs:label "Toylang"@en ;
               ex:firstAppeared 2026 ;
               ex:influencedBy ex:Python , ex:SPARQL .
  }
') AS changed;
```

```output
┌─────────┐
│ changed │
├─────────┤
│ 5       │
└─────────┘
```

**DELETE DATA** removes exactly the triples given:

```sql
SELECT sparql_update('
  PREFIX ex: <http://example.org/pl/>
  DELETE DATA { ex:Toylang ex:influencedBy ex:SPARQL }
') AS changed;
```

```output
┌─────────┐
│ changed │
├─────────┤
│ 1       │
└─────────┘
```

**INSERT ... WHERE** and **DELETE ... WHERE** compute the triples from a pattern, like SQL's
`INSERT ... SELECT`. Mark the languages from before 1960 as pioneers:

```sql
SELECT sparql_update('
  PREFIX ex: <http://example.org/pl/>
  INSERT { ?lang a ex:Pioneer }
  WHERE  { ?lang ex:firstAppeared ?year . FILTER (?year < 1960) }
') AS changed;
```

```output
┌─────────┐
│ changed │
├─────────┤
│ 3       │
└─────────┘
```

`DELETE { ... } INSERT { ... } WHERE { ... }` does both in one step, which is how you modify
values. The `WHERE` part is evaluated first, then all deletions, then all insertions. Rename
Toylang:

```sql
SELECT sparql_update('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  DELETE { ?lang rdfs:label ?old }
  INSERT { ?lang rdfs:label "Toy Language"@en }
  WHERE  { ?lang rdfs:label ?old . FILTER (STR(?old) = "Toylang") }
') AS changed;
SELECT v0 AS pioneers_and_toys FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name WHERE {
    { ?lang a ex:Pioneer } UNION { ?lang ex:influencedBy ex:Python ; ex:firstAppeared 2026 }
    ?lang rdfs:label ?name .
  }
  ORDER BY ?name
');
```

```output
┌─────────┐
│ changed │
├─────────┤
│ 2       │
└─────────┘
┌───────────────────┐
│ pioneers_and_toys │
├───────────────────┤
│ ALGOL 58          │
│ FLOW-MATIC        │
│ Fortran           │
│ Toy Language      │
└───────────────────┘
```

`DELETE WHERE { pattern }` is the short form when the triples to delete are exactly those that
match. Whole graphs are managed with:

- `CREATE GRAPH <g>`: create an empty named graph;
- `CLEAR GRAPH <g>`: remove its triples, keep the graph; `DROP GRAPH <g>`: remove the graph;
- `CLEAR NAMED` / `DROP NAMED`: the same for all named graphs, leaving the default graph alone;
- `CLEAR DEFAULT`: empty the default graph;
- `CLEAR ALL` / `DROP ALL`: **everything**, the default graph included.

The table `rdf_graph` lists the named graphs:

```sql
SELECT sparql_update('CREATE GRAPH <http://example.org/graph/scratch>') AS created;
SELECT sparql_term(g) AS named_graph FROM rdf_graph;
```

```output
┌─────────┐
│ created │
├─────────┤
│ 0       │
└─────────┘
┌────────────────────────────────────────┐
│              named_graph               │
├────────────────────────────────────────┤
│ <http://example.org/graph/survey-2025> │
│ <http://example.org/graph/survey-2026> │
│ <http://example.org/graph/editorial>   │
│ <http://example.org/graph/scratch>     │
└────────────────────────────────────────┘
```

Creating a graph changes no triples, hence the 0. Add a triple to it, empty it with `CLEAR`, and
remove it with `DROP`:

```sql
SELECT sparql_update('
  INSERT DATA { GRAPH <http://example.org/graph/scratch> {
    <http://example.org/pl/Toylang> <http://example.org/pl/note> "just testing" } }
') AS inserted;
SELECT sparql_update('CLEAR GRAPH <http://example.org/graph/scratch>') AS cleared;
SELECT count(*) AS named_graphs FROM rdf_graph;
SELECT sparql_update('DROP GRAPH <http://example.org/graph/scratch>') AS dropped;
SELECT count(*) AS named_graphs FROM rdf_graph;
```

```output
┌──────────┐
│ inserted │
├──────────┤
│ 1        │
└──────────┘
┌─────────┐
│ cleared │
├─────────┤
│ 1       │
└─────────┘
┌──────────────┐
│ named_graphs │
├──────────────┤
│ 4            │
└──────────────┘
┌─────────┐
│ dropped │
├─────────┤
│ 0       │
└─────────┘
┌──────────────┐
│ named_graphs │
├──────────────┤
│ 3            │
└──────────────┘
```

Inserting into a graph that does not exist yet simply creates it. But `CLEAR` or `DROP` of a
graph that does not exist, and `CREATE` of one that does, are errors unless you add `SILENT`:

```sql
SELECT sparql_update('DROP GRAPH <http://example.org/graph/nothing-here>');
SELECT sparql_update('DROP SILENT GRAPH <http://example.org/graph/nothing-here>') AS changed;
```

```output
Runtime error: sparql: invalid argument: graph <http://example.org/graph/nothing-here> does not exist
┌─────────┐
│ changed │
├─────────┤
│ 0       │
└─────────┘
```

Several operations can go in one request, separated by `;`. Now undo the whole lesson:

```sql
ROLLBACK;
SELECT sparql_ask('ASK { <http://example.org/pl/Toylang> ?p ?o }') AS toylang_exists;
```

```output
┌────────────────┐
│ toylang_exists │
├────────────────┤
│ 0              │
└────────────────┘
```

**Exercise 5.15.** Inside a transaction, delete all reviews written by "Jonas Berg" from the
editorial graph (each review is a blank node with several triples), check how many reviews are
left, and roll back.

<details>
<summary>Solution</summary>

```sql
BEGIN;
SELECT sparql_update('
  PREFIX ex: <http://example.org/pl/>
  DELETE WHERE {
    GRAPH <http://example.org/graph/editorial> { ?review ex:reviewer "Jonas Berg" ; ?p ?o }
  }
') AS triples_deleted;
SELECT sparql_count('
  PREFIX ex: <http://example.org/pl/>
  SELECT ?r WHERE { GRAPH <http://example.org/graph/editorial> { ?r a ex:Review } }
') AS reviews_left;
ROLLBACK;
```

```output
┌─────────────────┐
│ triples_deleted │
├─────────────────┤
│ 18              │
└─────────────────┘
┌──────────────┐
│ reviews_left │
├──────────────┤
│ 11           │
└──────────────┘
```

`DELETE WHERE` deletes every triple of each matching review, because `?p ?o` matches them all.

</details>

### 5.16 Updates and SQLite transactions

sparqlite's data lives in ordinary SQLite tables, so SQLite's transactions cover it:

- Each `sparql_update()` (and `sparql_load()`) call is **atomic**: if it fails half-way, nothing
  it did remains. Internally each call runs in its own savepoint.
- Inside `BEGIN ... COMMIT`, several calls, and ordinary SQL statements, form one transaction.
  `ROLLBACK` undoes all of them, as the previous lesson showed; `COMMIT` makes them permanent.
- Queries see the transaction's own uncommitted changes; other connections see them only after
  `COMMIT`.
- While a write transaction is open, other connections cannot write to the database file (the
  usual SQLite rule). Set `PRAGMA busy_timeout = 5000;` on connections that should wait for a lock
  instead of failing at once.

One restriction is specific to sparqlite: nothing can change the RDF data on a connection while a
`sparql_rows` or `sparql_view` scan on that connection is still open, whether in the same
statement or in another one that has not been read to the end, such as a Python cursor you are
looping over (see [Troubleshooting](#83-troubleshooting)). Read all the results first, then
write.

---

## 6. Using sparqlite from SQL

The course mostly used `sparql_rows`. This section goes through the whole SQL interface, still
in the tutorial-database session. [Section 8.1](#81-sql-functions-and-virtual-tables) lists every
function with its signature.

### 6.1 The pieces at a glance

| You want | Use |
|---|---|
| query results as rows to use in SQL | `sparql_rows(query [, options])`, a table-valued function |
| a SELECT query as a permanent, typed table | `CREATE VIRTUAL TABLE ... USING sparql_view(query, ...)` |
| results as one text document (JSON, XML, CSV, TSV, N-Triples, Turtle) | `sparql_query(query [, format])` |
| one value, a yes/no, or a count | `sparql_value`, `sparql_ask`, `sparql_count` |
| to change data | `sparql_load`, `sparql_load_file`, `sparql_update` |
| to see how a query runs | `sparql_explain(query [, mode])` |
| settings and counters | `sparql_config(key [, value])`, the `sparql_status` table |
| maintenance | `sparql_init`, `sparql_analyze`, `sparql_check`, `sparql_vacuum_terms` |
| term ids and terms | `sparql_term`, `sparql_term_id`, `sparql_term_value`, `sparql_term_kind`, `sparql_term_lex`, `sparql_term_lang`, `sparql_term_datatype` |
| the SPARQL-to-SQL translator (for comparison) | `sparql_query_sql`, `sparql_count_sql`, `sparql_to_sql` |

### 6.2 sparql_rows and the SQL type of each value

`sparql_rows(query)` returns one row per solution (one row per triple for CONSTRUCT and
DESCRIBE, a single row for ASK) with the columns described in [2.2](#22-the-first-query): `n`,
`v0`-`v15`, `t0`-`t15`, `id0`-`id15`, `vars` and `json`. It also has two hidden columns, `query`
and `opts`, which are what the arguments set: `sparql_rows('...')` is SQLite shorthand for
`SELECT ... FROM sparql_rows WHERE query = '...'`.

The `v` columns convert each RDF term to the closest SQL value. Here is every kind of term,
with its N-Triples form (`t0`), the value (`v0`) and SQLite's `typeof()` of it:

```sql
SELECT t0 AS term, v0 AS value, typeof(v0) AS sql_type FROM sparql_rows('
  PREFIX ex:  <http://example.org/pl/>
  PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
  SELECT ?x WHERE {
    { VALUES ?x { ex:Go "Go" "Go"@en 42 4.2 4.2e1 true
                  "2026-10-15"^^xsd:date "2026-10-15T12:30:00Z"^^xsd:dateTime
                  "99999999999999999999"^^xsd:integer UNDEF } }
    UNION
    { GRAPH <http://example.org/graph/editorial> { ?x ex:about ex:Go } }
  }
');
```

```output
┌────────────────────────────────────────────────────────────┬──────────────────────────┬──────────┐
│                            term                            │          value           │ sql_type │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ <http://example.org/pl/Go>                                 │ http://example.org/pl/Go │ text     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "Go"                                                       │ Go                       │ text     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "Go"@en                                                    │ Go                       │ text     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "42"^^<http://www.w3.org/2001/XMLSchema#integer>           │ 42                       │ integer  │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "4.2"^^<http://www.w3.org/2001/XMLSchema#decimal>          │ 4.2                      │ real     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "4.2e1"^^<http://www.w3.org/2001/XMLSchema#double>         │ 42.0                     │ real     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "true"^^<http://www.w3.org/2001/XMLSchema#boolean>         │ 1                        │ integer  │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "2026-10-15"^^<http://www.w3.org/2001/XMLSchema#date>      │ 2026-10-15               │ text     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "2026-10-15T12:30:00Z"^^<http://www.w3.org/2001/XMLSchema# │ 2026-10-15T12:30:00Z     │ text     │
│ dateTime>                                                  │                          │          │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ "99999999999999999999"^^<http://www.w3.org/2001/XMLSchema# │ 99999999999999999999     │ text     │
│ integer>                                                   │                          │          │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│                                                            │                          │ null     │
├────────────────────────────────────────────────────────────┼──────────────────────────┼──────────┤
│ _:b5                                                       │ _:b5                     │ text     │
└────────────────────────────────────────────────────────────┴──────────────────────────┴──────────┘
```

| RDF term | SQL value |
|---|---|
| IRI | TEXT, without the angle brackets |
| blank node | TEXT `_:label` |
| string, language-tagged string | TEXT (the tag is only in `t*`) |
| `xsd:integer` and its sub-types | INTEGER; TEXT if it does not fit in 64 bits |
| `xsd:decimal`, `xsd:double`, `xsd:float` | REAL |
| `xsd:boolean` | INTEGER 1 or 0 |
| dates and times | TEXT in ISO 8601 form, which SQLite's date functions understand |
| any other datatype | TEXT, the lexical form |
| unbound | NULL |

Use `t*` when the difference between `"Go"` and `"Go"@en`, or between `42` and `"42"`, matters.
The `id*` columns hold the term ids of 2.4 (explained in 6.5); they are cheap to read (no
decoding) and are a good key for joins between two `sparql_rows` calls on the same database.

A blank-node label such as `_:b5` cannot be used to find that node again. In a query, `_:b5`
acts like a variable that is not returned, so this pattern matches every triple of the default
graph:

```sql
SELECT sparql_count('SELECT * WHERE { _:b5 ?p ?o }') AS matches;
```

```output
┌─────────┐
│ matches │
├─────────┤
│ 2146    │
└─────────┘
```

To get back to a blank node, start from a named resource it hangs off, as the review queries of
lesson 5.13 do, or keep its `id*` value and use it on the SQL side, for example with the `__id`
columns of 6.7.

The optional second argument is a JSON object with settings for this one call: `engine`,
`timeout_ms`, `max_memory_mb`, `union_default_graph`, `join_policy`, `reorder`, `probe_counts`
and `probe_cap` (see 6.10), for example `sparql_rows(query, '{"timeout_ms": 500}')`.

`sparql_rows` reads its results lazily, a batch at a time, and only converts the columns the SQL
statement actually uses: `SELECT count(*) FROM sparql_rows(...)` converts nothing.

### 6.3 Results as text: sparql_query

`sparql_query(query, format)` returns the whole result as one text value in a standard format:
`json` (the default), `xml`, `csv` or `tsv` for SELECT and ASK queries; `ntriples` (the default),
`nquads` or `turtle` for CONSTRUCT and DESCRIBE. Switch to list mode for long text:

```sql
.mode list
SELECT sparql_query('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name ?year
  WHERE { VALUES ?lang { ex:Go ex:Rust } ?lang rdfs:label ?name ; ex:firstAppeared ?year }
  ORDER BY ?year
', 'csv');
```

```output
name,year
Go,2009
Rust,2012
```

CSV gives plain values; TSV writes RDF terms, so the language tags and datatypes survive (numbers
are written in their short form):

```sql
SELECT sparql_query('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name ?year
  WHERE { VALUES ?lang { ex:Go ex:Rust } ?lang rdfs:label ?name ; ex:firstAppeared ?year }
  ORDER BY ?year
', 'tsv');
```

```output
?name	?year
"Go"@en	2009
"Rust"@en	2012
```

```sql
SELECT sparql_query('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name ?year
  WHERE { VALUES ?lang { ex:Go ex:Rust } ?lang rdfs:label ?name ; ex:firstAppeared ?year }
  ORDER BY ?year
', 'xml');
```

```output
<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="name"/><variable name="year"/></head><results><result><binding name="name"><literal xml:lang="en">Go</literal></binding><binding name="year"><literal datatype="http://www.w3.org/2001/XMLSchema#integer">2009</literal></binding></result><result><binding name="name"><literal xml:lang="en">Rust</literal></binding><binding name="year"><literal datatype="http://www.w3.org/2001/XMLSchema#integer">2012</literal></binding></result></results></sparql>
```

These are the formats a SPARQL endpoint returns, so `sparql_query()` is the function to use when
you build one: pass the query through and send the text back.

### 6.4 Single values: sparql_value, sparql_ask and sparql_count

- `sparql_value(query)` returns the first variable of the first solution (use `ORDER BY` to say
  which solution is first), converted like the `v` columns, or NULL if there is none. For an ASK
  query it returns 1 or 0.
- `sparql_ask(query)` returns 1 if the query has any solution, else 0.
- `sparql_count(query)` returns the number of solutions (triples for CONSTRUCT). It runs the
  query completely but converts nothing, so it is the fastest way to count.

```sql
.mode box --wordwrap on
SELECT
  sparql_value('PREFIX ex: <http://example.org/pl/>
                SELECT ?year WHERE { ex:Rust ex:firstAppeared ?year }') AS rust_year,
  sparql_value('PREFIX ex: <http://example.org/pl/>
                SELECT ?year WHERE { ex:Rust ex:lastAppeared ?year }') AS no_value,
  sparql_ask('PREFIX ex: <http://example.org/pl/>
              ASK { ex:Rust ex:influencedBy ex:Haskell }') AS haskell_influenced_rust,
  sparql_count('PREFIX ex: <http://example.org/pl/>
                SELECT ?lang WHERE { ?lang ex:typing ex:StaticTyping }') AS statically_typed;
```

```output
┌───────────┬──────────┬─────────────────────────┬──────────────────┐
│ rust_year │ no_value │ haskell_influenced_rust │ statically_typed │
├───────────┼──────────┼─────────────────────────┼──────────────────┤
│ 2012      │          │ 1                       │ 39               │
└───────────┴──────────┴─────────────────────────┴──────────────────┘
```

They are ordinary scalar functions, so they work where SQL allows an expression, for example in
`WHERE`, `CASE` or `UPDATE ... SET`, and in views and triggers (see 6.9).

### 6.5 Terms and ids

Section 2.4 showed that `rdf_quad` stores every term as an integer *term id*. This is how the ids
are made.

The dictionary table `rdf_term` has one row per stored term. The rows below 1024 are reserved for
built-in IRIs such as `rdf:type` and the XML Schema datatypes (35 of them are in use); the data's
own terms get row numbers from 1024 on. The id of a dictionary term is its row number times 16
plus a *tag* in the low 4 bits that says what kind of term it is (1 = IRI):

```sql
SELECT id AS row, tag, lex, id * 16 + tag AS term_id
FROM rdf_term
WHERE lex IN ('http://example.org/pl/Go', 'http://example.org/pl/Rust')
ORDER BY id;
```

```output
┌──────┬─────┬────────────────────────────┬─────────┐
│ row  │ tag │            lex             │ term_id │
├──────┼─────┼────────────────────────────┼─────────┤
│ 1831 │ 1   │ http://example.org/pl/Go   │ 29297   │
│ 1840 │ 1   │ http://example.org/pl/Rust │ 29441   │
└──────┴─────┴────────────────────────────┴─────────┘
```

Many values need no dictionary row, because their id encodes the value itself: integers and
decimals of ordinary size (up to about 17 digits) written in their usual form, booleans, dates
and dateTimes (to the millisecond), strings of up to 7 bytes without a language tag, and blank
nodes. Everything else is stored in `rdf_term`: IRIs, language-tagged strings, longer strings,
doubles and floats, very large numbers, numbers written in an unusual form such as
`"01"^^xsd:integer`, and literals of all other datatypes.

The helper functions, all taking an id:

| Function | Returns |
|---|---|
| `sparql_term(id)` | the term in N-Triples syntax |
| `sparql_term_value(id)` | the SQL value, as in the `v` columns |
| `sparql_term_kind(id)` | `iri`, `literal` or `bnode` |
| `sparql_term_lex(id)` | the IRI, the blank node label, or a literal's text |
| `sparql_term_lang(id)` | a literal's language tag, else NULL |
| `sparql_term_datatype(id)` | a literal's datatype IRI, else NULL |
| `sparql_term_id(text)` | the reverse: the id of a term written in N-Triples syntax (bare numbers and `true`/`false` are accepted, as in Turtle; prefixed names such as `ex:Go` are not), or NULL if the store does not contain it |

```sql
SELECT id0 AS id,
       sparql_term(id0) AS term,
       sparql_term_kind(id0) AS kind,
       sparql_term_lex(id0) AS lex,
       sparql_term_lang(id0) AS lang
FROM sparql_rows('SELECT ?label WHERE { <http://example.org/pl/Lua> ?p ?label .
                                        FILTER (isLiteral(?label) && LANG(?label) != "") }
                         ORDER BY ?label');
```

```output
┌───────┬──────────────────────────────────────────────────────────────┬─────────┬─────────────────────────────────────────────────────────────┬──────┐
│  id   │                             term                             │  kind   │                             lex                             │ lang │
├───────┼──────────────────────────────────────────────────────────────┼─────────┼─────────────────────────────────────────────────────────────┼──────┤
│ 27764 │ "'Lua' means 'moon' in Portuguese."@en                       │ literal │ 'Lua' means 'moon' in Portuguese.                           │ en   │
├───────┼──────────────────────────────────────────────────────────────┼─────────┼─────────────────────────────────────────────────────────────┼──────┤
│ 27748 │ "Lightweight, embeddable scripting language from PUC-Rio in  │ literal │ Lightweight, embeddable scripting language from PUC-Rio in  │ en   │
│       │ Brazil."@en                                                  │         │ Brazil.                                                     │      │
├───────┼──────────────────────────────────────────────────────────────┼─────────┼─────────────────────────────────────────────────────────────┼──────┤
│ 27732 │ "Lua"@en                                                     │ literal │ Lua                                                         │ en   │
└───────┴──────────────────────────────────────────────────────────────┴─────────┴─────────────────────────────────────────────────────────────┴──────┘
```

`sparql_term_id()` only looks terms up and never adds them; it returns NULL for a term that is not
in the store. Values with inline ids (integers, decimals, booleans, dates, short strings) always
have one, whether the data uses them or not. A double such as `4.2e1` needs a dictionary row, and
this store has none:

```sql
SELECT sparql_term_id('<http://example.org/pl/Rust>') AS rust,
       sparql_term_id('<http://example.org/pl/NoSuchLanguage>') AS missing,
       sparql_term_id('2012') AS the_number_2012,
       sparql_term_id('4.2e1') AS a_double;
```

```output
┌───────┬─────────┬─────────────────┬──────────┐
│ rust  │ missing │ the_number_2012 │ a_double │
├───────┼─────────┼─────────────────┼──────────┤
│ 29441 │         │ 32198           │          │
└───────┴─────────┴─────────────────┴──────────┘
```

A prefixed name is an error rather than NULL, because no `PREFIX` applies outside a SPARQL query.
The message says what to write instead:

```sql
SELECT sparql_term_id('ex:Go');
```

```output
Runtime error: sparql: invalid argument: not an N-Triples term: "ex:Go" (ex:Go is a prefixed name, and N-Triples has none: no PREFIX applies outside a SPARQL query, so write the full IRI in angle brackets: the namespace IRI that ex: stands for, followed by Go (if ex: is <http://example.org/>, that is <http://example.org/Go>))
```

In this tutorial `ex:` stands for `<http://example.org/pl/>`, so the term is
`<http://example.org/pl/Go>`. For the standard prefixes `rdf:`, `rdfs:`, `xsd:` and `owl:` the
message gives the full IRI itself.

### 6.6 Loading data: sparql_load and sparql_load_file

`sparql_load(data, format, base, graph)` parses RDF text (or a BLOB) and returns the number of new
triples or quads. Only `data` is required:

- `format`: `turtle`/`ttl` (the default), `ntriples`/`nt`, `nquads`/`nq`, `trig`,
  `rdfxml`/`xml`, `n3` or `jsonld`.
- `base`: the base IRI for relative IRIs such as `<alice>` in the data.
- `graph`: an IRI; the triples go into that named graph instead of the default graph (for
  triple formats).

All of it happens in one transaction step: a parse error part-way leaves the store unchanged.
`readfile()`, which the sqlite3 shell provides, reads a file into a BLOB, so
`SELECT sparql_load(readfile('my-data.ttl'))` loads a file from the shell.

`sparql_load_file(path, format, base, graph)` reads the file itself, guesses the format from the
extension (`.ttl`, `.nt`, `.nq`, `.trig`, `.rdf`, `.xml`, `.owl`, `.n3`, `.jsonld`, `.json`) and
uses the file's `file://` URL as the base. Because a function that reads arbitrary files is a
risk when the SQL comes from someone else, it is switched off until you allow it on the
connection; the same setting governs SPARQL's `LOAD <file:...>`:

```sql
SELECT sparql_load_file('tutorial/data/languages.ttl');
```

```output
Runtime error: sparql: invalid argument: sparql_load_file is disabled; enable it with sparql_config('allow_files', 1)
```

```sql
SELECT sparql_config('allow_files', 1) AS allow_files;
BEGIN;
SELECT sparql_load_file('tutorial/data/languages.ttl', NULL, NULL,
                        'http://example.org/graph/copy') AS new_quads;
SELECT count(*) AS named_graphs FROM rdf_graph;
ROLLBACK;
SELECT sparql_config('allow_files', 0) AS allow_files;
```

```output
┌─────────────┐
│ allow_files │
├─────────────┤
│ 1           │
└─────────────┘
┌───────────┐
│ new_quads │
├───────────┤
│ 2126      │
└───────────┘
┌──────────────┐
│ named_graphs │
├──────────────┤
│ 4            │
└──────────────┘
┌─────────────┐
│ allow_files │
├─────────────┤
│ 0           │
└─────────────┘
```

Relative paths are relative to the current directory of the process, here the shell's.
`sparql_config('allow_files', ...)` cannot be called from views or triggers stored in the
database (TEMP ones are fine, see 6.9), and it cannot be set per call.

### 6.7 sparql_view: a SPARQL query as a table

`sparql_rows` needs the query every time and names its columns `v0`, `v1`, .... A **`sparql_view`**
virtual table stores a SELECT query once and names the columns after its variables:

```sql
CREATE VIRTUAL TABLE temp.languages USING sparql_view('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?lang ?name ?year ?homepage
  WHERE {
    ?lang a/rdfs:subClassOf* ex:ComputerLanguage ;
          rdfs:label ?name .
    OPTIONAL { ?lang ex:firstAppeared ?year }
    OPTIONAL { ?lang schema:url ?homepage }
  }
');
SELECT name, year, homepage FROM languages WHERE year >= 2015 ORDER BY year, name;
```

```output
┌────────┬──────┬────────────────────────────────────────────────┐
│  name  │ year │                    homepage                    │
├────────┼──────┼────────────────────────────────────────────────┤
│ Raku   │ 2015 │ https://raku.org/                              │
│ Gleam  │ 2016 │ https://gleam.run/                             │
│ Zig    │ 2016 │ https://ziglang.org/                           │
│ Carbon │ 2022 │ https://github.com/carbon-language/carbon-lang │
│ Mojo   │ 2023 │                                                │
└────────┴──────┴────────────────────────────────────────────────┘
```

Every query against the table runs the SPARQL query (there is no stored copy), so the table is
always current. Besides the value columns there are hidden ones, which `SELECT *` does not show:
for each variable `x`, `x__term` (the N-Triples text, like `t*`) and `x__id` (the term id), and
`opts`:

```sql
SELECT name, hidden FROM pragma_table_xinfo('languages');
```

```output
┌────────────────┬────────┐
│      name      │ hidden │
├────────────────┼────────┤
│ lang           │ 0      │
│ name           │ 0      │
│ year           │ 0      │
│ homepage       │ 0      │
│ lang__term     │ 1      │
│ name__term     │ 1      │
│ year__term     │ 1      │
│ homepage__term │ 1      │
│ lang__id       │ 1      │
│ name__id       │ 1      │
│ year__id       │ 1      │
│ homepage__id   │ 1      │
│ opts           │ 1      │
└────────────────┴────────┘
```

**Pushdown.** An equality test on an `__term` or `__id` column is handed to the SPARQL engine,
which then starts from that one term instead of producing every row and letting SQLite filter
them. Both of these return Rust, but the first one asks the engine for Rust only, while the
second makes it produce all 94 languages:

```sql
SELECT name, year FROM languages WHERE lang__term = '<http://example.org/pl/Rust>';
SELECT name, year FROM languages WHERE name = 'Rust';
```

```output
┌──────┬──────┐
│ name │ year │
├──────┼──────┤
│ Rust │ 2012 │
└──────┴──────┘
┌──────┬──────┐
│ name │ year │
├──────┼──────┤
│ Rust │ 2012 │
└──────┴──────┘
```

The `__id` columns work the same way with term ids, which suits joins with values that came from
another sparqlite query or from `sparql_term_id()`:

```sql
SELECT name, lang__id FROM languages WHERE lang__id = sparql_term_id('<http://example.org/pl/Go>');
```

```output
┌──────┬──────────┐
│ name │ lang__id │
├──────┼──────────┤
│ Go   │ 29297    │
└──────┴──────────┘
```

Only `=` on `__term` and `__id` columns is pushed down; value columns never are, because a
value such as `'Rust'` could stand for several different terms. `__term` must be the exact
N-Triples spelling.

Settings for the view go after the query as `key=value` arguments, with the same keys as the
`sparql_rows` options, e.g. `USING sparql_view('SELECT ...', timeout_ms=2000, engine=native)`.
The query must be a SELECT, and two variables must not differ only in case.

`temp.` puts the table in the temporary schema: it disappears when the connection closes. Without
`temp.` the definition is stored in the database file (see 6.9).

### 6.8 Joining SPARQL results with SQL tables

Since `sparql_rows` and `sparql_view` are tables, they join with ordinary tables. The first two
examples use the `languages` table from 6.7; if you have restarted the shell since then, run its
`CREATE VIRTUAL TABLE temp.languages ...` statement again first. `job_postings` (fictional, see
4.2) stores the language as an IRI in `language_iri`, which is exactly the value of the view's
`lang` column:

```sql
SELECT j.language, count(*) AS postings, l.year AS first_appeared
FROM job_postings AS j
JOIN languages AS l ON l.lang = j.language_iri
GROUP BY j.language
ORDER BY postings DESC, j.language
LIMIT 5;
```

```output
┌────────────┬──────────┬────────────────┐
│  language  │ postings │ first_appeared │
├────────────┼──────────┼────────────────┤
│ Python     │ 37       │ 1991           │
│ TypeScript │ 33       │ 2012           │
│ Java       │ 27       │ 1995           │
│ JavaScript │ 27       │ 1995           │
│ C++        │ 22       │ 1985           │
└────────────┴──────────┴────────────────┘
```

A `LEFT JOIN` finds postings whose language is not in the graph:

```sql
SELECT j.language, count(*) AS postings
FROM job_postings AS j
LEFT JOIN languages AS l ON l.lang = j.language_iri
WHERE l.lang IS NULL
GROUP BY j.language;
```

```output
┌──────────┬──────────┐
│ language │ postings │
├──────────┼──────────┤
│ ABAP     │ 3        │
└──────────┴──────────┘
```

The average advertised salary per paradigm combines a SPARQL path with an SQL aggregate. A
posting counts once for each paradigm of its language (Python is procedural, functional and
class-based), so the counts add up to more than 300:

```sql
SELECT p.v1 AS paradigm,
       count(*) AS postings,
       CAST(round(avg(j.salary_usd)) AS INTEGER) AS avg_salary_usd
FROM job_postings AS j
JOIN sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?lang ?paradigm
  WHERE {
    ?lang ex:paradigm ?p .
    ?p rdfs:label ?paradigm .
    FILTER (LANG(?paradigm) = "en")
  }
') AS p ON p.v0 = j.language_iri
GROUP BY paradigm
ORDER BY avg_salary_usd DESC;
```

```output
┌───────────────────────────────┬──────────┬────────────────┐
│           paradigm            │ postings │ avg_salary_usd │
├───────────────────────────────┼──────────┼────────────────┤
│ purely functional programming │ 4        │ 119750         │
│ array programming             │ 13       │ 114731         │
│ object-oriented programming   │ 47       │ 114426         │
│ concurrent programming        │ 56       │ 106902         │
│ class-based programming       │ 155      │ 106342         │
│ functional programming        │ 207      │ 105249         │
│ prototype-based programming   │ 32       │ 101266         │
│ procedural programming        │ 157      │ 100366         │
│ declarative programming       │ 8        │ 80875          │
└───────────────────────────────┴──────────┴────────────────┘
```

SPARQL results work in common table expressions and with window functions like any other rows.
The three best-paid languages within each typing discipline:

```sql
WITH typing AS (
  SELECT v0 AS lang, v1 AS typing FROM sparql_rows('
    PREFIX ex:   <http://example.org/pl/>
    PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
    SELECT ?lang ?typing WHERE { ?lang ex:typing ?t . ?t rdfs:label ?typing }')
),
per_language AS (
  SELECT t.typing, j.language, count(*) AS postings, avg(j.salary_usd) AS avg_salary
  FROM job_postings AS j JOIN typing AS t ON t.lang = j.language_iri
  GROUP BY t.typing, j.language
),
ranked AS (
  SELECT *, rank() OVER (PARTITION BY typing ORDER BY avg_salary DESC) AS rank
  FROM per_language
)
SELECT typing, rank, language, postings, CAST(round(avg_salary) AS INTEGER) AS avg_salary_usd
FROM ranked WHERE rank <= 3
ORDER BY typing, rank;
```

```output
┌────────────────┬──────┬────────────┬──────────┬────────────────┐
│     typing     │ rank │  language  │ postings │ avg_salary_usd │
├────────────────┼──────┼────────────┼──────────┼────────────────┤
│ dynamic typing │ 1    │ Julia      │ 2        │ 175000         │
│ dynamic typing │ 2    │ Erlang     │ 2        │ 150000         │
│ dynamic typing │ 3    │ Lua        │ 5        │ 123400         │
│ gradual typing │ 1    │ TypeScript │ 33       │ 110788         │
│ static typing  │ 1    │ OCaml      │ 4        │ 125750         │
│ static typing  │ 2    │ F#         │ 3        │ 124833         │
│ static typing  │ 3    │ Zig        │ 2        │ 124750         │
└────────────────┴──────┴────────────┴──────────┴────────────────┘
```

In these joins SQLite reads the SPARQL results once and looks up the matching postings through
the index on `job_postings.language_iri`. `EXPLAIN QUERY PLAN` shows the order it chose; the
`SCAN ... VIRTUAL TABLE` line should be the outer loop, otherwise SQLite may run the SPARQL query
again for every row of the other table. If it is not, copy the SPARQL results into a temporary
table first.

You can also build the SPARQL text with SQL, for example from a table of queries
(`FROM my_queries AS q, sparql_rows(q.text)`) or with `printf()`. If any part comes from user
input, check it first: it is code, exactly like SQL built from strings.

### 6.9 Saving queries as views

A SQL view over `sparql_rows` saves a query under a name. `CREATE TEMP VIEW` lasts until the
connection closes; `CREATE VIEW` stores it in the database file:

```sql
CREATE VIEW language_years AS
SELECT v0 AS language, v1 AS year FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name ?year WHERE { ?lang ex:firstAppeared ?year ; rdfs:label ?name }
');
SELECT year / 10 * 10 AS decade, count(*) AS languages
FROM language_years GROUP BY decade ORDER BY decade;
```

```output
┌────────┬───────────┐
│ decade │ languages │
├────────┼───────────┤
│ 1950   │ 3         │
│ 1960   │ 15        │
│ 1970   │ 14        │
│ 1980   │ 18        │
│ 1990   │ 15        │
│ 2000   │ 13        │
│ 2010   │ 13        │
│ 2020   │ 2         │
└────────┴───────────┘
```

The scalar functions of 6.4 work in stored views too:

```sql
PRAGMA trusted_schema;
CREATE VIEW language_stats AS
SELECT sparql_count('PREFIX ex: <http://example.org/pl/>
                     SELECT ?lang WHERE { ?lang ex:firstAppeared ?year }') AS dated_languages,
       sparql_value('PREFIX ex: <http://example.org/pl/>
                     SELECT (MIN(?year) AS ?first) WHERE { ?lang ex:firstAppeared ?year }') AS earliest;
SELECT * FROM language_stats;
```

```output
┌────────────────┐
│ trusted_schema │
├────────────────┤
│ 0              │
└────────────────┘
┌─────────────────┬──────────┐
│ dated_languages │ earliest │
├─────────────────┼──────────┤
│ 93              │ 1955     │
└─────────────────┴──────────┘
```

Things to know about stored (non-`temp`) views and `sparql_view` tables:

- Any connection that uses them must load the extension first. Without it, only the objects that
  need it fail (`no such table: main.sparql_rows`, `no such module: sparql_view`); the rest of the
  database works normally.
- With `PRAGMA trusted_schema` off, as in the sqlite3 shell used here (the `0` above), SQLite
  lets views and triggers stored in the database call only functions that are marked as harmless
  (`SQLITE_INNOCUOUS`). sparqlite marks every function that only reads that way: `sparql_rows`,
  `sparql_query`, `sparql_value`, `sparql_count`, `sparql_ask`, `sparql_explain`, the
  `sparql_term*` functions and the one-argument `sparql_config`. So stored views and triggers
  can query the store whatever that setting says.
- Functions that change data (`sparql_load`, `sparql_update`, `sparql_init`, ...) and
  `sparql_config` with two arguments are never allowed in stored views or triggers (`unsafe use
  of sparql_update()`, see 8.3). Temporary views and triggers are created by the connection
  itself, and SQLite allows them there.

```sql
DROP VIEW language_years;
DROP VIEW language_stats;
```

### 6.10 Settings: sparql_config and sparql_status

`sparql_config(key)` returns a setting and `sparql_config(key, value)` changes it for this
connection (it is not stored in the database). The `sparql_status` table shows all of them,
together with counters:

```sql
SELECT key, value FROM sparql_status WHERE key LIKE 'config.%';
```

```output
┌────────────────────────────┬────────┐
│            key             │ value  │
├────────────────────────────┼────────┤
│ config.engine              │ native │
│ config.timeout_ms          │ 0      │
│ config.max_memory_mb       │ 1024   │
│ config.union_default_graph │ 0      │
│ config.decode_cache_mb     │ 64     │
│ config.join_policy         │ auto   │
│ config.reorder             │ 1      │
│ config.probe_counts        │ 32     │
│ config.probe_cap           │ 1000   │
│ config.allow_files         │ 0      │
│ config.reader              │ auto   │
│ config.page_cache_mb       │ 128    │
└────────────────────────────┴────────┘
```

| Key | Default | Meaning |
|---|---|---|
| `engine` | `native` | `native` runs sparqlite's engine; `sql` translates the query to one SQL statement instead (see 6.12) |
| `timeout_ms` | `0` | stop a query after this many milliseconds; 0 = never |
| `max_memory_mb` | `1024` | memory budget for one query's intermediate results (hash tables, sorts) |
| `union_default_graph` | `0` | 1 = the default graph is the union of all graphs (lesson 5.13) |
| `join_policy` | `auto` | steer the planner: `hash`, `bind` or `merge` prefer that kind of join (section 7.3); `nosql` is `auto` without handing chains of joins to SQLite as one SQL statement |
| `reorder` | `1` | 0 = join the patterns in the order written instead of choosing an order |
| `probe_counts`, `probe_cap` | `32`, `1000` | how much the planner may count in the indexes while planning |
| `decode_cache_mb` | `64` | cache of decoded terms |
| `allow_files` | `0` | 1 = allow `sparql_load_file` and `LOAD <file:...>` |
| `reader` | `auto` | how query scans read the indexes: `auto`, `btree` or `sql` (section 7.4) |
| `page_cache_mb` | `128` | page cache of the direct B-tree reader |

The keys `engine`, `timeout_ms`, `max_memory_mb`, `union_default_graph`, `join_policy`,
`reorder`, `probe_counts` and `probe_cap` can also be set for one call: in the `sparql_rows`
options and as `sparql_view` arguments. A limit set for one call can only make the connection's
limit stricter: the smaller `timeout_ms` or `max_memory_mb` wins, and `"timeout_ms": 0` does not
switch off a timeout set for the connection. The others (`decode_cache_mb`, `allow_files`,
`reader`, `page_cache_mb`) are per connection only. The other rows of `sparql_status` count work
done by this connection (`rows_emitted`, `terms_decoded`, cache hits, ...) and describe the last
change:

```sql
SELECT key, value FROM sparql_status
WHERE key IN ('version', 'sqlite_version', 'schema_state', 'open_cursors', 'last_op');
```

```output
┌────────────────┬──────────────────────────┐
│      key       │          value           │
├────────────────┼──────────────────────────┤
│ version        │ 0.1.0                    │
│ sqlite_version │ 3.45.1                   │
│ schema_state   │ ready (schema_version 1) │
│ open_cursors   │ 0                        │
│ last_op        │ load                     │
└────────────────┴──────────────────────────┘
```

### 6.11 Maintenance: sparql_analyze, sparql_check and sparql_vacuum_terms

- `sparql_analyze()` recomputes the statistics that the query planner uses to choose join orders
  and returns a summary as JSON. Loads run it automatically when the store has grown by more than
  25% since the last analysis; run it yourself after many small updates. (It is separate from
  SQLite's own `ANALYZE`, which helps SQLite plan SQL statements, including those of the SQL
  translator in 6.12.)
- `sparql_check()` verifies the store's internal consistency and returns `ok: ...` or a list of
  problems.
- `sparql_vacuum_terms()` deletes dictionary entries that no triple uses any more, for example
  after deleting data, and returns how many it removed. Deleting triples never removes terms by
  itself.

<!-- verify: ignore-timing -->
```sql
.mode list
SELECT sparql_analyze();
SELECT sparql_check();
.mode box --wordwrap on
```

```output
{"analyzed_quads":2413,"default_graph":{"characteristic_sets":87,"heavy_hitters":1,"objects":1173,"predicates":31,"subjects":299,"triples":2146},"ms":4,"named_graphs":{"characteristic_sets":5,"heavy_hitters":0,"objects":206,"predicates":10,"subjects":46,"triples":267}}
ok: 963 terms, 2413 quads, 3 named graphs
```

(`sparql_check` counts all rows of `rdf_term`: the 928 dictionary terms of the data that the load
summary in 4.1 reported, plus the 35 built-in rows of 6.5.)

To see `sparql_vacuum_terms` at work, delete the 67 `rdfs:comment` notes (long, unique strings,
so each has a dictionary entry) inside a transaction, vacuum, check, and roll back:

```sql
BEGIN;
SELECT sparql_update('
  DELETE WHERE { ?s <http://www.w3.org/2000/01/rdf-schema#comment> ?comment }
') AS deleted;
SELECT sparql_vacuum_terms() AS terms_removed;
SELECT sparql_check() AS check_result;
ROLLBACK;
```

```output
┌─────────┐
│ deleted │
├─────────┤
│ 67      │
└─────────┘
┌───────────────┐
│ terms_removed │
├───────────────┤
│ 67            │
└───────────────┘
┌───────────────────────────────────────────┐
│               check_result                │
├───────────────────────────────────────────┤
│ ok: 896 terms, 2346 quads, 3 named graphs │
└───────────────────────────────────────────┘
```

### 6.12 The SQL translator: sparql_query_sql, sparql_count_sql and sparql_to_sql

sparqlite contains a second, simpler way to run a query: translate the whole SPARQL query into
one SQL statement over `rdf_quad` and let SQLite run it. It exists as a baseline, to compare
against the native engine and to show what SPARQL means in SQL terms. `sparql_to_sql()` shows the
statement:

```sql
.mode list
SELECT sparql_to_sql('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX schema: <https://schema.org/>
  SELECT ?name WHERE { ?lang ex:designer ?person . ?person schema:name ?name }
');
```

```output
SELECT t2.o AS v2 FROM main.rdf_quad AS t1, main.rdf_quad AS t2 WHERE t1.g = 0 AND t1.p = ?1 AND t2.g = 0 AND t1.o = t2.s AND t2.p = ?2
```

Each triple pattern becomes one copy of `rdf_quad` (`t1`, `t2`), a shared variable becomes a
join condition, and constants become parameters (`?1`, `?2`). `sparql_explain(query, 'sql')` shows
the same text with the parameters' values. `sparql_query_sql()` and `sparql_count_sql()` are
`sparql_query()` and `sparql_count()` with the translator, and the setting `engine = sql` (for
the connection or in the options of one call) makes every other function use it:

```sql
.mode box --wordwrap on
SELECT
  sparql_count('PREFIX ex: <http://example.org/pl/>
                SELECT * WHERE { ?a ex:influencedBy ?b . ?b ex:influencedBy ?c }') AS native,
  sparql_count_sql('PREFIX ex: <http://example.org/pl/>
                SELECT * WHERE { ?a ex:influencedBy ?b . ?b ex:influencedBy ?c }') AS translated;
```

```output
┌────────┬────────────┐
│ native │ translated │
├────────┼────────────┤
│ 1024   │ 1024       │
└────────┴────────────┘
```

The translator does not support CONSTRUCT and DESCRIBE.

### 6.13 Timeouts, interrupts and memory limits

A query that runs too long can be stopped three ways:

- **`timeout_ms`**: a time limit per query, for the connection or for one call. This cross
  product of every triple with every other triple, twice, would run for hours:

  ```sql
  SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f }',
                                   '{"timeout_ms": 200}');
  ```

  ```output
  Runtime error: sparql: query timeout
  ```

  For `sparql_rows` and `sparql_view`, the time counts from the start of the scan, including the
  time your SQL spends between rows.
- **Interrupts.** Ctrl-C in the sqlite3 shell, and `sqlite3_interrupt()` (Python:
  `connection.interrupt()`) from another thread, stop a running query with the error
  `sparql: interrupted`. A `sparql_rows` or `sparql_view` scan whose rows arrive quickly may
  instead stop with SQLite's own `interrupted`, when SQLite notices the interrupt between two rows
  before sparqlite does. So a program should look for `interrupted` in the message. (Scans also
  set the error code `SQLITE_INTERRUPT`; the scalar functions set the generic `SQLITE_ERROR`.)
  Here a timer thread interrupts two queries that would run for hours:

  <!-- verify: python -->
  ```python
  import sqlite3, threading

  con = sqlite3.connect("tutorial/tutorial.db")
  con.enable_load_extension(True)
  con.load_extension("ext/target/release/libsparqlite")
  con.enable_load_extension(False)

  endless = "SELECT (COUNT(*) AS ?n) WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f }"
  for sql in ["SELECT sparql_count(?)", "SELECT v0 FROM sparql_rows(?)"]:
      threading.Timer(0.5, con.interrupt).start()   # interrupt after half a second
      try:
          con.execute(sql, (endless,)).fetchall()
      except sqlite3.OperationalError as e:
          print(sql, "->", e)
  ```

  ```output
  SELECT sparql_count(?) -> sparql: interrupted
  SELECT v0 FROM sparql_rows(?) -> sparql: interrupted
  ```

- **`max_memory_mb`** caps the memory a single query may use for intermediate results. When it is
  exceeded the query stops with an error instead of exhausting the machine:

  ```sql
  SELECT sparql_config('max_memory_mb', 1);
  SELECT sparql_count('SELECT DISTINCT ?a ?c WHERE { ?a ?p ?b . ?c ?q ?d }');
  SELECT sparql_config('max_memory_mb', 1024);
  ```

  ```output
  ┌───────────────────────────────────┐
  │ sparql_config('max_memory_mb', 1) │
  ├───────────────────────────────────┤
  │ 1                                 │
  └───────────────────────────────────┘
  Runtime error: sparql: memory budget exceeded (1 MB)
  ┌──────────────────────────────────────┐
  │ sparql_config('max_memory_mb', 1024) │
  ├──────────────────────────────────────┤
  │ 1024                                 │
  └──────────────────────────────────────┘
  ```

### 6.14 Errors

All errors from sparqlite are ordinary SQL errors whose message starts with `sparql:` and says
what went wrong. In the shell they appear as `Runtime error: ...`; in Python as
`sqlite3.OperationalError`. A mistake in the query text gives the position:

```sql
SELECT sparql_query('SELECT ?x WHERE { ?x ?p }');
```

```output
Runtime error: sparql: syntax error: error at 1:26: expected one of LATERAL, SERVICE, [_]
```

The position (line 1, column 26) is where the parser gave up, which is usually just after the
real mistake: here the end of the text, after the triple pattern `?x ?p` that has no object. The
list after "expected" is the parser's guess at what could come next. It can be long and odd
(`LATERAL` is a proposed extension to SPARQL, not part of SPARQL 1.1), and sometimes a
plain-English hint is buried in it, as in the `GROUP BY` example of lesson 5.8.

A mistake in RDF data gives the line and column:

```sql
SELECT sparql_load('<http://example.org/a> <http://example.org/b> "unterminated .');
```

```output
Runtime error: sparql: RDF parse error: Parser error at line 1 between columns 47 and 62: Unexpected end of file
```

The kinds of message: `syntax error` (query text), `RDF parse error` (data), `invalid argument`
(a wrong format name, setting or argument), `unsupported` (`SERVICE`, or nesting too deep; see
8.2 and 8.3), `query timeout`, `interrupted`, `memory budget exceeded`, `RDF store schema missing
or incompatible`, `cannot modify the RDF store while a sparql_rows cursor is open`, and
`SQLite error N: ...` for a problem reported by SQLite itself, such as `database is locked`.
Section 8.3 explains the common ones.

Finally, drop the view table from 6.7 (it would disappear anyway when the shell exits):

```sql
DROP TABLE temp.languages;
```

---

## 7. Performance

The tutorial database is too small for performance to matter. This section builds a bigger one
and shows how to measure queries, read query plans, and which settings matter. All numbers below
come from one run on a shared 4-core Linux virtual machine; they vary by several tens of percent
from run to run there, and your machine will give different ones. Compare numbers measured on
the same machine with each other, not with these.

### 7.1 A larger dataset

`--people N` adds a synthetic social network of N people (about 8.4 triples each): names, birth
years, a city, one or two liked languages, and `foaf:knows` links to one to five others. Nobody in
it is real. 100,000 people make about 840,000 triples and an 88 MB `people.nt` file. Build it into
a separate database:

<!-- verify: setup -->
<!-- verify: ignore-timing -->
```sh
python3 tutorial/make_tutorial_data.py --people 100000 --db tutorial/people.db --ext ext/target/release/libsparqlite
```

```output
wrote languages.ttl         2126 triples (94 languages, 117 people, 40 organisations)
wrote survey.trig            287 quads   (FICTIONAL; 3 named graphs + their metadata)
wrote job_postings.csv       300 rows    (FICTIONAL), and job_postings.sql
wrote people.nt           836188 triples (SYNTHETIC; 100000 people, 2.3s)
note: the existing load.sql does not match these files (another --people setting or an older version of this script); add --emit-sql to rewrite it
loaded sparqlite 0.1.0 (SQLite 3.45.1) from ext/target/release/libsparqlite.so
  languages.ttl          2126 new quads  (0.01s)
  survey.trig             287 new quads  (0.00s)
  people.nt            836188 new quads  (3.46s)
summary: 838601 quads, 104506 dictionary terms, 3 named graphs, 94 languages, 300 job postings
wrote tutorial/people.db
```

(The note about `load.sql` only means that the checked-in `load.sql` does not load `people.nt`;
add `--emit-sql` if you want a version that does.) The load of `people.nt` ran in *bulk mode*:
when one load adds at least 100,000 triples and grows the store by more than 30%, sparqlite drops
its indexes, inserts in sorted order and rebuilds them, which is much faster than updating eight
indexes row by row. Load large data in one call, or in a few large ones.

`people.nt` is only needed for the load. You can delete it afterwards
(`rm tutorial/data/people.nt`); in a git checkout it would otherwise show up as a new, 88 MB
file.

Open it in a new shell, with the timer on:

<!-- verify: session tutorial/people.db -->
```sh
sqlite3 tutorial/people.db
```

```sql
.load ext/target/release/libsparqlite
.mode box --wordwrap on
SELECT count(*) AS quads FROM rdf_quad;
SELECT page_count * page_size / 1000000 AS megabytes FROM pragma_page_count, pragma_page_size;
.timer on
```

```output
┌────────┐
│ quads  │
├────────┤
│ 838601 │
└────────┘
┌───────────┐
│ megabytes │
├───────────┤
│ 61        │
└───────────┘
```

The store takes about 73 bytes per triple, all indexes included.

### 7.2 Timing queries

With `.timer on` the shell prints `Run Time: real ... user ... sys ...` after each statement.
`real` is the elapsed time in seconds. Some typical queries:

<!-- verify: ignore-timing -->
```sql
SELECT v0 AS language, v1 AS fans FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
  SELECT ?name (COUNT(*) AS ?fans)
  WHERE { ?person ex:likes ?lang . ?lang rdfs:label ?name }
  GROUP BY ?name
  ORDER BY DESC(?fans)
  LIMIT 5
');
```

```output
┌────────────┬───────┐
│  language  │ fans  │
├────────────┼───────┤
│ JavaScript │ 22155 │
│ Python     │ 20413 │
│ TypeScript │ 15975 │
│ Java       │ 10709 │
│ C#         │ 9969  │
└────────────┴───────┘
Run Time: real 0.012 user 0.008067 sys 0.003837
```

<!-- verify: ignore-timing -->
```sql
SELECT v0 AS friend, v1 AS born FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?name ?year
  WHERE {
    <http://example.org/people/p42> foaf:knows ?friend .
    ?friend foaf:name ?name ; ex:birthYear ?year .
  }
  ORDER BY ?name
');
```

```output
┌────────────────┬──────┐
│     friend     │ born │
├────────────────┼──────┤
│ Ben Williams   │ 1979 │
│ Dmitri Nielsen │ 1995 │
│ Hana Le        │ 1988 │
│ James Ivanova  │ 1969 │
│ Rosa O'Brien   │ 1993 │
└────────────────┴──────┘
Run Time: real 0.001 user 0.000203 sys 0.000303
```

<!-- verify: ignore-timing -->
```sql
SELECT sparql_count('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT DISTINCT ?person WHERE { ?person foaf:knows/ex:likes ex:Rust }
') AS people_with_a_friend_who_likes_rust;
```

```output
┌─────────────────────────────────────┐
│ people_with_a_friend_who_likes_rust │
├─────────────────────────────────────┤
│ 14398                               │
└─────────────────────────────────────┘
Run Time: real 0.008 user 0.004090 sys 0.004025
```

Turning ids into values costs time, and sparqlite only does it for the columns you read.
Counting 100,000 solutions is cheap; reading all their names takes a few times longer; and
counting distinct names inside SPARQL works on ids and is cheapest of all:

<!-- verify: ignore-timing -->
```sql
SELECT count(*) AS solutions FROM sparql_rows('
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?person ?name WHERE { ?person a foaf:Person ; foaf:name ?name }');
SELECT count(DISTINCT v1) AS distinct_names FROM sparql_rows('
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?person ?name WHERE { ?person a foaf:Person ; foaf:name ?name }');
SELECT sparql_count('
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT DISTINCT ?name WHERE { ?person a foaf:Person ; foaf:name ?name }') AS distinct_names;
```

```output
┌───────────┐
│ solutions │
├───────────┤
│ 100000    │
└───────────┘
Run Time: real 0.013 user 0.011847 sys 0.000336
┌────────────────┐
│ distinct_names │
├────────────────┤
│ 3600           │
└────────────────┘
Run Time: real 0.052 user 0.052079 sys 0.000000
┌────────────────┐
│ distinct_names │
├────────────────┤
│ 3600           │
└────────────────┘
Run Time: real 0.008 user 0.008677 sys 0.000000
```

So: aggregate and filter in SPARQL where you can, select only the variables you need, and read
`id*` columns instead of values when you only need to compare or join.

### 7.3 Reading a query plan

`sparql_explain(query)` shows the plan the engine chose, without running the query. Take
"pairs of people where one knows someone who knows the other, and both live in the same city":

```sql
.timer off
.mode list
SELECT sparql_explain('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c
  WHERE {
    ?a foaf:knows ?b .
    ?b foaf:knows ?c .
    ?a ex:livesIn ?city .
    ?c ex:livesIn ?city .
  }
');
```

```output
#0 Project [?a ?c]                                  est=2.0e5 cost=113.7ms
#1  MergeJoin key=?c gallop=10 check=[?city]        est=2.0e5 cost=113.7ms
#2   SortBySlot ?c                                  est=9.0e5 cost=104.6ms
#3    MergeJoin key=?b gallop=10                    est=9.0e5 cost=47.7ms
#4     SortBySlot ?b                                est=3.0e5 cost=32.7ms
#5      MergeJoin key=?a gallop=10                  est=3.0e5 cost=15.2ms
#6       Scan Gpso ?a <http://xmlns.com/foaf/0.1/knows> ?b (sorted ?a) est=3.0e5 cost=10.2ms
#7       Scan Gpso ?a <http://example.org/pl/livesIn> ?city (sorted ?a) est=1.0e5 cost=3.4ms
#8     Scan Gpso ?b <http://xmlns.com/foaf/0.1/knows> ?c (sorted ?b) est=3.0e5 cost=10.2ms
#9   Scan Gpso ?c <http://example.org/pl/livesIn> ?city (sorted ?c) est=1.0e5 cost=3.4ms
```

How to read it:

- Each line is an operator; the operators indented below it are its inputs. Read from the
  innermost lines outwards: data flows upwards.
- `Scan Gpso ?a <...knows> ?b` reads one triple pattern from one of the store's indexes. The
  index name gives the column order: `Gpso` is graph, predicate, subject, object, so with the
  predicate fixed it returns the triples sorted by subject (`sorted ?a`).
- Joins: `MergeJoin key=?x` merges two inputs that are both sorted by `?x`; `check=[?y]` compares
  another shared variable once `?x` matches, and `gallop=10` lets it skip ahead in the index after
  10 rows without a match. `HashJoin keys=[...]` builds a hash table from one input;
  `BindJoin seed=[?x]` looks up the right side once for each row of the left side, with `?x`
  filled in (like an index nested-loop join). `SortBySlot` re-sorts rows for a following merge
  join.
- Other operators: `Project` (the `SELECT` list), `Distinct`, `Group` (`GROUP BY`), `TopK`
  (`ORDER BY` with `LIMIT`), `Extend` (`BIND` and expressions), `Filter`, `Union`, `LeftJoin`
  (`OPTIONAL`), `SqlJoin` (a chain of index lookups handed to SQLite as one SQL statement), and
  more.
- `est` is the planner's estimate of the number of rows (`9.0e5` is 900,000) and `cost` its
  estimate of the time.

So the engine reads everyone's friends and everyone's city (`#6` and `#7`, both sorted by `?a`)
and merges them (`#5`), re-sorts by `?b` to add the friends' friends (`#3`), re-sorts by `?c`, and
finally merges with the city of each `?c` (`#1`), keeping a row only when that city is `?a`'s.

The last join could also have merged on `?city` and checked `?c`. That would be a poor choice:
the 100,000 people live in only 20 cities (Tokyo alone has 22,664 of them), so a merge on `?city`
would pair each of the 900,000 rows from below with every inhabitant of its city before checking
`?c`, billions of comparisons. On `?c` each row meets just one city. The planner's cost model
counts those comparisons, and that is how it picks the key. Run the query:

<!-- verify: ignore-timing -->
```sql
.timer on
.mode box --wordwrap on
SELECT count(*) AS pairs FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c .
                       ?a ex:livesIn ?city . ?c ex:livesIn ?city . }
');
```

```output
┌────────┐
│ pairs  │
├────────┤
│ 292595 │
└────────┘
Run Time: real 0.184 user 0.151594 sys 0.031396
```

**Steering the planner.** The setting `join_policy` makes the planner prefer one kind of join:
`hash`, `bind` or `merge` (and `nosql` keeps it from handing chains of lookups to SQLite). Set it
for one call in the options of `sparql_rows`, or for the connection with `sparql_config`. For
this query the planner's own choice is already the fastest, so forcing the others shows what they
would cost:

<!-- verify: ignore-timing -->
```sql
SELECT count(*) AS pairs FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c .
                       ?a ex:livesIn ?city . ?c ex:livesIn ?city . }
', '{"join_policy": "hash"}');
SELECT count(*) AS pairs FROM sparql_rows('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c .
                       ?a ex:livesIn ?city . ?c ex:livesIn ?city . }
', '{"join_policy": "bind"}');
```

```output
┌────────┐
│ pairs  │
├────────┤
│ 292595 │
└────────┘
Run Time: real 0.276 user 0.262410 sys 0.011781
┌────────┐
│ pairs  │
├────────┤
│ 292595 │
└────────┘
Run Time: real 0.873 user 0.866750 sys 0.000000
```

The same answer, at different speeds: in the run shown, 0.28 seconds with hash joins and 0.87
with bind joins, against 0.18 for the planner's plan. (`"engine": "sql"`, the SQL translator of
6.12, needs about 1.5 seconds.) When a query of yours is slow, these settings are a quick way to
find out whether another plan would do better.

`sparql_explain(query, 'analyze')` runs the query and adds what actually happened to each line:
`rows` produced, `opens` (how often the operator was started), `seeks` (index lookups) and `ms`
(time spent in it, including its inputs). The last line gives the totals. `explain` uses the
connection's settings, so set the policy with `sparql_config` this time. First the bind joins:

<!-- verify: ignore-timing -->
```sql
.timer off
.mode list
SELECT sparql_config('join_policy', 'bind');
SELECT sparql_explain('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c .
                       ?a ex:livesIn ?city . ?c ex:livesIn ?city . }
', 'analyze');
```

```output
bind
#0 Project [?a ?c]                                  est=2.0e5 cost=1.10s rows=292595 opens=1 seeks=0 ms=957.373
#1  BindJoin seed=[?c ?city] sort_left adaptive     est=2.0e5 cost=1.10s rows=292595 opens=1 seeks=0 ms=956.635
#2   BindJoin seed=[?b] sort_left adaptive          est=9.0e5 cost=472.8ms rows=903660 opens=1 seeks=0 ms=411.703
#3    BindJoin seed=[?a] adaptive                   est=3.0e5 cost=45.0ms rows=299630 opens=1 seeks=0 ms=48.182
#4     Scan Gpso ?a <http://xmlns.com/foaf/0.1/knows> ?b (sorted ?a) est=3.0e5 cost=10.2ms rows=299630 opens=1 seeks=1 ms=9.756
#5     Scan Gpso ?a <http://example.org/pl/livesIn> ?city (sorted ?city) est=1 cost=1.3us rows=100000 opens=100000 seeks=100000 ms=13.667
#6    Scan Gpso ?b <http://xmlns.com/foaf/0.1/knows> ?c (sorted ?c) est=3 cost=1.4us rows=872768 opens=291247 seeks=291247 ms=61.498
#7   Scan Gpso ?c <http://example.org/pl/livesIn> ?city est=0.1 cost=1.3us rows=290081 opens=889110 seeks=889110 ms=99.306
-- 292595 rows; planning 0.082 ms, execution 958.538 ms; decode hits 0, misses 0
```

A `BindJoin` starts its right input for each row from the left, except that rows arriving
together with the same seed values share one start (`opens`), and each start is an index lookup
(`seeks`): the city of every `?a` (`#5`, 100,000 lookups), the friends of every `?b` (`#6`,
291,247) and every (`?c`, `?city`) pair (`#7`, 889,110). (`sort_left` sorts each batch of left
rows by the seed first, so that equal seeds come together; `adaptive` lets the join switch to
hashing if the left side turns out far larger than estimated.) On such an input `est` is per
lookup and `rows` is the total. Each lookup is quick, but 1.3 million of them add up. The
planner saw this coming: its estimate for this plan is `cost=1.10s` (line `#0`), against
`cost=113.7ms` for its own choice. Now the planner's own plan:

<!-- verify: ignore-timing -->
```sql
SELECT sparql_config('join_policy', 'auto');
SELECT sparql_explain('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c .
                       ?a ex:livesIn ?city . ?c ex:livesIn ?city . }
', 'analyze');
```

```output
auto
#0 Project [?a ?c]                                  est=2.0e5 cost=113.7ms rows=292595 opens=1 seeks=0 ms=187.263
#1  MergeJoin key=?c gallop=10 check=[?city]        est=2.0e5 cost=113.7ms rows=292595 opens=1 seeks=0 ms=186.612
#2   SortBySlot ?c                                  est=9.0e5 cost=104.6ms rows=903660 opens=1 seeks=0 ms=170.059
#3    MergeJoin key=?b gallop=10                    est=9.0e5 cost=47.7ms rows=903660 opens=1 seeks=0 ms=77.574
#4     SortBySlot ?b                                est=3.0e5 cost=32.7ms rows=299630 opens=1 seeks=0 ms=54.783
#5      MergeJoin key=?a gallop=10                  est=3.0e5 cost=15.2ms rows=299630 opens=1 seeks=0 ms=23.380
#6       Scan Gpso ?a <http://xmlns.com/foaf/0.1/knows> ?b (sorted ?a) est=3.0e5 cost=10.2ms rows=299630 opens=1 seeks=1 ms=11.932
#7       Scan Gpso ?a <http://example.org/pl/livesIn> ?city (sorted ?a) est=1.0e5 cost=3.4ms rows=100000 opens=1 seeks=1 ms=3.100
#8     Scan Gpso ?b <http://xmlns.com/foaf/0.1/knows> ?c (sorted ?b) est=3.0e5 cost=10.2ms rows=298976 opens=1 seeks=24 ms=9.907
#9   Scan Gpso ?c <http://example.org/pl/livesIn> ?city (sorted ?c) est=1.0e5 cost=3.4ms rows=99997 opens=1 seeks=2 ms=3.119
-- 292595 rows; planning 0.133 ms, execution 192.511 ms; decode hits 0, misses 0
```

Every operator runs once (`opens=1`), and the scans read their indexes in order, seeking only to
start and to skip ahead (the 24 seeks of `#8` are gallops). Most of the time goes into the two
`SortBySlot` re-sorts. `ms` includes the inputs, so subtract: in the run shown, `#2` took 170 ms
and its input `#3` 78 ms, so sorting 903,660 rows by `?c` took about 90 ms, and the sort by `?b`
(`#4` minus `#5`) another 30, together two thirds of the 193 ms.

Comparing `est` with `rows` shows where the planner's estimates are off. Here they are close: the
result has 292,595 rows against an estimate of 200,000. An estimate that is off by a factor of ten
or more on a large input is the usual cause of a poor plan; running `sparql_analyze()` (7.5) after
large changes to the data is the first thing to try.

**Join order.** The planner also chooses the order of the joins. With `reorder` set to 0 it joins
the patterns in the order they are written instead, which shows why the order matters. Write the
two `livesIn` patterns first:

```sql
SELECT sparql_config('reorder', 0);
SELECT sparql_explain('
  PREFIX ex:   <http://example.org/pl/>
  PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT ?a ?c WHERE { ?a ex:livesIn ?city . ?c ex:livesIn ?city .
                       ?a foaf:knows ?b . ?b foaf:knows ?c . }
');
SELECT sparql_config('reorder', 1);
```

```output
0
#0 Project [?a ?c]                                  est=4.5e4 cost=187.91s
#1  HashJoin keys=[?c ?b] build=right               est=4.5e4 cost=187.91s
#2   Scan Gpso ?b <http://xmlns.com/foaf/0.1/knows> ?c (sorted ?b) est=3.0e5 cost=10.2ms
#3   MergeJoin key=?a gallop=10                     est=1.5e9 cost=8.05s
#4    HashJoin keys=[?city] build=right             est=5.0e8 cost=2.04s
#5     Scan Gpso ?c <http://example.org/pl/livesIn> ?city (sorted ?c) est=1.0e5 cost=3.4ms
#6     Scan Gpso ?a <http://example.org/pl/livesIn> ?city (sorted ?a) est=1.0e5 cost=3.4ms
#7    Scan Gpso ?a <http://xmlns.com/foaf/0.1/knows> ?b (sorted ?a) est=3.0e5 cost=10.2ms

1
```

The first join (`#4`) pairs everyone with everyone else in the same city, an estimated 500 million
rows, and the planner puts the cost of the whole plan at about three minutes. (Run once for this
tutorial, it took two and a half, against a fifth of a second for the order the planner chooses;
the checker does not run it.)

The other modes of `sparql_explain` are `algebra` (the query as parsed, in SPARQL algebra),
`logical` (after sparqlite's rewrites), `sql` and `sqlplan` (the translator's SQL and SQLite's
plan for it).

### 7.4 The B-tree reader

sparqlite's engine reads the index rows of `rdf_quad` in one of two ways. The plain way prepares
SQL statements and steps through them. The faster way decodes SQLite's B-tree pages from the
database file directly, which skips SQLite's statement machinery for large scans. The setting
`reader` chooses: `auto` (the default) uses the direct reader whenever it can, `sql` never uses
it, and `btree` currently behaves like `auto`.

The direct reader is used only when it is safe:

- the database is a file in one of the rollback-journal modes (the default `DELETE`, or
  `TRUNCATE`, `PERSIST`, ...), not in WAL mode; in-memory and temporary databases always use SQL;
- the connection does not hold a write transaction of its own (inside your `BEGIN ... COMMIT`,
  after a write, scans go through SQL);
- the connection does not use SQLite's shared-cache mode.

Otherwise sparqlite falls back to SQL statements, with identical results. The `btree_*` rows of
`sparql_status` show what happened. A scan-heavy query (how many friend-of-a-friend paths are
there?) with each reader:

<!-- verify: ignore-timing -->
```sql
.mode box --wordwrap on
.timer on
SELECT sparql_value('PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT (COUNT(*) AS ?n) WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c }');
SELECT sparql_config('reader', 'sql');
SELECT sparql_value('PREFIX foaf: <http://xmlns.com/foaf/0.1/>
  SELECT (COUNT(*) AS ?n) WHERE { ?a foaf:knows ?b . ?b foaf:knows ?c }');
SELECT sparql_config('reader', 'auto');
.timer off
```

```output
┌─────────────────────────────────────────────────────────┐
│ sparql_value('PREFIX foaf: <http://xmlns.com/foaf/0.1/> │
├─────────────────────────────────────────────────────────┤
│ 903660                                                  │
└─────────────────────────────────────────────────────────┘
Run Time: real 0.023 user 0.022895 sys 0.000000
┌────────────────────────────────┐
│ sparql_config('reader', 'sql') │
├────────────────────────────────┤
│ sql                            │
└────────────────────────────────┘
Run Time: real 0.000 user 0.000038 sys 0.000000
┌─────────────────────────────────────────────────────────┐
│ sparql_value('PREFIX foaf: <http://xmlns.com/foaf/0.1/> │
├─────────────────────────────────────────────────────────┤
│ 903660                                                  │
└─────────────────────────────────────────────────────────┘
Run Time: real 0.062 user 0.053638 sys 0.008063
┌─────────────────────────────────┐
│ sparql_config('reader', 'auto') │
├─────────────────────────────────┤
│ auto                            │
└─────────────────────────────────┘
Run Time: real 0.000 user 0.000039 sys 0.000001
```

In the run shown, the direct reader took 23 ms and the SQL statements 62 ms; over several runs
for this tutorial it was two to three times faster on such scans. For queries that mostly look up
a few rows the difference is small. Check that it is in use:

```sql
SELECT key, value FROM sparql_status
WHERE key IN ('reader', 'btree_fallbacks', 'btree_last_fallback');
SELECT (SELECT value FROM sparql_status WHERE key = 'btree_pages_read') > 0 AS btree_reader_used;
```

```output
┌─────────────────────┬───────┐
│         key         │ value │
├─────────────────────┼───────┤
│ reader              │ auto  │
│ btree_fallbacks     │ 0     │
│ btree_last_fallback │ none  │
└─────────────────────┴───────┘
┌───────────────────┐
│ btree_reader_used │
├───────────────────┤
│ 1                 │
└───────────────────┘
```

In WAL mode it is not used, and `btree_last_fallback` says why:

```sql
PRAGMA journal_mode = WAL;
SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }') AS triples;
SELECT value AS btree_last_fallback FROM sparql_status WHERE key = 'btree_last_fallback';
PRAGMA journal_mode = DELETE;
```

```output
┌──────────────┐
│ journal_mode │
├──────────────┤
│ wal          │
└──────────────┘
┌─────────┐
│ triples │
├─────────┤
│ 838334  │
└─────────┘
┌───────────────────────────────────────────┐
│            btree_last_fallback            │
├───────────────────────────────────────────┤
│ precondition: the database is in WAL mode │
└───────────────────────────────────────────┘
┌──────────────┐
│ journal_mode │
├──────────────┤
│ delete       │
└──────────────┘
```

WAL has its own advantages (readers and a writer do not block each other), so choose by
workload: WAL for many concurrent writers and readers, the default journal mode for read-mostly
analysis.

### 7.5 Statistics: sparql_analyze

The planner chooses join orders from statistics that `sparql_analyze()` collects: the number of
triples per predicate with their distinct subjects and objects, *characteristic sets* (which
combinations of predicates subjects have, e.g. "has a name, a birth year and a city"), and
*heavy hitters* (very common predicate-object pairs, such as `rdf:type foaf:Person`). Loads run it
automatically after the store has grown by more than 25%; after many small updates, run it
yourself. It took a fraction of a second here:

<!-- verify: ignore-timing -->
```sql
.mode list
SELECT sparql_analyze();
```

```output
{"analyzed_quads":838601,"default_graph":{"characteristic_sets":89,"heavy_hitters":112,"objects":95982,"predicates":37,"subjects":100319,"triples":838334},"ms":254,"named_graphs":{"characteristic_sets":5,"heavy_hitters":0,"objects":206,"predicates":10,"subjects":46,"triples":267}}
```

### 7.6 SQLite settings

- **Page size.** 8 KiB pages suit the store (`make_tutorial_data.py` uses them). `PRAGMA
  page_size = 8192;` only takes effect before the first table is created, so run it before
  `sparql_init()` or the first load in a new database file (or run `VACUUM` afterwards to
  rebuild the file with the new size).
- **Page caches.** `PRAGMA cache_size` sets SQLite's own page cache (by default about 2 MB); it
  matters for the `sql` reader, the SQL translator and writes. The direct B-tree reader has its
  own cache, `sparql_config('page_cache_mb', ...)` (128 MB by default). In the runs for this
  tutorial, raising `cache_size` to 256 MB made no measurable difference, because the 61 MB
  database was already in the operating system's file cache.
- **Journal mode.** See 7.4.
- **Transactions.** Many small `sparql_update` or `sparql_load` calls are much faster inside one
  `BEGIN ... COMMIT` than each in its own transaction, as with any SQLite writes.
- **Statistics.** sparqlite's planner uses `sparql_analyze`; SQLite's `ANALYZE` helps the SQL
  translator (`engine = sql`).

---

## 8. Reference and troubleshooting

### 8.1 SQL functions and virtual tables

Arguments in brackets are optional. "Writer" functions change the database; they, and the
two-argument `sparql_config`, can only be called from top-level SQL statements (and from TEMP
views and triggers), never from views, triggers or `CHECK` constraints stored in the database,
and never from inside another sparqlite call (see 8.3). All other functions only read, and work
anywhere, stored views and triggers included (6.9).

| Function | Returns |
|---|---|
| `sparql_version()` | TEXT: `sparqlite <version> (SQLite <version>)` |
| `sparql_init()` | INTEGER: the storage schema version; creates the `rdf_*` tables if needed. Writer. |
| `sparql_load(data [, format [, base [, graph]]])` | INTEGER: new triples/quads. `data` is TEXT or BLOB; `format` is `turtle` (default) / `ttl`, `ntriples` / `nt`, `nquads` / `nq`, `trig`, `rdfxml` / `xml`, `n3` or `jsonld`; `graph` is a target graph IRI. Writer. |
| `sparql_load_file(path [, format [, base [, graph]]])` | INTEGER: new triples/quads; format from the file extension, base = the file's `file://` URL. Needs `sparql_config('allow_files', 1)`. Writer. |
| `sparql_update(update)` | INTEGER: triples actually added + triples actually removed. `LOAD <file:...>` needs `allow_files`. Writer. |
| `sparql_query(query [, format])` | TEXT: SELECT/ASK as `json` (default), `xml`, `csv` or `tsv`; CONSTRUCT/DESCRIBE as `ntriples` (default), `nquads` or `turtle` |
| `sparql_ask(query)` | INTEGER 1/0 |
| `sparql_count(query)` | INTEGER: number of solutions (or triples), without converting any value |
| `sparql_value(query)` | the first variable of the first solution as a SQL value, or NULL; 1/0 for ASK |
| `sparql_explain(query [, mode])` | TEXT: `plan` (default), `analyze` (runs the query), `algebra`, `logical`, `sql`, `sqlplan` |
| `sparql_analyze()` | TEXT: JSON summary of the new statistics. Writer. |
| `sparql_check()` | TEXT: `ok: ...` or the problems found |
| `sparql_vacuum_terms()` | INTEGER: unused dictionary entries deleted. Writer. |
| `sparql_config(key)` | TEXT: the current value of a setting (table in [6.10](#610-settings-sparql_config-and-sparql_status)) |
| `sparql_config(key, value)` | TEXT: the new value. Top-level SQL only. |
| `sparql_query_sql(query [, format])`, `sparql_count_sql(query)`, `sparql_to_sql(query)` | as `sparql_query` / `sparql_count` but with the SQL translator; the translated SQL |
| `sparql_term(id)` | TEXT: the term in N-Triples syntax, or NULL |
| `sparql_term_value(id)` | the term as a SQL value |
| `sparql_term_id(ntriples)` | INTEGER id of an existing term, or NULL. The term is in N-Triples syntax; bare numbers and `true`/`false` are accepted, prefixed names are not. |
| `sparql_term_kind(id)` | TEXT: `iri`, `literal` or `bnode` |
| `sparql_term_lex(id)`, `sparql_term_lang(id)`, `sparql_term_datatype(id)` | TEXT: lexical form / IRI / label; language tag; datatype IRI |

The functions `sparql_num`, `sparql_str`, `sparql_ebv`, `sparql_sortkey`, `sparql_regex`,
`sparql_eval` and the aggregates `sparql_sum`, `sparql_avg`, `sparql_min`, `sparql_max`,
`sparql_sample` and `sparql_group_concat` also exist. They are helpers that the SQL translator's
generated SQL calls; they are not meant to be called directly.

| Virtual table | Columns |
|---|---|
| `sparql_rows(query [, options])` | `n`, `v0`-`v15` (values), `t0`-`t15` (N-Triples), `id0`-`id15` (term ids), `vars` (JSON array), `json` (the solution as SPARQL JSON); hidden `query`, `opts`. `options` is a JSON object of per-call settings. |
| `CREATE VIRTUAL TABLE name USING sparql_view(query [, key=value ...])` | one column per variable of the SELECT query; hidden `<var>__term`, `<var>__id` and `opts`. `=` on `__term` / `__id` is pushed into the query. |
| `sparql_status` | `key`, `value`: settings (`config.*`), versions, schema state, caches, counters |

Storage tables (read them, do not write them with plain SQL): `rdf_quad(g, s, p, o)` (one row
per quad; `g = 0` is the default graph), `rdf_term(id, tag, lex, dt, lang, h)` (the dictionary),
`rdf_graph(g)` (the named graphs), `rdf_meta` (schema version and counters) and the statistics
tables `rdf_stat_*`.

### 8.2 SPARQL support

sparqlite implements SPARQL 1.1 Query and Update: all four query forms, property paths,
aggregates, sub-queries, `VALUES`, `BIND`, `MINUS`, `EXISTS`, named graphs with `FROM` /
`FROM NAMED`, the SPARQL 1.1 functions, and the update operations `INSERT` / `DELETE` (`DATA`,
`WHERE` and the combined form), `LOAD`, `CLEAR`, `DROP`, `CREATE`, `ADD`, `MOVE` and `COPY`. Of the
639 W3C SPARQL 1.1 test cases that its test runner executes, 636 pass; the other three are
handled differently by the SPARQL parser it uses. Not supported: `SERVICE` (federated queries),
and `LOAD` from the web (`LOAD` reads only `file:` IRIs, and only with `allow_files`). Very deeply
nested queries (dozens of nested sub-queries, for example) are refused with a "nests too deeply"
error rather than risking the host program's stack.

### 8.3 Troubleshooting

**`.load` fails.**

- `Error: ext/target/release/libsparqlite.so: cannot open shared object file: No such file or
  directory`: the path is wrong (run the shell from the project folder, or give the full path) or
  the extension is not built yet (section 1.2).
- `undefined symbol: sqlite3_mycopy_init` (or similar): the file was renamed, so the shell
  derived the wrong entry point name; add it: `.load ./mycopy.so sqlite3_sparqlite_init`.
- On Windows, an error saying that the module could not be found usually means the path still
  has the `lib` prefix: the file is `sparqlite.dll` (see 1.3).
- `SQLite version mismatch: 3031001 < 3034001`: the SQLite library is older than 3.34.1.
- On macOS, Apple's `/usr/bin/sqlite3` has no `.load` command at all; use Homebrew's sqlite3
  (section 1.1).
- An error that mentions the file format or architecture (`wrong ELF class`, `incompatible
  architecture`): the extension was built for another kind of machine or for a 32-bit/64-bit
  host that does not match; build it on, and for, the machine that loads it.
- Python: `AttributeError: ... has no attribute 'enable_load_extension'` means that Python's
  `sqlite3` module was built without extension support; use another Python build, or the
  sqlite3 shell.

**`no such function: sparql_query`** (or any other `sparql_` name): the extension is not
loaded on this connection. Each connection, including each new shell session and each
connection in a pool, must load it:

<!-- verify: session -->
```sh
sqlite3
```

```sql
SELECT sparql_version();
```

```output
Parse error: no such function: sparql_version
  SELECT sparql_version();
         ^--- error here
```

```sql
.load ext/target/release/libsparqlite
.mode list
SELECT sparql_version();
```

```output
sparqlite 0.1.0 (SQLite 3.45.1)
```

**`RDF store schema missing or incompatible: the RDF store is not initialized`**: the database
has no `rdf_*` tables yet. Loads and updates create them automatically and queries on such a
database simply find nothing, but `sparql_analyze`, `sparql_check` and `sparql_vacuum_terms`
need an existing store. Run `SELECT sparql_init();`. The same message with another reason means
the tables were created by an incompatible version of sparqlite.

```sql
SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }') AS empty;
SELECT sparql_check();
SELECT sparql_init();
SELECT sparql_check();
```

```output
0
Runtime error: sparql: RDF store schema missing or incompatible: the RDF store is not initialized (run sparql_init())
1
ok: 35 terms, 0 quads, 0 named graphs
```

**My query returns nothing.** The usual suspects, in order:

1. The data is in a named graph and the pattern is not inside `GRAPH` (lesson 5.13), or the
   reverse.
2. A string literal without its language tag: `"Go"` does not match `"Go"@en` (lesson 5.3).
   Compare `STR(?label) = "Go"`.
3. A typo in an IRI or prefix. IRIs are compared exactly; `<http://example.org/pl/python>` is not
   `<http://example.org/pl/Python>`, and `schema:` is `https://`, not `http://`. Start with a
   pattern that matches and add one triple pattern at a time.
4. A number compared with a string: `FILTER (?year = "1991")` compares an integer with a string
   and is never true; write `?year = 1991`.
5. A `FILTER` that tests a variable from an `OPTIONAL` part: where the variable is unbound, the
   test is an error and the solution is dropped (lesson 5.4). Use `!BOUND(?x) || ...`, or put
   the filter inside the `OPTIONAL`.
6. `FROM NAMED` without `FROM` (the default graph is then empty), or `FROM` without `FROM NAMED`
   (then `GRAPH` finds nothing); see lesson 5.13.

**`cannot modify the RDF store while a sparql_rows cursor is open`**: a writer function ran while
a `sparql_rows` or `sparql_view` scan on the same connection was still open, either in the same
statement or in another statement that has not been read to the end. Read all the rows first (or
copy them into a temporary table), or close the cursor, then write. In the shell, this happens
when one statement does both:

```sql
SELECT sparql_load('<http://example.org/a> <http://example.org/p> "1" .', 'nt') AS loaded;
SELECT sparql_update('INSERT DATA { <http://example.org/b> <http://example.org/p> "2" }')
FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }');
```

```output
1
Runtime error: sparql: cannot modify the RDF store while a sparql_rows cursor is open
```

In a program it typically happens in a loop over the rows of a query that writes for each row.
The cursor is still open while the loop runs, so the first write fails; `fetchall()` reads all
rows first and closes the scan:

<!-- verify: python -->
```python
import sqlite3

con = sqlite3.connect(":memory:")
con.enable_load_extension(True)
con.load_extension("ext/target/release/libsparqlite")
con.enable_load_extension(False)
con.execute("""SELECT sparql_load('
  <http://example.org/a> <http://example.org/p> "1" .
  <http://example.org/b> <http://example.org/p> "2" .', 'nt')""")

query = "SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o } ORDER BY ?s')"
mark = "SELECT sparql_update('INSERT DATA { <%s> <http://example.org/seen> true }')"

cur = con.execute(query)
try:
    for (s,) in cur:              # the scan is still open inside the loop
        con.execute(mark % s)
except sqlite3.OperationalError as e:
    print("error:", e)
cur.close()

for (s,) in con.execute(query).fetchall():   # all rows read first
    con.execute(mark % s)
print(con.execute("SELECT sparql_count('SELECT * WHERE { ?s <http://example.org/seen> ?o }')")
      .fetchone()[0], "marked")
```

```output
error: sparql: cannot modify the RDF store while a sparql_rows cursor is open
2 marked
```

**`this writer cannot run inside a statement that writes`**: a writer function was called from an
`INSERT ... SELECT`, `UPDATE` or similar. Only `sparql_update` may do that, and only outside an
explicit transaction; call the others as their own `SELECT` statements:

```sql
CREATE TEMP TABLE results(n INTEGER);
INSERT INTO results SELECT sparql_load('<http://example.org/c> <http://example.org/p> "3" .', 'nt');
INSERT INTO results SELECT sparql_update('INSERT DATA { <http://example.org/d> <http://example.org/p> "4" }');
SELECT n FROM results;
```

```output
Runtime error: sparql: invalid argument: this writer cannot run inside a statement that writes (INSERT … SELECT, UPDATE …) here: only sparql_update can, and only outside an explicit transaction; call it as its own SELECT statement
1
```

**`unsafe use of sparql_update()`** (or another writer): the function was used in a view,
trigger or other schema object stored in the database. Writers are never allowed there; TEMP
views and triggers are not affected. Query functions such as `sparql_rows` and `sparql_value()`
work in stored views and triggers too; see [6.9](#69-saving-queries-as-views).

```sql
CREATE TABLE log(x);
CREATE TRIGGER log_update AFTER INSERT ON log BEGIN
  SELECT sparql_update('INSERT DATA { <http://example.org/e> <http://example.org/p> "5" }');
END;
INSERT INTO log VALUES (1);
```

```output
Parse error: unsafe use of sparql_update()
```

**`sparql_update refused: the definition of table ... names a sparqlite writer`**: SQLite does not
stop a `CHECK` constraint stored in the database from calling a writer, so sparqlite does. While
any table stored in the database names a writer in its definition, writers run only from a plain
top-level `SELECT` that calls them by name (such as `SELECT sparql_update(...)`) while no other
statement of the connection is running; anything else, including the `CHECK` itself, is refused:

```sql
CREATE TABLE notes(msg TEXT CHECK (sparql_update('INSERT DATA { <http://example.org/f> <http://example.org/p> "6" }') >= 0));
INSERT INTO notes VALUES ('hello');
DROP TABLE notes;
```

```output
Runtime error: sparql: invalid argument: sparql_update refused: the definition of table main.notes names a sparqlite writer, and SQLite lets a CHECK constraint stored in a database call SQLITE_DIRECTONLY functions; while such a table is in main or an attached database, writers run only from a top-level SELECT that calls them by name, with no other statement of this connection running
```

**`sparqlite calls nest too deeply`**, **`sparql_update cannot run inside another sparqlite
call`**: sparqlite reads and writes its `rdf_*` tables with ordinary SQL, so a trigger on one of
them (or a view put in place of one) runs inside sparqlite's own work, and if it calls sparqlite
again, the calls nest. A query function in a trigger on `rdf_quad` works, but writers never run
nested, and more than 8 nested calls are refused before they can overflow the program's stack.
Remove such triggers, and check that the database's `rdf_*` objects are sparqlite's own tables.
(The "nests too deeply" error of 8.2, for query text nested too deeply, is a different limit.)

**`no query solution`**: `sparql_rows` was used without a query (`SELECT * FROM sparql_rows`), or in
a join where SQLite cannot work out the query text before scanning it.

**`sparql: SQLite error 5: database is locked`**: another connection holds a write lock. Set
`PRAGMA busy_timeout = 5000;` (milliseconds) so that the connection waits, and keep write
transactions short.

**`query timeout`**, **`interrupted`**, **`memory budget exceeded`**: see
[6.13](#613-timeouts-interrupts-and-memory-limits).

**Queries are slower than expected on a WAL database**: the direct B-tree reader is not used in
WAL mode (section 7.4); `SELECT value FROM sparql_status WHERE key = 'btree_last_fallback'` says
why it was last skipped. Also check the plan (section 7.3) and whether `sparql_analyze()` has run
since the data changed a lot.

---

That is the end of the tutorial. To go further: [docs/SPEC.md](docs/SPEC.md) describes the design
in depth, and the README of each crate in `crates/` explains one part of the implementation.
