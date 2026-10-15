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

Each integer is a *term id*. IRIs and strings are stored once, in the dictionary table
`rdf_term`; the ids of small numbers, dates, booleans and blank nodes encode the value itself,
so `34` and `29` above never appear in `rdf_term`. `sparql_term(id)` turns any id back into the
term, in N-Triples syntax:

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

```sql
SELECT id, tag, lex FROM rdf_term WHERE id >= 16384;
```

```output
```

(`id >= 16384` skips the 63 built-in rows, such as `rdf:type` and the XML Schema datatypes, that
`sparql_init()` creates. The low 4 bits of an id say what kind of term it is.)

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
data with `sparql_load` and `sparql_update` (section 5.16), which keep the dictionary, indexes
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
');
```

```output
┌───────────────┐
│ sparql_load(' │
├───────────────┤
│ 8             │
└───────────────┘
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
', 'trig');
```

```output
┌───────────────┐
│ sparql_load(' │
├───────────────┤
│ 3             │
└───────────────┘
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
.mode box
```

If you leave the shell and come back, repeat these two lines. Some examples change settings; the
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
