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

**How to read the examples.** Run every command from the project folder, `sqlite-sparql-rust/`.
A block marked `sql` is typed into the `sqlite3` shell; the block after it shows what the shell
printed. All outputs were copied from real runs (SQLite 3.45.1 on Linux) and are re-checked by
[`tutorial/verify_tutorial.py`](tutorial/verify_tutorial.py), which replays the whole tutorial in a
scratch folder. Timings depend on your machine, so yours will differ.

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

### 1.2 Build the extension

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

If `.load` fails, see [Troubleshooting](#83-troubleshooting).

**From Python**, enable extension loading on the connection, load, and switch it off again:

<!-- verify: python -->
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
data. `sparql_query()` runs a query and returns the answer as one piece of text, by default in
the standard SPARQL JSON results format:

```sql
SELECT sparql_query('
  PREFIX ex: <http://example.org/>
  SELECT ?name ?age
  WHERE { ?person ex:name ?name ;
                  ex:age  ?age . }
  ORDER BY ?name
');
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
| `id0` … `id15` | the term's internal id in the store (more in 2.4) |
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
Run Time: real 0.000 user 0.000151 sys 0.000000
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

Each integer is a *term id*. IRIs, language-tagged strings and strings longer than 7 bytes are
stored once, in the dictionary table `rdf_term`, and their id points to that row. Every other
term gets an id that encodes the value itself: integers, decimals, booleans, dates, short strings
such as `"Alice"`, and blank nodes. So `34`, `29`, `"Alice"` and `"Bob"` above never appear in
`rdf_term`. `sparql_term(id)` turns any id back into the term, in N-Triples syntax:

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

The dictionary holds the three IRIs of the example (the predicates and both people) under row
numbers from 1024 on; rows 1 to 63 hold built-in IRIs such as `rdf:type` and the XML Schema
datatypes. A term id is the row number times 16 plus a *tag* in the low 4 bits that says what kind
of term it is (1 = IRI), so row 1024 is id 16385:

```sql
SELECT id AS row, tag, lex, id * 16 + tag AS term_id FROM rdf_term WHERE id >= 1024;
```

```output
┌──────┬─────┬──────────────────────────┬─────────┐
│ row  │ tag │           lex            │ term_id │
├──────┼─────┼──────────────────────────┼─────────┤
│ 1024 │ 1   │ http://example.org/alice │ 16385   │
│ 1025 │ 1   │ http://example.org/name  │ 16401   │
│ 1026 │ 1   │ http://example.org/age   │ 16417   │
│ 1027 │ 1   │ http://example.org/knows │ 16433   │
│ 1028 │ 1   │ http://example.org/bob   │ 16449   │
└──────┴─────┴──────────────────────────┴─────────┘
```

The opposite direction, from a term to its id, is `sparql_term_id()`; there are also helpers that
take an id apart:

```sql
SELECT sparql_term_id('<http://example.org/alice>') AS id,
       sparql_term_kind(sparql_term_id('"Alice"')) AS kind,
       sparql_term_value(sparql_term_id('34')) AS value,
       sparql_term_datatype(sparql_term_id('34')) AS datatype;
```

```output
┌───────┬─────────┬───────┬──────────────────────────────────────────┐
│  id   │  kind   │ value │                 datatype                 │
├───────┼─────────┼───────┼──────────────────────────────────────────┤
│ 16385 │ literal │ 34    │ http://www.w3.org/2001/XMLSchema#integer │
└───────┴─────────┴───────┴──────────────────────────────────────────┘
```

You can read these tables with plain SQL, as above. Do not write to them with plain SQL: change
data with `sparql_load` and `sparql_update` (lesson 5.15), which keep the dictionary, indexes
and statistics consistent.

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
| `true` | a boolean, short for `"true"^^xsd:boolean` |
| `"1997-04-02"^^xsd:date` | a date: the text plus a **datatype** IRI after `^^` |

`xsd:` is the XML Schema datatype vocabulary, `<http://www.w3.org/2001/XMLSchema#>`. A literal is
always text plus either a datatype or a language tag; that is what lets SPARQL compare `34` and
`29` as numbers and dates as dates.

### 3.3 Blank nodes

Sometimes a thing has no name of its own, for example "Bob's chess club membership since 2019".
Turtle writes such an anonymous node in square brackets; inside the store it becomes a **blank
node**, a node with a local, meaningless label such as `_:b1`:

```turtle
ex:bob ex:member [ ex:club "Chess club" ; ex:since 2019 ] .
```

This is three triples: `ex:bob ex:member _:b1`, `_:b1 ex:club "Chess club"` and
`_:b1 ex:since 2019`.

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

Each stored row is now a *quad* (graph, subject, predicate, object), which is why the table is
called `rdf_quad`. SPARQL's `GRAPH` keyword (lesson 5.13) matches the graph name:

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
stays in the project folder):

<!-- verify: setup fresh -->
<!-- verify: ignore-timing -->
```sh
python3 tutorial/make_tutorial_data.py --emit-sql
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
sparql_analyze: {"analyzed_quads":2413,"default_graph":{"characteristic_sets":87,"heavy_hitters":1,"objects":1173,"predicates":31,"subjects":299,"triples":2146},"ms":17,"named_graphs":{"characteristic_sets":5,"heavy_hitters":0,"objects":206,"predicates":10,"subjects":46,"triples":267}}
quads: 2413, dictionary terms: 928, named graphs: 3, job postings: 300
```

Both routes create the same `tutorial/tutorial.db`. Running either one again rebuilds it from
scratch (route 2 says `removed previous copy: ... quads` when the database already held the
data). Run `python3 tutorial/make_tutorial_data.py --help` for all options.

### 4.2 What is in it

| Where | What | Real? |
|---|---|---|
| default graph, from `languages.ttl` | 94 computer languages (91 programming languages and the query languages SQL, XQuery and SPARQL), 117 people who designed them, 40 organisations; for each language its label, first year, designers, developers, the languages that influenced it, paradigms, typing discipline, file extensions, and some homepages and licences | **Real**, checked against the English Wikipedia infoboxes. A missing value means "not recorded here", not "none". |
| default graph, from `languages.ttl` | a small vocabulary: classes such as `ex:ProgrammingLanguage`, a paradigm class hierarchy (`ex:Functional` is a subclass of `ex:Declarative`, ...), and labels in English, French, German, Spanish and Japanese | Real vocabulary, written for this tutorial |
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
in the middle of one. If you leave the shell and come back, repeat these two lines. Some examples change settings; the
text says when, and how to change them back.

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
- Keywords are case-insensitive (`select`, `where`); variable names and IRIs are not.

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
`WHERE`. Comparisons (`=`, `!=`, `<`, `<=`, `>`, `>=`) compare numbers as numbers, strings as
strings and dates as dates; `&&` is "and", `||` is "or", `!` is "not". Languages that appeared
in the 1950s or that are called "Go":

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
argument `"i"` ignores case), plus `STRLEN`, `UCASE` and `LCASE`. Languages whose English
description mentions Apple, in any capitalisation:

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
┌─────────────┬──────────────────────────────────────────────────────────────┐
│  language   │                         description                          │
├─────────────┼──────────────────────────────────────────────────────────────┤
│ Dylan       │ Multi-paradigm language created at Apple, combining Lisp ide │
│             │ as with an ALGOL-like syntax.                                │
├─────────────┼──────────────────────────────────────────────────────────────┤
│ Objective-C │ C with Smalltalk-style messaging; long the main language for │
│             │  Apple platforms.                                            │
├─────────────┼──────────────────────────────────────────────────────────────┤
│ Swift       │ Apple's general-purpose language for its platforms and beyon │
│             │ d.                                                           │
└─────────────┴──────────────────────────────────────────────────────────────┘
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

(`(LANG(?label) AS ?lang)` computes a new column; lesson 5.9 explains such expressions.) Language
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

**Exercise 5.4.** List the people born before 1925 (`schema:birthDate`, an `xsd:date`) with their
date of death (`schema:deathDate`) where one is recorded. Compare dates with a typed literal such
as `"1925-01-01"^^xsd:date` (declare `PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>`).

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
    FILTER (?born < "1925-01-01"^^xsd:date)
  }
  ORDER BY ?born
');
```

```output
┌─────────────────────────┬────────────┬────────────┐
│          name           │    born    │    died    │
├─────────────────────────┼────────────┼────────────┤
│ Grace Hopper            │ 1906-12-09 │ 1992-01-01 │
│ Adriaan van Wijngaarden │ 1916-11-02 │ 1987-02-07 │
│ Christopher Strachey    │ 1916-11-16 │ 1975-05-18 │
│ Kenneth E. Iverson      │ 1920-12-17 │ 2004-10-19 │
│ Alan Perlis             │ 1922-04-01 │ 1990-02-07 │
│ John Backus             │ 1924-12-03 │ 2007-03-17 │
└─────────────────────────┴────────────┴────────────┘
```

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
in the other branch's solutions. The patterns after the `UNION` apply to both branches.

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
differ when nothing is shared. `MINUS` only removes solutions that share at least one variable
with the removed pattern, so a pattern without shared variables removes nothing. `NOT EXISTS`
simply asks "does this pattern match?", which for a pattern without shared variables is the same
question for every solution:

```sql
SELECT v0 AS minus_keeps, v1 AS not_exists_keeps FROM sparql_rows('
  PREFIX ex: <http://example.org/pl/>
  SELECT
    (SUM(IF(?m, 1, 0)) AS ?minus)
    (SUM(IF(?n, 1, 0)) AS ?notexists)
  WHERE {
    { SELECT ?lang (true AS ?m) (false AS ?n) WHERE {
        ?lang a ex:ProgrammingLanguage MINUS { ex:Python ex:designer ?d } } }
    UNION
    { SELECT ?lang (false AS ?m) (true AS ?n) WHERE {
        ?lang a ex:ProgrammingLanguage FILTER NOT EXISTS { ex:Python ex:designer ?d } } }
  }
');
```

```output
┌─────────────┬──────────────────┐
│ minus_keeps │ not_exists_keeps │
├─────────────┼──────────────────┤
│ 91          │ 0                │
└─────────────┴──────────────────┘
```

That query packs both versions into one (with sub-queries from lesson 5.11); it is easier to try
them one at a time with `sparql_count()`, a SQL function that counts a query's solutions:

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
finds no shared variable and removes none. Rule of thumb: use `FILTER NOT EXISTS` when you mean
"there is no ...", and `MINUS` when you mean "take away these solutions". (`FILTER EXISTS` is the
positive form.)

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
are separated by spaces. Unbound values sort first. `LIMIT n` keeps the first n solutions and
`OFFSET n` skips n, which together page through a result. The five most recent languages:

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
SQL. In SPARQL an aggregate must be given a name with `(... AS ?name)`. Per paradigm: how many
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
  ORDER BY DESC(?n)
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

`HAVING` filters groups after aggregation. `SAMPLE(?x)` picks any one value from the group, and
`GROUP_CONCAT(?x; separator=" ")` joins all of them into one string. The people who designed at
least three languages:

```sql
SELECT v0 AS designer, v1 AS languages, v2 AS names FROM sparql_rows('
  PREFIX ex:     <http://example.org/pl/>
  PREFIX rdfs:   <http://www.w3.org/2000/01/rdf-schema#>
  PREFIX schema: <https://schema.org/>
  SELECT ?designer (COUNT(?lang) AS ?n) (GROUP_CONCAT(?language; separator=", ") AS ?names)
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
┌──────────────────┬───────────┬──────────────────────────────────────────────────┐
│     designer     │ languages │                      names                       │
├──────────────────┼───────────┼──────────────────────────────────────────────────┤
│ Niklaus Wirth    │ 5         │ ALGOL W, Pascal, Modula-2, Object Pascal, Oberon │
│ Anders Hejlsberg │ 3         │ Object Pascal, C#, TypeScript                    │
│ John Backus      │ 3         │ Fortran, ALGOL 58, ALGOL 60                      │
└──────────────────┴───────────┴──────────────────────────────────────────────────┘
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
```

`COUNT(*)` counts all solutions, and `COUNT(?older)` skips the ones where `?older` is unbound.
But `SUM`, `AVG`, `MIN`, `MAX` and `GROUP_CONCAT` have **no value at all** for a group in which
their expression fails for some solution, because the variable is unbound there or a value is
not a number. Count, filter, or give missing values a default with `COALESCE` (lesson 5.9):
`SUM(COALESCE(?respondents, 0))` is 100090.

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
