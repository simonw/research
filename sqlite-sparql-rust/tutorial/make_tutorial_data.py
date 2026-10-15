#!/usr/bin/env python3
"""Generate (and optionally load) the example data for the sparqlite tutorial.

Python 3.8+ standard library only.  Every run writes byte-identical files.

What it writes (into tutorial/data/ next to this script, or --out DIR):

  languages.ttl     REAL facts: ~90 programming languages, their designers and
                    developer organisations, "influenced by" edges, a paradigm class
                    hierarchy, typing disciplines, file extensions, some homepages
                    and licences.  The core graph of the tutorial (default graph).
  survey.trig       FICTIONAL: three named graphs -- a made-up developer survey for
                    2025 and 2026 and an "editorial" graph of reviews.  Metadata
                    about the graphs goes in the default graph.
  job_postings.csv  FICTIONAL: an ordinary relational table of job adverts, for
  job_postings.sql  joining SPARQL results with plain SQL (CREATE TABLE + INSERT).
  people.nt         SYNTHETIC (only with --people N): a social network of N
                    people (about 8.4 triples each) for the performance section.
  load.sql          (only with --emit-sql) a script for the sqlite3 shell that
                    loads all of the above with readfile().

Three ways to get a database:

1. Python does everything (needs a Python whose sqlite3 module can load
   extensions; most Linux and Homebrew builds can, macOS's system Python can't):

     python3 tutorial/make_tutorial_data.py --db tutorial.db \\
         --ext ext/target/release/libsparqlite

2. The sqlite3 shell does the loading (works everywhere the shell does):

     python3 tutorial/make_tutorial_data.py --emit-sql
     cd tutorial/data
     sqlite3 ../tutorial.db
     sqlite> .load ../../ext/target/release/libsparqlite
     sqlite> .read load.sql

   readfile() resolves names against the shell's current directory, which is
   why load.sql must be run from inside the data directory.

3. By hand, one file at a time, in any sqlite3 session with the extension
   loaded:  SELECT sparql_load(readfile('languages.ttl'));

Re-running is safe: --db rebuilds the database from scratch, and load.sql
removes an earlier copy of the tutorial data (SPARQL CLEAR ALL) before loading,
so give the tutorial its own database file.
"""

from __future__ import annotations

import argparse
import bisect
import csv
import datetime as dt
import io
import os
import sqlite3
import sys
import tempfile
import time

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
DEFAULT_OUT = os.path.join(SCRIPT_DIR, "data")
GENERATOR_VERSION = "1"

# ---------------------------------------------------------------------------
# Namespaces
# ---------------------------------------------------------------------------

EX = "http://example.org/pl/"
PERSON = EX + "person/"
ORG = EX + "org/"
GRAPH_NS = "http://example.org/graph/"
PEOPLE = "http://example.org/people/"
CITY = "http://example.org/city/"
SPDX = "http://spdx.org/licenses/"

RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
RDFS = "http://www.w3.org/2000/01/rdf-schema#"
XSD = "http://www.w3.org/2001/XMLSchema#"
SCHEMA = "https://schema.org/"
DCTERMS = "http://purl.org/dc/terms/"
SKOS = "http://www.w3.org/2004/02/skos/core#"
FOAF = "http://xmlns.com/foaf/0.1/"

PREFIXES = [
    ("rdf", RDF), ("rdfs", RDFS), ("xsd", XSD), ("schema", SCHEMA),
    ("dcterms", DCTERMS), ("skos", SKOS), ("ex", EX), ("person", PERSON),
    ("org", ORG),
]

# ---------------------------------------------------------------------------
# Curated, real-world data.  Sources: the infoboxes of the English Wikipedia
# articles ("First appeared", "Designed by", "Developer", "Influenced by",
# "Typing discipline", "Filename extensions", "License", "Website").  Anything
# uncertain was left out rather than guessed.
# ---------------------------------------------------------------------------

# slug -> (name, birthDate, deathDate, Turing Award year).  Dates only where they
# are well documented; a missing deathDate means "not recorded here", nothing more.
PERSONS = {
    "john-backus": ("John Backus", "1924-12-03", "2007-03-17", 1977),
    "grace-hopper": ("Grace Hopper", "1906-12-09", "1992-01-01", None),
    "john-mccarthy": ("John McCarthy", "1927-09-04", "2011-10-24", 1971),
    "jean-sammet": ("Jean E. Sammet", "1928-03-23", "2017-05-22", None),
    "peter-naur": ("Peter Naur", "1928-10-25", "2016-01-03", 2005),
    "alan-perlis": ("Alan Perlis", "1922-04-01", "1990-02-07", 1966),
    "adriaan-van-wijngaarden": ("Adriaan van Wijngaarden", "1916-11-02", "1987-02-07", None),
    "ole-johan-dahl": ("Ole-Johan Dahl", "1931-10-12", "2002-06-29", 2001),
    "kristen-nygaard": ("Kristen Nygaard", "1926-08-27", "2002-08-10", 2001),
    "christopher-strachey": ("Christopher Strachey", "1916-11-16", "1975-05-18", None),
    "john-kemeny": ("John G. Kemeny", "1926-05-31", "1992-12-26", None),
    "thomas-kurtz": ("Thomas E. Kurtz", "1928-02-22", "2024-11-12", None),
    "kenneth-iverson": ("Kenneth E. Iverson", "1920-12-17", "2004-10-19", 1979),
    "roger-hui": ("Roger Hui", None, None, None),
    "peter-landin": ("Peter Landin", "1930-06-05", "2009-06-03", None),
    "niklaus-wirth": ("Niklaus Wirth", "1934-02-15", "2024-01-01", 1984),
    "tony-hoare": ("Tony Hoare", "1934-01-11", None, 1980),
    "martin-richards": ("Martin Richards", "1940-07-21", None, None),
    "wally-feurzeig": ("Wally Feurzeig", None, None, None),
    "seymour-papert": ("Seymour Papert", "1928-02-29", "2016-07-31", None),
    "cynthia-solomon": ("Cynthia Solomon", None, None, None),
    "ken-thompson": ("Ken Thompson", "1943-02-04", None, 1983),
    "dennis-ritchie": ("Dennis Ritchie", "1941-09-09", None, 1983),
    "charles-moore": ("Charles H. Moore", None, None, None),
    "alan-kay": ("Alan Kay", "1940-05-17", None, 2003),
    "dan-ingalls": ("Dan Ingalls", None, None, None),
    "adele-goldberg": ("Adele Goldberg", "1945-07-07", None, None),
    "alain-colmerauer": ("Alain Colmerauer", "1941-01-24", "2017-05-12", None),
    "robin-milner": ("Robin Milner", "1934-01-13", "2010-03-20", 1991),
    "donald-chamberlin": ("Donald D. Chamberlin", None, None, None),
    "raymond-boyce": ("Raymond F. Boyce", None, None, None),
    "guy-steele": ("Guy L. Steele Jr.", "1954-10-02", None, None),
    "gerald-sussman": ("Gerald Jay Sussman", "1947-02-08", None, None),
    "barbara-liskov": ("Barbara Liskov", "1939-11-07", None, 2008),
    "john-chambers": ("John Chambers", None, None, None),
    "alfred-aho": ("Alfred Aho", "1941-08-09", None, 2020),
    "peter-weinberger": ("Peter J. Weinberger", None, None, None),
    "brian-kernighan": ("Brian Kernighan", "1942-01-30", None, None),
    "david-farber": ("David J. Farber", None, None, None),
    "ralph-griswold": ("Ralph E. Griswold", None, None, None),
    "ivan-polonsky": ("Ivan P. Polonsky", None, None, None),
    "jean-ichbiah": ("Jean Ichbiah", "1940-03-25", "2007-01-26", None),
    "john-warnock": ("John Warnock", "1940-10-06", "2023-08-19", None),
    "charles-geschke": ("Charles Geschke", "1939-09-11", "2021-04-16", None),
    "tom-love": ("Tom Love", None, None, None),
    "brad-cox": ("Brad Cox", None, None, None),
    "scott-fahlman": ("Scott Fahlman", None, None, None),
    "richard-gabriel": ("Richard P. Gabriel", None, None, None),
    "kent-pitman": ("Kent Pitman", None, None, None),
    "cleve-moler": ("Cleve Moler", "1939-08-17", None, None),
    "bjarne-stroustrup": ("Bjarne Stroustrup", "1950-12-30", None, None),
    "david-turner": ("David Turner", None, None, None),
    "larry-tesler": ("Larry Tesler", "1945-04-24", "2020-02-16", None),
    "anders-hejlsberg": ("Anders Hejlsberg", "1960-12-02", None, None),
    "bertrand-meyer": ("Bertrand Meyer", "1950-11-21", None, None),
    "joe-armstrong": ("Joe Armstrong", "1950-12-27", "2019-04-20", None),
    "robert-virding": ("Robert Virding", None, None, None),
    "mike-williams": ("Mike Williams", None, None, None),
    "david-ungar": ("David Ungar", None, None, None),
    "randall-smith": ("Randall Smith", None, None, None),
    "larry-wall": ("Larry Wall", "1954-09-27", None, None),
    "leo-geurts": ("Leo Geurts", None, None, None),
    "lambert-meertens": ("Lambert Meertens", None, None, None),
    "steven-pemberton": ("Steven Pemberton", None, None, None),
    "john-ousterhout": ("John Ousterhout", "1954-10-15", None, None),
    "luca-cardelli": ("Luca Cardelli", None, None, None),
    "greg-nelson": ("Greg Nelson", None, None, None),
    "stephen-wolfram": ("Stephen Wolfram", "1959-08-29", None, None),
    "simon-peyton-jones": ("Simon Peyton Jones", None, None, None),
    "philip-wadler": ("Philip Wadler", "1956-04-08", None, None),
    "paul-hudak": ("Paul Hudak", "1952-07-15", "2015-04-29", None),
    "john-hughes": ("John Hughes", None, None, None),
    "guido-van-rossum": ("Guido van Rossum", "1956-01-31", None, None),
    "roberto-ierusalimschy": ("Roberto Ierusalimschy", None, None, None),
    "waldemar-celes": ("Waldemar Celes", None, None, None),
    "luiz-henrique-de-figueiredo": ("Luiz Henrique de Figueiredo", None, None, None),
    "ross-ihaka": ("Ross Ihaka", None, None, None),
    "robert-gentleman": ("Robert Gentleman", None, None, None),
    "arthur-whitney": ("Arthur Whitney", None, None, None),
    "yukihiro-matsumoto": ("Yukihiro Matsumoto", "1965-04-14", None, None),
    "james-gosling": ("James Gosling", "1955-05-19", None, None),
    "brendan-eich": ("Brendan Eich", "1961-07-04", None, None),
    "rasmus-lerdorf": ("Rasmus Lerdorf", "1968-11-22", None, None),
    "sean-dorward": ("Sean Dorward", None, None, None),
    "phil-winterbottom": ("Phil Winterbottom", None, None, None),
    "rob-pike": ("Rob Pike", None, None, None),
    "zoltan-somogyi": ("Zoltan Somogyi", None, None, None),
    "xavier-leroy": ("Xavier Leroy", None, None, None),
    "walter-bright": ("Walter Bright", None, None, None),
    "andrei-alexandrescu": ("Andrei Alexandrescu", None, None, None),
    "james-strachan": ("James Strachan", None, None, None),
    "martin-odersky": ("Martin Odersky", None, None, None),
    "don-syme": ("Don Syme", None, None, None),
    "jeffrey-snover": ("Jeffrey Snover", None, None, None),
    "rich-hickey": ("Rich Hickey", None, None, None),
    "jonathan-robie": ("Jonathan Robie", None, None, None),
    "daniela-florescu": ("Daniela Florescu", None, None, None),
    "andreas-rumpf": ("Andreas Rumpf", None, None, None),
    "robert-griesemer": ("Robert Griesemer", None, None, None),
    "jeremy-ashkenas": ("Jeremy Ashkenas", None, None, None),
    "graydon-hoare": ("Graydon Hoare", None, None, None),
    "andrey-breslav": ("Andrey Breslav", None, None, None),
    "lars-bak": ("Lars Bak", None, None, None),
    "kasper-lund": ("Kasper Lund", None, None, None),
    "jeff-bezanson": ("Jeff Bezanson", None, None, None),
    "stefan-karpinski": ("Stefan Karpinski", None, None, None),
    "viral-shah": ("Viral B. Shah", None, None, None),
    "alan-edelman": ("Alan Edelman", None, None, None),
    "jose-valim": ("José Valim", None, None, None),
    "evan-czaplicki": ("Evan Czaplicki", None, None, None),
    "luke-hoban": ("Luke Hoban", None, None, None),
    "chris-lattner": ("Chris Lattner", None, None, None),
    "ary-borenszweig": ("Ary Borenszweig", None, None, None),
    "juan-wajnerman": ("Juan Wajnerman", None, None, None),
    "brian-cardiff": ("Brian Cardiff", None, None, None),
    "andrew-kelley": ("Andrew Kelley", None, None, None),
    "louis-pilfold": ("Louis Pilfold", None, None, None),
}

# slug -> (name, [alternative names])
ORGS = {
    "ibm": ("IBM", ["International Business Machines"]),
    "remington-rand": ("Remington Rand", []),
    "codasyl": ("CODASYL", ["Conference on Data Systems Languages"]),
    "bell-labs": ("Bell Labs", ["Bell Telephone Laboratories"]),
    "norwegian-computing-center": ("Norwegian Computing Center", []),
    "dartmouth-college": ("Dartmouth College", []),
    "xerox-parc": ("Xerox PARC", ["Xerox Palo Alto Research Center"]),
    "mit": ("Massachusetts Institute of Technology", ["MIT"]),
    "university-of-edinburgh": ("University of Edinburgh", []),
    "eth-zurich": ("ETH Zurich", []),
    "inria": ("Inria", []),
    "ericsson": ("Ericsson", []),
    "adobe": ("Adobe", []),
    "apple": ("Apple", ["Apple Computer", "Apple Inc."]),
    "microsoft": ("Microsoft", []),
    "sun-microsystems": ("Sun Microsystems", []),
    "oracle": ("Oracle", []),
    "netscape": ("Netscape", []),
    "mozilla": ("Mozilla", []),
    "google": ("Google", []),
    "jetbrains": ("JetBrains", []),
    "meta": ("Meta Platforms", ["Facebook"]),
    "wolfram-research": ("Wolfram Research", []),
    "mathworks": ("MathWorks", []),
    "puc-rio": ("Pontifical Catholic University of Rio de Janeiro", ["PUC-Rio"]),
    "cwi": ("Centrum Wiskunde & Informatica", ["CWI"]),
    "psf": ("Python Software Foundation", ["PSF"]),
    "kx-systems": ("Kx Systems", []),
    "university-of-melbourne": ("University of Melbourne", []),
    "dec": ("Digital Equipment Corporation", ["DEC"]),
    "olivetti": ("Olivetti", []),
    "att-labs": ("AT&T Labs", []),
    "cornell-university": ("Cornell University", []),
    "apache": ("Apache Software Foundation", []),
    "epfl": ("EPFL", ["École Polytechnique Fédérale de Lausanne"]),
    "manas": ("Manas Technology Solutions", []),
    "zig-software-foundation": ("Zig Software Foundation", []),
    "modular": ("Modular", ["Modular Inc."]),
    "w3c": ("World Wide Web Consortium", ["W3C"]),
    "plt": ("PLT Inc.", []),
}

# Paradigm class hierarchy: local name -> (superclass, {lang: label}, comment)
PARADIGMS = [
    ("Paradigm", None, {"en": "programming paradigm"},
     "Root of the paradigm hierarchy. Languages point at paradigms with ex:paradigm."),
    ("Imperative", "Paradigm",
     {"en": "imperative programming", "fr": "programmation impérative",
      "de": "imperative Programmierung", "es": "programación imperativa",
      "ja": "命令型プログラミング"},
     "Programs are sequences of statements that change state."),
    ("Procedural", "Imperative",
     {"en": "procedural programming", "fr": "programmation procédurale",
      "de": "prozedurale Programmierung", "es": "programación procedimental",
      "ja": "手続き型プログラミング"},
     "Imperative programming organised into procedures."),
    ("ObjectOriented", "Imperative",
     {"en": "object-oriented programming", "fr": "programmation orientée objet",
      "de": "objektorientierte Programmierung", "es": "programación orientada a objetos",
      "ja": "オブジェクト指向プログラミング"},
     "Programs are built from objects that combine state and behaviour."),
    ("ClassBased", "ObjectOriented", {"en": "class-based programming"},
     "Object orientation where objects are instances of classes."),
    ("PrototypeBased", "ObjectOriented",
     {"en": "prototype-based programming", "de": "prototypenbasierte Programmierung"},
     "Object orientation without classes: objects inherit from other objects."),
    ("Declarative", "Paradigm",
     {"en": "declarative programming", "fr": "programmation déclarative",
      "de": "deklarative Programmierung", "es": "programación declarativa",
      "ja": "宣言型プログラミング"},
     "Programs describe what to compute rather than how."),
    ("Functional", "Declarative",
     {"en": "functional programming", "fr": "programmation fonctionnelle",
      "de": "funktionale Programmierung", "es": "programación funcional",
      "ja": "関数型プログラミング"},
     "Programs are built by applying and composing functions."),
    ("PurelyFunctional", "Functional", {"en": "purely functional programming"},
     "Functional programming without side effects in ordinary functions."),
    ("Logic", "Declarative",
     {"en": "logic programming", "fr": "programmation logique",
      "de": "logische Programmierung", "es": "programación lógica",
      "ja": "論理プログラミング"},
     "Programs are sets of logical facts and rules."),
    ("Concurrent", "Paradigm",
     {"en": "concurrent programming", "fr": "programmation concurrente",
      "es": "programación concurrente"},
     "Language-level support for computations that run at the same time."),
    ("ArrayProgramming", "Paradigm", {"en": "array programming"},
     "Operations apply to whole arrays at once."),
    ("Concatenative", "Paradigm", {"en": "concatenative (stack-based) programming"},
     "Programs are compositions of words that operate on a stack."),
]
# A British-English variant, so LANGMATCHES(LANG(?l), "en") has something to match
# that LANG(?l) = "en" does not.
PARADIGM_ALT = {"ObjectOriented": [("object-orientated programming", "en-GB")]}

TYPING = {
    "static": ("StaticTyping", "static typing", "Types are checked before the program runs."),
    "dynamic": ("DynamicTyping", "dynamic typing", "Types are checked while the program runs."),
    "gradual": ("GradualTyping", "gradual typing",
                "Type annotations are optional; annotated code is checked statically."),
    "untyped": ("Untyped", "untyped", "The language has a single data type (typically the machine word)."),
}
STRENGTH = {
    "strong": ("StrongTyping", "strong typing", "Few implicit conversions between unrelated types."),
    "weak": ("WeakTyping", "weak typing", "Many implicit conversions between types."),
}

LICENSES = {
    "MIT": "MIT License",
    "Apache-2.0": "Apache License 2.0",
    "BSD-2-Clause": "BSD 2-Clause License",
    "BSD-3-Clause": "BSD 3-Clause License",
    "PSF-2.0": "Python Software Foundation License 2.0",
    "EPL-1.0": "Eclipse Public License 1.0",
    "Ruby": "Ruby License",
    "PHP-3.01": "PHP License v3.01",
    "BSL-1.0": "Boost Software License 1.0",
}

# Short descriptions in other languages for a subset (LANG()/LANGMATCHES lessons).
I18N = {
    "systems": {"fr": "langage de programmation système", "de": "Systemprogrammiersprache",
                "es": "lenguaje de programación de sistemas", "ja": "システムプログラミング言語"},
    "functional": {"fr": "langage de programmation fonctionnel",
                   "de": "funktionale Programmiersprache",
                   "es": "lenguaje de programación funcional", "ja": "関数型プログラミング言語"},
    "oo": {"fr": "langage de programmation orienté objet",
           "de": "objektorientierte Programmiersprache",
           "es": "lenguaje de programación orientado a objetos",
           "ja": "オブジェクト指向プログラミング言語"},
    "logic": {"fr": "langage de programmation logique", "de": "logische Programmiersprache",
              "es": "lenguaje de programación lógica"},
    "query": {"fr": "langage de requête", "de": "Abfragesprache", "es": "lenguaje de consulta",
              "ja": "問い合わせ言語"},
    # Ruby's official Japanese tagline.
    "ruby": {"ja": "オブジェクト指向スクリプト言語"},
}

LANGUAGES = []


def lang(key, label, year, *, desc, designers=(), developers=(), influenced_by=(),
         paradigms=(), typing=None, strength=None, inference=False, ext=(), url=None,
         licenses=(), alt=(), comment=None, i18n=None, first_release=None, v1=None,
         cls="ProgrammingLanguage"):
    LANGUAGES.append(dict(
        key=key, label=label, year=year, desc=desc, designers=list(designers),
        developers=list(developers), influenced_by=list(influenced_by),
        paradigms=list(paradigms), typing=typing, strength=strength, inference=inference,
        ext=list(ext), url=url, licenses=list(licenses), alt=list(alt), comment=comment,
        i18n=i18n, first_release=first_release, v1=v1, cls=cls))


# ----- 1950s -----
lang("FlowMatic", "FLOW-MATIC", 1955,
     desc="Early English-like data-processing language developed under Grace Hopper for the UNIVAC I.",
     designers=["grace-hopper"], developers=["remington-rand"], paradigms=["Imperative"],
     alt=["B-0"])
lang("Fortran", "Fortran", 1957,
     desc="Compiled imperative language for numeric and scientific computing, developed at IBM.",
     designers=["john-backus"], developers=["ibm"], paradigms=["Procedural", "ArrayProgramming"],
     typing="static", strength="strong", ext=[".f", ".for", ".f90"],
     url="https://fortran-lang.org/", alt=["FORTRAN"],
     comment="The name derives from 'Formula Translation'.")
lang("ALGOL58", "ALGOL 58", 1958,
     desc="The first ALGOL, originally called the International Algebraic Language (IAL).",
     designers=["john-backus", "alan-perlis"], influenced_by=["Fortran"],
     paradigms=["Procedural"], alt=["IAL"])
lang("COBOL", "COBOL", 1959,
     desc="Common Business-Oriented Language, designed for business data processing.",
     designers=["jean-sammet"], developers=["codasyl"], influenced_by=["FlowMatic"],
     paradigms=["Procedural"], typing="static", strength="weak",
     ext=[".cbl", ".cob", ".cpy"], alt=["Common Business-Oriented Language"])
# ----- 1960s -----
lang("Lisp", "Lisp", 1960,
     desc="Family of languages built around list processing, where code is data.",
     designers=["john-mccarthy"], paradigms=["Functional", "Procedural"],
     typing="dynamic", strength="strong", alt=["LISP"],
     comment="The name derives from 'LISt Processor'.")
lang("ALGOL60", "ALGOL 60", 1960,
     desc="Block-structured algorithmic language whose report popularised Backus-Naur form.",
     designers=["john-backus", "peter-naur", "alan-perlis", "john-mccarthy",
                "adriaan-van-wijngaarden"],
     influenced_by=["ALGOL58"], paradigms=["Procedural"], typing="static")
lang("SNOBOL", "SNOBOL", 1962,
     desc="String-oriented language with built-in pattern matching, developed at Bell Labs.",
     designers=["david-farber", "ralph-griswold", "ivan-polonsky"], developers=["bell-labs"],
     paradigms=["Imperative"], typing="dynamic")
lang("Simula", "Simula", 1962,
     desc="Simulation language that introduced classes, objects and inheritance.",
     designers=["ole-johan-dahl", "kristen-nygaard"], developers=["norwegian-computing-center"],
     influenced_by=["ALGOL60"], paradigms=["Procedural", "ClassBased"], typing="static",
     i18n="oo")
lang("CPL", "CPL", 1963,
     desc="Combined Programming Language, developed by the universities of Cambridge and London.",
     designers=["christopher-strachey"], influenced_by=["ALGOL60"], paradigms=["Procedural"],
     alt=["Combined Programming Language"])
lang("BASIC", "BASIC", 1964,
     desc="Beginners' All-purpose Symbolic Instruction Code, created at Dartmouth College for teaching.",
     designers=["john-kemeny", "thomas-kurtz"], developers=["dartmouth-college"],
     influenced_by=["ALGOL60", "Fortran"], paradigms=["Procedural"],
     alt=["Beginners' All-purpose Symbolic Instruction Code"], first_release="1964-05-01")
lang("PLI", "PL/I", 1964,
     desc="Programming Language One, IBM's language for both scientific and business work.",
     developers=["ibm"], influenced_by=["COBOL", "Fortran", "ALGOL60"],
     paradigms=["Procedural"], alt=["PL/1"])
lang("APL", "APL", 1966,
     desc="Array language with a concise notation built from special symbols.",
     designers=["kenneth-iverson"], developers=["ibm"],
     paradigms=["ArrayProgramming", "Functional"], typing="dynamic",
     alt=["A Programming Language"],
     comment="Named after Kenneth Iverson's 1962 book 'A Programming Language'.")
lang("ISWIM", "ISWIM", 1966,
     desc="Peter Landin's abstract language that shaped later functional languages.",
     designers=["peter-landin"], influenced_by=["ALGOL60", "Lisp"], paradigms=["Functional"],
     alt=["If you See What I Mean"])
lang("ALGOLW", "ALGOL W", 1966,
     desc="Niklaus Wirth and Tony Hoare's proposed successor to ALGOL 60.",
     designers=["niklaus-wirth", "tony-hoare"], influenced_by=["ALGOL60"],
     paradigms=["Procedural"], typing="static")
lang("BCPL", "BCPL", 1967,
     desc="Basic Combined Programming Language, a typeless language for writing compilers and systems.",
     designers=["martin-richards"], influenced_by=["CPL"], paradigms=["Procedural"],
     typing="untyped", alt=["Basic Combined Programming Language"])
lang("Logo", "Logo", 1967,
     desc="Educational language best known for turtle graphics.",
     designers=["wally-feurzeig", "seymour-papert", "cynthia-solomon"], influenced_by=["Lisp"],
     paradigms=["Functional", "Procedural"], typing="dynamic")
lang("ALGOL68", "ALGOL 68", 1968,
     desc="Successor to ALGOL 60 with an orthogonal design, defined by a two-level grammar.",
     designers=["adriaan-van-wijngaarden"], influenced_by=["ALGOL60"],
     paradigms=["Procedural"], typing="static", strength="strong")
lang("B", "B", 1969,
     desc="Typeless language from Bell Labs derived from BCPL; the predecessor of C.",
     designers=["ken-thompson"], developers=["bell-labs"], influenced_by=["BCPL", "PLI"],
     paradigms=["Procedural"], typing="untyped")
# ----- 1970s -----
lang("Pascal", "Pascal", 1970,
     desc="Small structured language designed by Niklaus Wirth, widely used for teaching.",
     designers=["niklaus-wirth"], developers=["eth-zurich"], influenced_by=["ALGOLW", "Simula"],
     paradigms=["Procedural"], typing="static", strength="strong", ext=[".pas", ".pp"],
     comment="Named after the mathematician Blaise Pascal.")
lang("Forth", "Forth", 1970,
     desc="Stack-based, concatenative language with an interactive interpreter.",
     designers=["charles-moore"], paradigms=["Concatenative", "Procedural"], typing="untyped",
     ext=[".fs", ".fth", ".4th"])
lang("C", "C", 1972,
     desc="Systems programming language developed at Bell Labs alongside Unix.",
     designers=["dennis-ritchie"], developers=["bell-labs"],
     influenced_by=["B", "ALGOL68", "PLI", "Fortran"], paradigms=["Procedural"],
     typing="static", strength="weak", ext=[".c", ".h"], i18n="systems")
lang("Smalltalk", "Smalltalk", 1972,
     desc="Purely object-oriented language and live programming environment from Xerox PARC.",
     designers=["alan-kay", "dan-ingalls", "adele-goldberg"], developers=["xerox-parc"],
     influenced_by=["Lisp", "Simula", "Logo"], paradigms=["ClassBased"],
     typing="dynamic", strength="strong", ext=[".st"], i18n="oo")
lang("Prolog", "Prolog", 1972,
     desc="Logic programming language based on Horn clauses and backtracking search.",
     designers=["alain-colmerauer"], paradigms=["Logic"], ext=[".pl", ".pro"], i18n="logic",
     comment="The name comes from the French 'programmation en logique'.")
lang("ML", "ML", 1973,
     desc="Meta Language of the LCF theorem prover, with Hindley-Milner type inference.",
     designers=["robin-milner"], developers=["university-of-edinburgh"], influenced_by=["ISWIM"],
     paradigms=["Functional"], typing="static", strength="strong", inference=True,
     alt=["Meta Language"])
lang("SQL", "SQL", 1974,
     desc="Declarative language for querying and managing relational databases.",
     designers=["donald-chamberlin", "raymond-boyce"], developers=["ibm"],
     paradigms=["Declarative"], ext=[".sql"], alt=["SEQUEL", "Structured Query Language"],
     i18n="query", cls="QueryLanguage")
lang("Scheme", "Scheme", 1975,
     desc="Minimalist dialect of Lisp with lexical scope and proper tail calls.",
     designers=["guy-steele", "gerald-sussman"], developers=["mit"],
     influenced_by=["Lisp", "ALGOL60"], paradigms=["Functional", "Procedural"],
     typing="dynamic", strength="strong", ext=[".scm", ".ss"])
lang("CLU", "CLU", 1975,
     desc="Language from MIT that introduced abstract data types (clusters) and iterators.",
     designers=["barbara-liskov"], developers=["mit"],
     influenced_by=["ALGOL60", "Lisp", "Simula"], paradigms=["ObjectOriented", "Procedural"],
     typing="static", strength="strong", comment="The name derives from 'cluster'.")
lang("Mesa", "Mesa", 1976,
     desc="Modular systems programming language developed at Xerox PARC.",
     developers=["xerox-parc"], paradigms=["Procedural"])
lang("S", "S", 1976,
     desc="Statistical programming language developed at Bell Labs; the ancestor of R.",
     designers=["john-chambers"], developers=["bell-labs"],
     paradigms=["Imperative", "ObjectOriented"])
lang("AWK", "AWK", 1977,
     desc="Pattern-action language for processing text files.",
     designers=["alfred-aho", "peter-weinberger", "brian-kernighan"], developers=["bell-labs"],
     influenced_by=["C", "SNOBOL"], paradigms=["Procedural"], ext=[".awk"], alt=["awk"],
     comment="Named after the initials of its designers: Aho, Weinberger and Kernighan.")
lang("Icon", "Icon", 1977,
     desc="High-level language with goal-directed evaluation and generators.",
     designers=["ralph-griswold"], influenced_by=["SNOBOL"], paradigms=["Procedural"],
     typing="dynamic", ext=[".icn"])
lang("Modula2", "Modula-2", 1978,
     desc="Niklaus Wirth's modular successor to Pascal.",
     designers=["niklaus-wirth"], developers=["eth-zurich"],
     influenced_by=["Pascal", "Mesa", "ALGOLW"], paradigms=["Procedural"],
     typing="static", strength="strong", ext=[".mod", ".def"])
# ----- 1980s -----
lang("Ada", "Ada", 1980,
     desc="Language commissioned by the US Department of Defense for large, reliable systems.",
     designers=["jean-ichbiah"], influenced_by=["Pascal", "ALGOL68", "Simula"],
     paradigms=["Procedural", "ObjectOriented", "Concurrent"], typing="static",
     strength="strong", ext=[".adb", ".ads"],
     comment="Named after Ada Lovelace.")
lang("PostScript", "PostScript", 1982,
     desc="Page description language and stack-based programming language from Adobe.",
     designers=["john-warnock", "charles-geschke"], developers=["adobe"],
     paradigms=["Concatenative"], ext=[".ps"])
lang("StandardML", "Standard ML", 1983,
     desc="Standardised dialect of ML with a formal definition.",
     designers=["robin-milner"], influenced_by=["ML"], paradigms=["Functional"],
     typing="static", strength="strong", inference=True, ext=[".sml"], alt=["SML"])
lang("ObjectiveC", "Objective-C", 1984,
     desc="C with Smalltalk-style messaging; long the main language for Apple platforms.",
     designers=["tom-love", "brad-cox"], developers=["apple"], influenced_by=["C", "Smalltalk"],
     paradigms=["ClassBased"], ext=[".m", ".mm", ".h"], alt=["ObjC"])
lang("CommonLisp", "Common Lisp", 1984,
     desc="ANSI-standardised dialect of Lisp.",
     designers=["scott-fahlman", "richard-gabriel", "kent-pitman", "guy-steele"],
     influenced_by=["Lisp", "Scheme"], paradigms=["Functional", "Procedural", "ClassBased"],
     typing="dynamic", strength="strong", ext=[".lisp", ".lsp", ".cl"], alt=["CL"])
# MATLAB's infobox gives "late 1970s" rather than a year, so it has no ex:firstAppeared.
lang("MATLAB", "MATLAB", None,
     desc="Numerical computing language and environment from MathWorks.",
     designers=["cleve-moler"], developers=["mathworks"],
     paradigms=["ArrayProgramming", "Procedural", "ObjectOriented"], ext=[".m"],
     url="https://www.mathworks.com/products/matlab.html", alt=["MATrix LABoratory"],
     comment="First appeared in the late 1970s; no single year is recorded here.")
lang("CPlusPlus", "C++", 1985,
     desc="Extension of C with classes, templates and zero-overhead abstractions.",
     designers=["bjarne-stroustrup"], developers=["bell-labs"],
     influenced_by=["C", "Simula", "ALGOL68", "Ada", "ML", "CLU", "BCPL"],
     paradigms=["Procedural", "ClassBased"], typing="static",
     ext=[".cpp", ".cc", ".cxx", ".hpp", ".hh", ".h"], alt=["C with Classes"])
lang("Caml", "Caml", 1985,
     desc="Dialect of ML developed at Inria.", developers=["inria"], influenced_by=["ML"],
     paradigms=["Functional"], typing="static", strength="strong", inference=True)
lang("Miranda", "Miranda", 1985,
     desc="Lazy, purely functional language by David Turner.", designers=["david-turner"],
     influenced_by=["ML"], paradigms=["PurelyFunctional"], typing="static", strength="strong",
     inference=True)
lang("ObjectPascal", "Object Pascal", 1986,
     desc="Object-oriented extension of Pascal, later the language of Delphi.",
     designers=["larry-tesler", "niklaus-wirth", "anders-hejlsberg"], developers=["apple"],
     influenced_by=["Pascal", "Smalltalk", "Simula"], paradigms=["Procedural", "ClassBased"],
     typing="static", ext=[".pas", ".pp"])
lang("Eiffel", "Eiffel", 1986,
     desc="Object-oriented language built around design by contract.",
     designers=["bertrand-meyer"], influenced_by=["Ada", "Simula"], paradigms=["ClassBased"],
     typing="static", strength="strong", ext=[".e"], url="https://www.eiffel.org/",
     comment="Named after the engineer Gustave Eiffel.")
lang("Erlang", "Erlang", 1986,
     desc="Concurrent functional language for fault-tolerant telecom systems, from Ericsson.",
     designers=["joe-armstrong", "robert-virding", "mike-williams"], developers=["ericsson"],
     influenced_by=["Lisp", "Prolog", "Smalltalk"], paradigms=["Functional", "Concurrent"],
     typing="dynamic", strength="strong", ext=[".erl", ".hrl"], url="https://www.erlang.org/",
     licenses=["Apache-2.0"], i18n="functional")
lang("Self", "Self", 1987,
     desc="Prototype-based object-oriented language derived from Smalltalk.",
     designers=["david-ungar", "randall-smith"], influenced_by=["Smalltalk"],
     paradigms=["PrototypeBased"], typing="dynamic")
lang("Perl", "Perl", 1987,
     desc="Text-processing and general-purpose scripting language by Larry Wall.",
     designers=["larry-wall"], influenced_by=["AWK", "BASIC", "C", "CPlusPlus", "Lisp", "Pascal"],
     paradigms=["Procedural", "ClassBased", "Functional"], typing="dynamic",
     ext=[".pl", ".pm"], url="https://www.perl.org/", first_release="1987-12-18",
     comment="Officially not an acronym; later backronymed as 'Practical Extraction and Report Language'.")
lang("ABC", "ABC", 1987,
     desc="Teaching language from CWI that inspired Python.",
     designers=["leo-geurts", "lambert-meertens", "steven-pemberton"], developers=["cwi"],
     influenced_by=["ALGOL68"], paradigms=["Procedural"])
lang("Oberon", "Oberon", 1987,
     desc="Niklaus Wirth's compact successor to Modula-2.",
     designers=["niklaus-wirth"], developers=["eth-zurich"], influenced_by=["Modula2"],
     paradigms=["Procedural", "ObjectOriented"], typing="static", strength="strong")
lang("Tcl", "Tcl", 1988,
     desc="Tool Command Language, an embeddable scripting language.",
     designers=["john-ousterhout"], influenced_by=["AWK", "Lisp"], paradigms=["Procedural"],
     typing="dynamic", ext=[".tcl"], url="https://www.tcl-lang.org/",
     alt=["Tool Command Language"])
lang("Modula3", "Modula-3", 1988,
     desc="Successor to Modula-2 developed at DEC and Olivetti.",
     designers=["luca-cardelli", "greg-nelson"], developers=["dec", "olivetti"],
     influenced_by=["Modula2", "Pascal", "Mesa"], paradigms=["Procedural", "ObjectOriented"],
     typing="static", strength="strong", ext=[".m3", ".i3"])
lang("WolframLanguage", "Wolfram Language", 1988,
     desc="Symbolic language of Mathematica.",
     designers=["stephen-wolfram"], developers=["wolfram-research"],
     paradigms=["Functional", "ArrayProgramming"], ext=[".nb", ".m", ".wl"],
     url="https://www.wolfram.com/language/",
     comment="The language of Mathematica, named Wolfram Language in 2013.")
# ----- 1990s -----
lang("Haskell", "Haskell", 1990,
     desc="Purely functional language with lazy evaluation and type classes.",
     designers=["simon-peyton-jones", "philip-wadler", "paul-hudak", "john-hughes"],
     influenced_by=["Miranda", "ML", "StandardML", "Lisp", "Scheme", "ISWIM"],
     paradigms=["PurelyFunctional"], typing="static", strength="strong", inference=True,
     ext=[".hs", ".lhs"], url="https://www.haskell.org/", i18n="functional",
     comment="Named after the logician Haskell Curry.")
lang("J", "J", 1990,
     desc="Array language written in plain ASCII, by Kenneth Iverson and Roger Hui.",
     designers=["kenneth-iverson", "roger-hui"], influenced_by=["APL"],
     paradigms=["ArrayProgramming", "Functional"], ext=[".ijs"],
     url="https://www.jsoftware.com/")
lang("Python", "Python", 1991,
     desc="General-purpose language that emphasises readability.",
     designers=["guido-van-rossum"], developers=["psf"],
     influenced_by=["ABC", "ALGOL68", "C", "CPlusPlus", "CLU", "Haskell", "Icon", "Lisp",
                    "Modula3", "Perl", "StandardML"],
     paradigms=["ClassBased", "Procedural", "Functional"], typing="dynamic", strength="strong",
     ext=[".py", ".pyi", ".pyw"], url="https://www.python.org/", licenses=["PSF-2.0"],
     first_release="1991-02-20", comment="Named after the comedy group Monty Python.")
lang("Dylan", "Dylan", 1992,
     desc="Multi-paradigm language created at Apple, combining Lisp ideas with an ALGOL-like syntax.",
     developers=["apple"], influenced_by=["Scheme", "CommonLisp"],
     paradigms=["Functional", "ObjectOriented"], typing="dynamic", strength="strong",
     ext=[".dylan"])
lang("Lua", "Lua", 1993,
     desc="Lightweight, embeddable scripting language from PUC-Rio in Brazil.",
     designers=["roberto-ierusalimschy", "waldemar-celes", "luiz-henrique-de-figueiredo"],
     developers=["puc-rio"], influenced_by=["CPlusPlus", "CLU", "Scheme", "SNOBOL"],
     paradigms=["Procedural", "PrototypeBased", "Functional"], typing="dynamic",
     strength="strong", ext=[".lua"], url="https://www.lua.org/", licenses=["MIT"],
     comment="'Lua' means 'moon' in Portuguese.")
lang("R", "R", 1993,
     desc="Language and environment for statistical computing and graphics.",
     designers=["ross-ihaka", "robert-gentleman"], influenced_by=["S", "Scheme", "CommonLisp"],
     paradigms=["Procedural", "Functional", "ObjectOriented", "ArrayProgramming"],
     typing="dynamic", ext=[".r", ".rds"], url="https://www.r-project.org/",
     comment="Named partly after the first names of its first two authors, Ross Ihaka and Robert Gentleman.")
lang("K", "K", 1993,
     desc="Terse array language by Arthur Whitney.", designers=["arthur-whitney"],
     developers=["kx-systems"], influenced_by=["APL"],
     paradigms=["ArrayProgramming", "Functional"], ext=[".k"])
lang("Ruby", "Ruby", 1995,
     desc="Dynamic object-oriented language designed for programmer happiness.",
     designers=["yukihiro-matsumoto"],
     influenced_by=["Ada", "CPlusPlus", "CLU", "Dylan", "Eiffel", "Lisp", "Lua", "Perl",
                    "Python", "Smalltalk"],
     paradigms=["ClassBased", "Functional"], typing="dynamic", strength="strong",
     ext=[".rb"], url="https://www.ruby-lang.org/", licenses=["Ruby", "BSD-2-Clause"],
     v1="1996-12-25", i18n="ruby")
lang("Java", "Java", 1995,
     desc="Class-based object-oriented language compiled to bytecode for the Java virtual machine.",
     designers=["james-gosling"], developers=["sun-microsystems", "oracle"],
     influenced_by=["CPlusPlus", "Smalltalk", "ObjectiveC", "Simula", "Mesa", "Modula3",
                    "Oberon", "Ada", "Eiffel", "CLU", "Lisp", "ObjectPascal", "CSharp"],
     paradigms=["ClassBased", "Concurrent"], typing="static", strength="strong",
     ext=[".java", ".class", ".jar"], first_release="1995-05-23", v1="1996-01-23",
     alt=["Oak"], i18n="oo", comment="Originally called Oak.")
lang("JavaScript", "JavaScript", 1995,
     desc="Scripting language of the web, created at Netscape.",
     designers=["brendan-eich"], developers=["netscape"],
     influenced_by=["Java", "Scheme", "Self", "AWK"],
     paradigms=["PrototypeBased", "Functional", "Procedural"], typing="dynamic",
     strength="weak", ext=[".js", ".mjs", ".cjs"], first_release="1995-12-04",
     alt=["ECMAScript", "Mocha", "LiveScript"],
     comment="Developed under the names Mocha and LiveScript; standardised as ECMAScript.")
lang("PHP", "PHP", 1995,
     desc="Server-side scripting language for the web.",
     designers=["rasmus-lerdorf"], influenced_by=["Perl", "C", "CPlusPlus", "Java", "Tcl"],
     paradigms=["Procedural", "ClassBased", "Functional"], typing="dynamic", strength="weak",
     ext=[".php"], url="https://www.php.net/", licenses=["PHP-3.01"],
     first_release="1995-06-08", alt=["PHP: Hypertext Preprocessor", "Personal Home Page"],
     comment="Originally 'Personal Home Page'; now the recursive acronym 'PHP: Hypertext Preprocessor'.")
lang("Racket", "Racket", 1995,
     desc="Descendant of Scheme designed for creating new languages.",
     developers=["plt"], influenced_by=["Scheme", "Eiffel"], paradigms=["Functional"],
     typing="dynamic", strength="strong", ext=[".rkt"], url="https://racket-lang.org/",
     licenses=["MIT", "Apache-2.0"], alt=["PLT Scheme"],
     comment="Renamed from PLT Scheme in 2010.")
lang("Limbo", "Limbo", 1995,
     desc="Concurrent language for the Inferno operating system, from Bell Labs.",
     designers=["sean-dorward", "phil-winterbottom", "rob-pike"], developers=["bell-labs"],
     influenced_by=["C"], paradigms=["Procedural", "Concurrent"])
lang("Mercury", "Mercury", 1995,
     desc="Purely declarative logic and functional language from the University of Melbourne.",
     designers=["zoltan-somogyi"], developers=["university-of-melbourne"],
     influenced_by=["Prolog", "Haskell"], paradigms=["Logic", "Functional"],
     typing="static", strength="strong", ext=[".m"], i18n="logic")
lang("OCaml", "OCaml", 1996,
     desc="ML dialect with objects, modules and a native-code compiler.",
     designers=["xavier-leroy"], developers=["inria"],
     influenced_by=["Caml", "StandardML", "Modula3"], paradigms=["Functional", "ObjectOriented"],
     typing="static", strength="strong", inference=True, ext=[".ml", ".mli"],
     url="https://ocaml.org/", alt=["Objective Caml"], i18n="functional",
     comment="Originally named Objective Caml.")
# ----- 2000s -----
lang("CSharp", "C#", 2000,
     desc="Multi-paradigm language for .NET, designed at Microsoft.",
     designers=["anders-hejlsberg"], developers=["microsoft"],
     influenced_by=["CPlusPlus", "Java", "Eiffel", "Haskell", "ML", "Modula3", "ObjectPascal"],
     paradigms=["ClassBased", "Functional"], typing="static", strength="strong",
     ext=[".cs", ".csx"], alt=["C Sharp"], comment="Developed under the code name Cool.")
lang("D", "D", 2001,
     desc="Systems programming language conceived as a successor to C++.",
     designers=["walter-bright", "andrei-alexandrescu"],
     influenced_by=["C", "CPlusPlus", "CSharp", "Java", "Eiffel", "Python"],
     paradigms=["Procedural", "ClassBased", "Functional"], typing="static", strength="strong",
     ext=[".d"], url="https://dlang.org/", licenses=["BSL-1.0"])
lang("Cyclone", "Cyclone", 2002,
     desc="Memory-safe dialect of C.", developers=["att-labs", "cornell-university"],
     influenced_by=["C"], paradigms=["Procedural"], typing="static")
lang("Groovy", "Groovy", 2003,
     desc="Dynamic language for the Java platform.",
     designers=["james-strachan"], developers=["apache"],
     influenced_by=["Java", "Python", "Ruby", "Smalltalk"],
     paradigms=["ClassBased", "Functional"], ext=[".groovy"], url="https://groovy-lang.org/",
     licenses=["Apache-2.0"])
lang("Scala", "Scala", 2004,
     desc="Combines object-oriented and functional programming on the JVM.",
     designers=["martin-odersky"], developers=["epfl"],
     influenced_by=["Java", "Haskell", "Erlang", "OCaml", "Smalltalk", "Scheme", "StandardML",
                    "Eiffel", "CommonLisp"],
     paradigms=["Functional", "ClassBased"], typing="static", strength="strong",
     inference=True, ext=[".scala", ".sc"], url="https://www.scala-lang.org/",
     licenses=["Apache-2.0"],
     comment="The name is a portmanteau of 'scalable' and 'language'.")
lang("FSharp", "F#", 2005,
     desc="Functional-first language for .NET in the ML family.",
     designers=["don-syme"], developers=["microsoft"],
     influenced_by=["OCaml", "CSharp", "Haskell", "Erlang", "Python", "ML"],
     paradigms=["Functional", "ObjectOriented"], typing="static", strength="strong",
     inference=True, ext=[".fs", ".fsi", ".fsx"], url="https://fsharp.org/",
     licenses=["MIT"], alt=["F Sharp"], i18n="functional")
lang("PowerShell", "PowerShell", 2006,
     desc="Task automation shell and scripting language from Microsoft.",
     designers=["jeffrey-snover"], developers=["microsoft"],
     influenced_by=["CSharp", "Perl", "Python"], paradigms=["Procedural"],
     ext=[".ps1", ".psm1", ".psd1"])
lang("Clojure", "Clojure", 2007,
     desc="Lisp dialect for the JVM built around immutable data structures.",
     designers=["rich-hickey"], influenced_by=["CommonLisp", "Scheme", "Haskell", "Erlang", "Java"],
     paradigms=["Functional", "Concurrent"], typing="dynamic", strength="strong",
     ext=[".clj", ".cljs", ".cljc", ".edn"], url="https://clojure.org/", licenses=["EPL-1.0"])
lang("XQuery", "XQuery", 2007,
     desc="Query and functional programming language for XML, standardised by the W3C.",
     designers=["donald-chamberlin", "jonathan-robie", "daniela-florescu"], developers=["w3c"],
     influenced_by=["SQL"], paradigms=["Declarative", "Functional"], ext=[".xq", ".xquery"],
     url="https://www.w3.org/TR/xquery-31/", i18n="query", cls="QueryLanguage")
lang("SPARQL", "SPARQL", 2008,
     desc="Query language for RDF graphs, standardised by the W3C.",
     developers=["w3c"], paradigms=["Declarative"], ext=[".rq"],
     url="https://www.w3.org/TR/sparql11-query/", first_release="2008-01-15",
     alt=["SPARQL Protocol and RDF Query Language"], i18n="query", cls="QueryLanguage",
     comment="SPARQL 1.0 became a W3C Recommendation on 15 January 2008; SPARQL 1.1 followed in 2013.")
lang("Nim", "Nim", 2008,
     desc="Statically typed compiled language with a Python-like syntax.",
     designers=["andreas-rumpf"],
     influenced_by=["Ada", "Modula3", "Lisp", "CPlusPlus", "ObjectPascal", "Python", "Oberon"],
     paradigms=["Procedural", "ObjectOriented", "Functional"], typing="static",
     strength="strong", ext=[".nim", ".nims"], url="https://nim-lang.org/", licenses=["MIT"],
     alt=["Nimrod"], comment="Originally named Nimrod.")
lang("Go", "Go", 2009,
     desc="Compiled language with built-in concurrency, designed at Google.",
     designers=["robert-griesemer", "rob-pike", "ken-thompson"], developers=["google"],
     influenced_by=["C", "Pascal", "Oberon", "Limbo"],
     paradigms=["Procedural", "Concurrent"], typing="static", strength="strong",
     ext=[".go"], url="https://go.dev/", licenses=["BSD-3-Clause"],
     first_release="2009-11-10", v1="2012-03-28", alt=["Golang"],
     comment="Often called Golang, after its former website golang.org.")
lang("CoffeeScript", "CoffeeScript", 2009,
     desc="Language with a terse syntax that compiles to JavaScript.",
     designers=["jeremy-ashkenas"],
     influenced_by=["JavaScript", "Python", "Ruby", "Haskell", "Perl"],
     paradigms=["PrototypeBased", "Functional"], typing="dynamic",
     ext=[".coffee"], url="https://coffeescript.org/", licenses=["MIT"])
# ----- 2010s and later -----
lang("Rust", "Rust", 2010,
     desc="Memory-safe systems programming language without a garbage collector.",
     designers=["graydon-hoare"], developers=["mozilla"],
     influenced_by=["CPlusPlus", "OCaml", "Haskell", "Erlang", "StandardML", "Cyclone",
                    "Scheme", "Ruby", "Limbo", "Swift", "CLU", "CSharp", "Elm"],
     paradigms=["Functional", "Procedural", "Concurrent"], typing="static",
     strength="strong", inference=True, ext=[".rs"], url="https://www.rust-lang.org/",
     licenses=["MIT", "Apache-2.0"], v1="2015-05-15", i18n="systems",
     comment="Announced by Mozilla in 2010; Rust 1.0 was released on 15 May 2015.")
lang("Kotlin", "Kotlin", 2011,
     desc="Concise JVM language from JetBrains, preferred by Google for Android development.",
     designers=["andrey-breslav"], developers=["jetbrains"],
     influenced_by=["Java", "Scala", "Groovy", "CSharp"],
     paradigms=["ClassBased", "Functional"], typing="static", strength="strong",
     inference=True, ext=[".kt", ".kts"], url="https://kotlinlang.org/",
     licenses=["Apache-2.0"], v1="2016-02-15",
     comment="Named after Kotlin Island near Saint Petersburg.")
lang("Dart", "Dart", 2011,
     desc="Client-optimised language from Google, used by the Flutter toolkit.",
     designers=["lars-bak", "kasper-lund"], developers=["google"],
     influenced_by=["Java", "JavaScript", "CSharp", "Smalltalk", "Erlang"],
     paradigms=["ClassBased", "Functional"], typing="static", strength="strong",
     ext=[".dart"], url="https://dart.dev/", licenses=["BSD-3-Clause"], v1="2013-11-14")
lang("Julia", "Julia", 2012,
     desc="High-performance dynamic language for technical computing.",
     designers=["jeff-bezanson", "stefan-karpinski", "viral-shah", "alan-edelman"],
     influenced_by=["Lisp", "Fortran", "MATLAB", "Python", "R", "Ruby", "Perl", "Lua", "C",
                    "Dylan", "Scheme", "WolframLanguage"],
     paradigms=["ArrayProgramming", "Functional", "Procedural"], typing="dynamic",
     ext=[".jl"], url="https://julialang.org/", licenses=["MIT"], v1="2018-08-08")
lang("Elixir", "Elixir", 2012,
     desc="Functional language that runs on the Erlang virtual machine (BEAM).",
     designers=["jose-valim"], influenced_by=["Erlang", "Ruby", "Clojure"],
     paradigms=["Functional", "Concurrent"], typing="dynamic", strength="strong",
     ext=[".ex", ".exs"], url="https://elixir-lang.org/", licenses=["Apache-2.0"])
lang("Elm", "Elm", 2012,
     desc="Purely functional language for web front ends that compiles to JavaScript.",
     designers=["evan-czaplicki"], influenced_by=["Haskell", "StandardML", "OCaml", "FSharp"],
     paradigms=["PurelyFunctional"], typing="static", strength="strong", inference=True,
     ext=[".elm"], url="https://elm-lang.org/", licenses=["BSD-3-Clause"], i18n="functional")
lang("TypeScript", "TypeScript", 2012,
     desc="Typed superset of JavaScript from Microsoft.",
     designers=["anders-hejlsberg", "luke-hoban"], developers=["microsoft"],
     influenced_by=["JavaScript", "CSharp", "Java"],
     paradigms=["ObjectOriented", "Functional"], typing="gradual",
     ext=[".ts", ".tsx", ".mts", ".cts"], url="https://www.typescriptlang.org/",
     licenses=["Apache-2.0"], first_release="2012-10-01", v1="2014-04-02")
lang("Swift", "Swift", 2014,
     desc="Apple's general-purpose language for its platforms and beyond.",
     designers=["chris-lattner"], developers=["apple"],
     influenced_by=["ObjectiveC", "Rust", "Haskell", "Ruby", "Python", "CSharp", "CLU", "D"],
     paradigms=["ClassBased", "Functional"], typing="static", strength="strong",
     inference=True, ext=[".swift"], url="https://www.swift.org/", licenses=["Apache-2.0"],
     first_release="2014-06-02", v1="2014-09-09")
lang("Hack", "Hack", 2014,
     desc="Gradually typed dialect of PHP created at Facebook for the HHVM runtime.",
     developers=["meta"], influenced_by=["PHP"], paradigms=["ClassBased", "Procedural"],
     typing="gradual", ext=[".hack", ".hh"], url="https://hacklang.org/")
lang("Crystal", "Crystal", 2014,
     desc="Compiled language with Ruby-inspired syntax and static type inference.",
     designers=["ary-borenszweig", "juan-wajnerman", "brian-cardiff"], developers=["manas"],
     influenced_by=["Ruby", "Go"], paradigms=["ClassBased", "Concurrent"], typing="static",
     inference=True, ext=[".cr"], url="https://crystal-lang.org/", licenses=["Apache-2.0"])
lang("Raku", "Raku", 2015,
     desc="Sister language of Perl, formerly known as Perl 6.",
     designers=["larry-wall"], influenced_by=["Perl", "Haskell", "Smalltalk"],
     paradigms=["Procedural", "ClassBased", "Functional"], typing="gradual",
     ext=[".raku", ".rakumod"], url="https://raku.org/", alt=["Perl 6"],
     comment="Renamed from Perl 6 in 2019.")
lang("Zig", "Zig", 2016,
     desc="Systems programming language with no hidden control flow, aiming to improve on C.",
     designers=["andrew-kelley"], developers=["zig-software-foundation"],
     influenced_by=["C", "CPlusPlus", "Go", "Rust"], paradigms=["Procedural"],
     typing="static", strength="strong", ext=[".zig", ".zon"], url="https://ziglang.org/",
     licenses=["MIT"], i18n="systems")
lang("Gleam", "Gleam", 2016,
     desc="Statically typed functional language for the Erlang virtual machine.",
     designers=["louis-pilfold"], influenced_by=["Erlang", "Elixir", "Elm", "Rust"],
     paradigms=["Functional", "Concurrent"], typing="static", strength="strong",
     inference=True, ext=[".gleam"], url="https://gleam.run/", licenses=["Apache-2.0"],
     v1="2024-03-04")
lang("Carbon", "Carbon", 2022,
     desc="Experimental successor language to C++, started at Google.",
     developers=["google"], influenced_by=["CPlusPlus", "Rust", "Swift"],
     paradigms=["Procedural", "ObjectOriented"], typing="static", ext=[".carbon"],
     url="https://github.com/carbon-language/carbon-lang")
lang("Mojo", "Mojo", 2023,
     desc="Python-like language for AI and high-performance computing, from Modular.",
     designers=["chris-lattner"], developers=["modular"], influenced_by=["Python", "Swift", "Rust"],
     paradigms=["Imperative"], ext=[".mojo", ".🔥"],
     comment="Source files may use the extension .🔥 (the fire emoji).")

# ---------------------------------------------------------------------------
# FICTIONAL survey numbers.  (share of respondents, "loved" %, median salary USD)
# None = not enough answers to publish a salary.
# ---------------------------------------------------------------------------

SURVEYS = [
    ("survey-2025", 2025, 48213, "2025-07-15", {
        "JavaScript": (0.618, 58.4, 82000), "Python": (0.534, 66.9, 94500),
        "TypeScript": (0.402, 71.2, 98000), "Java": (0.296, 49.8, 91000),
        "CSharp": (0.271, 60.3, 88500), "CPlusPlus": (0.224, 52.6, 97000),
        "C": (0.193, 47.1, 90000), "PHP": (0.181, 40.7, 67000),
        "Go": (0.142, 69.5, 112000), "Rust": (0.127, 82.7, 118000),
        "Kotlin": (0.096, 63.0, 101500), "Dart": (0.061, 57.4, 79000),
        "Lua": (0.059, 50.6, 83500), "Ruby": (0.058, 52.9, 104000),
        "Swift": (0.052, 61.8, 106000), "R": (0.044, 45.2, 86000),
        "Scala": (0.027, 54.1, 117000), "Perl": (0.024, 31.5, 89000),
        "Elixir": (0.023, 74.9, 109000), "Haskell": (0.019, 64.4, 102000),
        "Zig": (0.013, 76.1, 108000), "Julia": (0.012, 62.2, 99000),
        "Clojure": (0.011, 70.3, 121000), "Erlang": (0.010, 59.6, 113000),
        "FSharp": (0.009, 66.0, 110500), "Fortran": (0.009, 33.0, 87500),
        "OCaml": (0.008, 68.8, 115000), "COBOL": (0.006, 22.4, 92000),
    }),
    ("survey-2026", 2026, 51877, "2026-07-14", {
        "JavaScript": (0.609, 56.9, 84500), "Python": (0.561, 67.4, 97000),
        "TypeScript": (0.437, 72.8, 101000), "Java": (0.288, 50.5, 93000),
        "CSharp": (0.266, 61.1, 90500), "CPlusPlus": (0.218, 51.9, 99500),
        "C": (0.187, 46.0, 91500), "PHP": (0.169, 39.2, 68500),
        "Go": (0.151, 70.2, 114000), "Rust": (0.145, 81.3, 121500),
        "Kotlin": (0.094, 62.7, 103000), "Lua": (0.062, 52.3, 85000),
        "Dart": (0.057, 56.0, 80500), "Ruby": (0.053, 51.6, 105500),
        "Swift": (0.049, 60.5, 108500), "R": (0.041, 44.7, 87500),
        "Elixir": (0.025, 75.6, 111000), "Scala": (0.024, 51.8, 118500),
        "Perl": (0.021, 29.8, 90000), "Zig": (0.019, 79.4, 111500),
        "Haskell": (0.018, 63.5, 104500), "Julia": (0.013, 61.9, 101000),
        "Clojure": (0.010, 69.4, 123000), "FSharp": (0.010, 65.2, 112000),
        "Erlang": (0.009, 58.7, 114500), "OCaml": (0.009, 69.9, 117500),
        "Fortran": (0.008, 31.9, 89000), "Gleam": (0.006, 84.1, None),
        "Mojo": (0.004, 71.5, None),
    }),
]

# FICTIONAL editorial reviews: (language, rating 1-5, reviewer, date, {lang: text})
REVIEWS = [
    ("Python", 5, "Mei Tanaka", "2026-01-12",
     {"en": "A gentle first language with a huge standard library.",
      "fr": "Un premier langage accessible, avec une immense bibliothèque standard."}),
    ("Python", 4, "Jonas Berg", "2026-03-02",
     {"en": "Still our default choice, although packaging remains confusing."}),
    ("Rust", 5, "Priya Natarajan", "2026-02-18",
     {"en": "A steep learning curve, but the compiler is a patient teacher."}),
    ("Rust", 4, "Tomás Rivera", "2026-05-27",
     {"en": "Excellent tooling; compile times can test your patience."}),
    ("Go", 4, "Jonas Berg", "2026-01-30",
     {"en": "Small language, fast builds, boring in the best way."}),
    ("JavaScript", 3, "Tomás Rivera", "2026-02-05",
     {"en": "Runs everywhere; mind the implicit type conversions."}),
    ("Haskell", 4, "Priya Natarajan", "2026-03-21",
     {"en": "Changes how you think about programs, even if you never ship it."}),
    ("COBOL", 2, "Mei Tanaka", "2026-04-01",
     {"en": "Not glamorous, but it still runs a lot of payroll."}),
    ("Prolog", 4, "Priya Natarajan", "2026-04-14",
     {"en": "Describe the problem and let the search do the work."}),
    ("Zig", 4, "Tomás Rivera", "2026-06-09",
     {"en": "A promising alternative to C; still before version 1.0."}),
    ("SQL", 5, "Mei Tanaka", "2026-06-23",
     {"en": "Fifty years on, still the language every developer ends up using.",
      "de": "Auch nach fünfzig Jahren die Sprache, die jeder Entwickler früher oder später benutzt."}),
    ("PHP", 3, "Jonas Berg", "2026-07-08",
     {"en": "Much better than its reputation suggests."}),
    ("Kotlin", 4, "Mei Tanaka", "2026-08-17",
     {"en": "Java without the ceremony."}),
    ("Julia", 4, "Priya Natarajan", "2026-09-03",
     {"en": "Fast numerics without leaving a dynamic language."}),
]
EDITORS_PICKS = ["Python", "Rust", "SQL"]

# ---------------------------------------------------------------------------
# FICTIONAL job postings and SYNTHETIC people: shared reference data
# ---------------------------------------------------------------------------

# slug, name, ISO country, mid-level salary in USD for job postings (fictional),
# relative weight for where synthetic people live (rough, not census data).
CITIES = [
    ("san-francisco", "San Francisco", "US", 160000, 4.5),
    ("new-york", "New York", "US", 150000, 19.0),
    ("seattle", "Seattle", "US", 148000, 4.0),
    ("austin", "Austin", "US", 128000, 2.4),
    ("toronto", "Toronto", "CA", 98000, 6.0),
    ("london", "London", "GB", 92000, 10.0),
    ("dublin", "Dublin", "IE", 86000, 2.0),
    ("amsterdam", "Amsterdam", "NL", 82000, 2.5),
    ("berlin", "Berlin", "DE", 80000, 4.5),
    ("munich", "Munich", "DE", 84000, 2.9),
    ("zurich", "Zurich", "CH", 125000, 1.4),
    ("paris", "Paris", "FR", 70000, 11.0),
    ("stockholm", "Stockholm", "SE", 70000, 2.4),
    ("madrid", "Madrid", "ES", 50000, 6.5),
    ("warsaw", "Warsaw", "PL", 48000, 3.0),
    ("tokyo", "Tokyo", "JP", 68000, 37.0),
    ("singapore", "Singapore", "SG", 88000, 6.0),
    ("sydney", "Sydney", "AU", 92000, 5.0),
    ("bengaluru", "Bengaluru", "IN", 28000, 13.0),
    ("sao-paulo", "São Paulo", "BR", 34000, 22.0),
]

# language label -> (IRI local name or None, posting weight, salary factor, roles)
JOB_LANGUAGES = [
    ("Python", "Python", 16, 1.02, ["Backend Engineer", "Data Engineer", "Machine Learning Engineer"]),
    ("JavaScript", "JavaScript", 14, 0.95, ["Frontend Engineer", "Full-Stack Developer"]),
    ("TypeScript", "TypeScript", 12, 1.00, ["Frontend Engineer", "Full-Stack Engineer"]),
    ("Java", "Java", 12, 1.00, ["Backend Engineer", "Software Engineer"]),
    ("C#", "CSharp", 9, 0.97, ["Software Engineer", "Game Developer"]),
    ("C++", "CPlusPlus", 7, 1.05, ["Software Engineer", "Game Engine Programmer", "Quant Developer"]),
    ("Go", "Go", 7, 1.10, ["Backend Engineer", "Platform Engineer"]),
    ("PHP", "PHP", 6, 0.82, ["Web Developer", "Backend Developer"]),
    ("C", "C", 5, 0.98, ["Embedded Software Engineer", "Firmware Engineer"]),
    ("Rust", "Rust", 5, 1.12, ["Systems Engineer", "Backend Engineer"]),
    ("SQL", "SQL", 4, 0.90, ["Data Analyst", "Database Engineer"]),
    ("Kotlin", "Kotlin", 4, 1.03, ["Android Engineer", "Backend Engineer"]),
    ("Swift", "Swift", 3, 1.04, ["iOS Engineer"]),
    ("Ruby", "Ruby", 3, 1.00, ["Backend Engineer", "Full-Stack Developer"]),
    ("Dart", "Dart", 2, 0.93, ["Mobile Developer"]),
    ("Scala", "Scala", 2, 1.12, ["Data Engineer", "Backend Engineer"]),
    ("R", "R", 2, 0.95, ["Data Scientist", "Biostatistician"]),
    ("Lua", "Lua", 1, 0.90, ["Gameplay Programmer"]),
    ("Perl", "Perl", 1, 0.95, ["Systems Engineer"]),
    ("COBOL", "COBOL", 1, 1.00, ["Mainframe Developer"]),
    ("Julia", "Julia", 1, 1.00, ["Research Software Engineer"]),
    ("Elixir", "Elixir", 1, 1.06, ["Backend Engineer"]),
    ("Haskell", "Haskell", 1, 1.08, ["Software Engineer"]),
    ("ABAP", None, 1, 1.00, ["SAP Developer"]),  # deliberately NOT in languages.ttl
    ("Fortran", "Fortran", 0.5, 0.97, ["Scientific Software Engineer"]),
    ("Erlang", "Erlang", 0.5, 1.05, ["Telecom Software Engineer"]),
    ("Clojure", "Clojure", 0.5, 1.12, ["Software Engineer"]),
    ("OCaml", "OCaml", 0.5, 1.12, ["Software Engineer"]),
    ("F#", "FSharp", 0.5, 1.04, ["Software Engineer"]),
    ("Zig", "Zig", 0.5, 1.05, ["Systems Engineer"]),
]

LEVELS = [("Junior ", 0.70, 2), ("", 1.00, 4), ("Senior ", 1.30, 3), ("Staff ", 1.60, 1)]

COMPANIES = [
    "Bluefin Analytics", "Copperleaf Systems", "Driftwood Games", "Emberline Health",
    "Foxglove Robotics", "Granite Peak Finance", "Harborview Logistics", "Ironbark Security",
    "Juniper Lane Software", "Kestrel Mobility", "Lumen Bay Labs", "Meadowlark Media",
    "Nettlefield Energy", "Oakhollow Insurance", "Pinecrest Payments", "Quarry Lane Data",
    "Redwood Arc Studios", "Saltmarsh Biotech", "Tidewater Telecom", "Umberglow Retail",
    "Vantage Hill Capital", "Willowbrook Education", "Yarrow Aerospace", "Zephyr Freight",
]

# Roles that only make sense in some industries go to matching companies only.
ROLE_COMPANIES = {
    "Game Developer": ["Driftwood Games", "Redwood Arc Studios"],
    "Game Engine Programmer": ["Driftwood Games", "Redwood Arc Studios"],
    "Gameplay Programmer": ["Driftwood Games", "Redwood Arc Studios"],
    "Quant Developer": ["Granite Peak Finance", "Vantage Hill Capital", "Pinecrest Payments"],
    "Biostatistician": ["Saltmarsh Biotech", "Emberline Health"],
    "Telecom Software Engineer": ["Tidewater Telecom"],
    "SAP Developer": ["Oakhollow Insurance", "Harborview Logistics", "Umberglow Retail",
                      "Zephyr Freight"],
    "Mainframe Developer": ["Oakhollow Insurance", "Granite Peak Finance", "Pinecrest Payments"],
    "Embedded Software Engineer": ["Foxglove Robotics", "Kestrel Mobility", "Yarrow Aerospace",
                                   "Nettlefield Energy"],
    "Firmware Engineer": ["Foxglove Robotics", "Kestrel Mobility", "Yarrow Aerospace"],
    "Scientific Software Engineer": ["Yarrow Aerospace", "Nettlefield Energy", "Lumen Bay Labs"],
    "Research Software Engineer": ["Saltmarsh Biotech", "Lumen Bay Labs", "Nettlefield Energy"],
}

JOB_COUNT = 300
JOB_SEED = 20260101
JOB_FIRST_DAY = dt.date(2025, 10, 1)
JOB_DAYS = 365

FIRST_NAMES = [
    "Aiko", "Amara", "Andrei", "Ana", "Arjun", "Astrid", "Ben", "Carlos", "Chen", "Chloe",
    "Dmitri", "Elena", "Emeka", "Fatima", "Felix", "Hana", "Hiroshi", "Ines", "Isabel", "Jakob",
    "James", "Javier", "Ji-woo", "Kai", "Kofi", "Lars", "Leila", "Liam", "Lucia", "Mai",
    "Marco", "Maria", "Mateo", "Mei", "Mohammed", "Nadia", "Nikhil", "Nora", "Olga", "Omar",
    "Priya", "Rafael", "Ravi", "Rosa", "Sakura", "Samuel", "Sara", "Sofia", "Tariq", "Thandiwe",
    "Tomás", "Wei", "Yara", "Yusuf", "Zainab", "Zoe", "Ingrid", "Pablo", "Aisha", "Lukas",
]
LAST_NAMES = [
    "Ahmed", "Andersson", "Bauer", "Chen", "Costa", "da Silva", "Dubois", "Eriksen",
    "Fernández", "Fischer", "García", "Gupta", "Haddad", "Hansen", "Ivanova", "Jensen", "Kim",
    "Kowalski", "Kumar", "Larsen", "Le", "Li", "Lopez", "Martin", "Müller", "Nakamura",
    "Nguyen", "Novak", "Okafor", "Olsen", "Park", "Patel", "Pereira", "Petrov", "Rahman",
    "Rossi", "Sato", "Schmidt", "Silva", "Singh", "Smith", "Suzuki", "Tanaka", "Taylor",
    "Torres", "Wang", "Weber", "Williams", "Wong", "Yamamoto", "Yilmaz", "Zhang", "Moreau",
    "Murphy", "O'Brien", "Kovač", "Nielsen", "Ito", "Mensah", "Fontaine",
]

# ---------------------------------------------------------------------------
# Deterministic PRNG (SplitMix64): identical output on every Python version.
# ---------------------------------------------------------------------------

_MASK = (1 << 64) - 1


class Rng:
    def __init__(self, seed: int) -> None:
        self.state = seed & _MASK

    def next64(self) -> int:
        self.state = (self.state + 0x9E3779B97F4A7C15) & _MASK
        z = self.state
        z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & _MASK
        z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & _MASK
        return z ^ (z >> 31)

    def random(self) -> float:
        return (self.next64() >> 11) * (1.0 / 9007199254740992.0)

    def below(self, n: int) -> int:
        return self.next64() % n

    def choice(self, seq):
        return seq[self.below(len(seq))]


class Weighted:
    def __init__(self, weights) -> None:
        self.cum = []
        total = 0.0
        for w in weights:
            total += w
            self.cum.append(total)
        self.total = total

    def pick(self, rng: Rng) -> int:
        return min(bisect.bisect_right(self.cum, rng.random() * self.total), len(self.cum) - 1)


# ---------------------------------------------------------------------------
# Serialisation helpers
# ---------------------------------------------------------------------------


def esc(text: str) -> str:
    return (text.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
            .replace("\r", "\\r"))


def lit(text: str, lang_tag: str | None = None) -> str:
    return '"%s"%s' % (esc(text), "@" + lang_tag if lang_tag else "")


def date_lit(text: str) -> str:
    return '"%s"^^xsd:date' % text


def iri(full: str) -> str:
    return "<%s>" % full


class TurtleWriter:
    """Pretty Turtle/TriG output with one subject block per resource."""

    def __init__(self) -> None:
        self.out = io.StringIO()
        self.triples = 0
        self.indent = ""

    def line(self, text: str = "") -> None:
        self.out.write((self.indent + text).rstrip() + "\n")

    def comment(self, text: str) -> None:
        for part in text.split("\n"):
            self.line("# " + part if part else "#")

    def block(self, subject: str, pairs) -> None:
        """pairs: list of (predicate, [objects]); empty object lists are skipped."""
        pairs = [(p, objs) for p, objs in pairs if objs]
        if not pairs:
            return
        self.line(subject)
        for i, (pred, objs) in enumerate(pairs):
            self.triples += len(objs)
            end = " ." if i == len(pairs) - 1 else " ;"
            head = "    %s " % pred
            text = head + " , ".join(objs)
            if len(text) + len(end) <= 100:
                self.line(text + end)
                continue
            # wrap long object lists, one continuation line per ~100 columns
            pad = " " * len(head)
            cur = head
            for j, obj in enumerate(objs):
                piece = obj + (" ," if j < len(objs) - 1 else end)
                if cur.strip() and len(cur) + len(piece) + 1 > 100 and cur != head:
                    self.line(cur.rstrip())
                    cur = pad
                cur += piece + " "
            self.line(cur.rstrip())
        self.line()

    def prefixes(self, prefixes) -> None:
        for name, ns in prefixes:
            self.line("@prefix %s: <%s> ." % (name, ns))
        self.line()

    def text(self) -> str:
        return self.out.getvalue()


def validate() -> None:
    keys = {l["key"] for l in LANGUAGES}
    if len(keys) != len(LANGUAGES):
        raise SystemExit("duplicate language key")
    paradigms = {p[0] for p in PARADIGMS}
    for l in LANGUAGES:
        for k in l["influenced_by"]:
            if k not in keys:
                raise SystemExit("%s: unknown influence %s" % (l["key"], k))
        for p in l["designers"]:
            if p not in PERSONS:
                raise SystemExit("%s: unknown person %s" % (l["key"], p))
        for o in l["developers"]:
            if o not in ORGS:
                raise SystemExit("%s: unknown organisation %s" % (l["key"], o))
        for p in l["paradigms"]:
            if p not in paradigms:
                raise SystemExit("%s: unknown paradigm %s" % (l["key"], p))
        for lic in l["licenses"]:
            if lic not in LICENSES:
                raise SystemExit("%s: unknown licence %s" % (l["key"], lic))
    used_people = {p for l in LANGUAGES for p in l["designers"]}
    used_orgs = {o for l in LANGUAGES for o in l["developers"]}
    if set(PERSONS) - used_people:
        raise SystemExit("unused persons: %s" % sorted(set(PERSONS) - used_people))
    if set(ORGS) - used_orgs:
        raise SystemExit("unused organisations: %s" % sorted(set(ORGS) - used_orgs))
    for _, _, _, _, rows in SURVEYS:
        for k in rows:
            if k not in keys:
                raise SystemExit("survey: unknown language %s" % k)
    for r in REVIEWS:
        if r[0] not in keys:
            raise SystemExit("review: unknown language %s" % r[0])
    for _, key, *_ in JOB_LANGUAGES:
        if key is not None and key not in keys:
            raise SystemExit("jobs: unknown language %s" % key)


# ---------------------------------------------------------------------------
# languages.ttl
# ---------------------------------------------------------------------------


def make_languages_ttl() -> tuple[str, int]:
    w = TurtleWriter()
    w.comment(
        "languages.ttl -- the core graph of the sparqlite tutorial.\n"
        "Generated by tutorial/make_tutorial_data.py; edit the script, not this file.\n"
        "\n"
        "REAL-WORLD DATA about %d computer languages, %d people and %d organisations.\n"
        "Facts follow the infoboxes of the English Wikipedia articles (\"First appeared\",\n"
        "\"Designed by\", \"Developer\", \"Influenced by\", ...). Where a fact was uncertain it\n"
        "was left out, so a missing value means \"not recorded here\", never \"no\".\n"
        "\n"
        "Shape of a language (all optional except rdf:type and rdfs:label):\n"
        "  ex:Python a ex:ProgrammingLanguage ;      # or ex:QueryLanguage\n"
        "      rdfs:label \"Python\"@en ;              # English name, language-tagged\n"
        "      skos:altLabel \"...\" ;                 # other / former names (plain strings)\n"
        "      schema:description \"...\"@en ;         # some also @fr @de @es @ja\n"
        "      rdfs:comment \"...\"@en ;               # a trivia note, for some languages\n"
        "      ex:firstAppeared 1991 ;               # xsd:integer year\n"
        "      ex:firstReleaseDate \"1991-02-20\"^^xsd:date ;  # only where well documented\n"
        "      ex:version1Date \"...\"^^xsd:date ;     # release date of version 1.0 (a few)\n"
        "      ex:designer person:guido-van-rossum ;\n"
        "      ex:developer org:psf ;\n"
        "      ex:influencedBy ex:ABC , ex:C , ... ; # the language on the right influenced it\n"
        "      ex:paradigm ex:Functional , ... ;     # classes in a rdfs:subClassOf hierarchy\n"
        "      ex:typing ex:DynamicTyping ; ex:typeStrength ex:StrongTyping ;\n"
        "      ex:typeInference true ;\n"
        "      ex:fileExtension \".py\" , \".pyi\" ;\n"
        "      schema:url <https://www.python.org/> ; # homepage, only for some languages\n"
        "      dcterms:license <http://spdx.org/licenses/PSF-2.0> .  # reference implementation"
        % (len(LANGUAGES), len(PERSONS), len(ORGS)))
    w.line()
    w.prefixes(PREFIXES)

    # --- vocabulary ---
    w.comment("=" * 78 + "\nVocabulary: classes and properties used below\n" + "=" * 78)
    w.line()
    w.block("ex:ComputerLanguage", [
        ("a", ["rdfs:Class"]),
        ("rdfs:label", [lit("computer language", "en")]),
        ("rdfs:comment", [lit("Any formal language for computers: programming and query languages.", "en")]),
    ])
    w.block("ex:ProgrammingLanguage", [
        ("a", ["rdfs:Class"]),
        ("rdfs:subClassOf", ["ex:ComputerLanguage"]),
        ("rdfs:label", [lit("programming language", "en"), lit("langage de programmation", "fr"),
                        lit("Programmiersprache", "de"), lit("lenguaje de programación", "es"),
                        lit("プログラミング言語", "ja")]),
    ])
    w.block("ex:QueryLanguage", [
        ("a", ["rdfs:Class"]),
        ("rdfs:subClassOf", ["ex:ComputerLanguage"]),
        ("rdfs:label", [lit("query language", "en"), lit("langage de requête", "fr"),
                        lit("Abfragesprache", "de"), lit("lenguaje de consulta", "es"),
                        lit("問い合わせ言語", "ja")]),
    ])
    w.block("ex:Person", [
        ("a", ["rdfs:Class"]), ("rdfs:subClassOf", ["schema:Person"]),
        ("rdfs:label", [lit("person", "en")]),
    ])
    w.block("ex:Organization", [
        ("a", ["rdfs:Class"]), ("rdfs:subClassOf", ["schema:Organization"]),
        ("rdfs:label", [lit("organisation", "en")]),
    ])
    w.block("ex:TypeDiscipline", [
        ("a", ["rdfs:Class"]), ("rdfs:label", [lit("type discipline", "en")]),
    ])
    props = [
        ("firstAppeared", "first appeared", "Year the language first appeared (xsd:integer).",
         "ex:ComputerLanguage", "xsd:integer"),
        ("firstReleaseDate", "first release date", "Date of the first public release or announcement.",
         "ex:ComputerLanguage", "xsd:date"),
        ("version1Date", "version 1.0 date", "Release date of version 1.0.",
         "ex:ComputerLanguage", "xsd:date"),
        ("designer", "designer", "A person who designed the language.",
         "ex:ComputerLanguage", "ex:Person"),
        ("developer", "developer", "An organisation that developed or maintains the language.",
         "ex:ComputerLanguage", "ex:Organization"),
        ("influencedBy", "influenced by",
         "The object language influenced the subject language. Traverse it backwards with ^ex:influencedBy.",
         "ex:ComputerLanguage", "ex:ComputerLanguage"),
        ("paradigm", "paradigm", "A programming paradigm the language supports (a class below ex:Paradigm).",
         "ex:ComputerLanguage", "rdfs:Class"),
        ("typing", "typing", "When types are checked: static, dynamic, gradual, or untyped.",
         "ex:ComputerLanguage", "ex:TypeDiscipline"),
        ("typeStrength", "type strength", "Strong or weak typing, where uncontroversial.",
         "ex:ComputerLanguage", "ex:TypeDiscipline"),
        ("typeInference", "type inference", "true if the compiler infers most types.",
         "ex:ComputerLanguage", "xsd:boolean"),
        ("fileExtension", "file extension", "A conventional source file name extension, with the dot.",
         "ex:ComputerLanguage", "xsd:string"),
        ("turingAwardYear", "Turing Award year", "Year the person received the ACM Turing Award.",
         "ex:Person", "xsd:integer"),
    ]
    for local, label, comment, domain, rng in props:
        w.block("ex:" + local, [
            ("a", ["rdf:Property"]), ("rdfs:label", [lit(label, "en")]),
            ("rdfs:comment", [lit(comment, "en")]), ("rdfs:domain", [domain]),
            ("rdfs:range", [rng]),
        ])

    # --- paradigms ---
    w.comment("=" * 78 + "\nParadigms: a small class hierarchy (query it with rdfs:subClassOf*)\n"
              "  Paradigm\n"
              "    Imperative -> Procedural, ObjectOriented -> ClassBased, PrototypeBased\n"
              "    Declarative -> Functional -> PurelyFunctional; Logic\n"
              "    Concurrent, ArrayProgramming, Concatenative\n" + "=" * 78)
    w.line()
    for local, parent, labels, comment in PARADIGMS:
        label_objs = [lit(text, tag) for tag, text in labels.items()]
        w.block("ex:" + local, [
            ("a", ["rdfs:Class"]),
            ("rdfs:subClassOf", ["ex:" + parent] if parent else []),
            ("rdfs:label", label_objs),
            ("skos:altLabel", [lit(t, tag) for t, tag in PARADIGM_ALT.get(local, [])]),
            ("rdfs:comment", [lit(comment, "en")]),
        ])

    # --- typing ---
    w.comment("=" * 78 + "\nType disciplines\n" + "=" * 78)
    w.line()
    for table in (TYPING, STRENGTH):
        for local, label, comment in table.values():
            w.block("ex:" + local, [
                ("a", ["ex:TypeDiscipline"]), ("rdfs:label", [lit(label, "en")]),
                ("rdfs:comment", [lit(comment, "en")]),
            ])

    # --- licences ---
    w.comment("=" * 78 + "\nLicences (SPDX identifiers) of reference implementations\n" + "=" * 78)
    w.line()
    for spdx_id, name in LICENSES.items():
        w.block(iri(SPDX + spdx_id), [
            ("a", ["dcterms:LicenseDocument"]), ("rdfs:label", [lit(name, "en")]),
            ("dcterms:identifier", [lit(spdx_id)]),
        ])

    # --- organisations ---
    w.comment("=" * 78 + "\nOrganisations\n" + "=" * 78)
    w.line()
    for slug, (name, alts) in ORGS.items():
        w.block("org:" + slug, [
            ("a", ["ex:Organization"]), ("schema:name", [lit(name)]),
            ("skos:altLabel", [lit(a) for a in alts]),
        ])

    # --- people ---
    w.comment("=" * 78 + "\nPeople (designers). Dates only where well documented; no deathDate\n"
              "means only that none is recorded here.\n" + "=" * 78)
    w.line()
    for slug, (name, born, died, turing) in PERSONS.items():
        w.block("person:" + slug, [
            ("a", ["ex:Person"]), ("schema:name", [lit(name)]),
            ("schema:birthDate", [date_lit(born)] if born else []),
            ("schema:deathDate", [date_lit(died)] if died else []),
            ("ex:turingAwardYear", [str(turing)] if turing else []),
        ])

    # --- languages ---
    w.comment("=" * 78 + "\nLanguages, in order of first appearance\n" + "=" * 78)
    w.line()
    for l in LANGUAGES:
        who = ", ".join(PERSONS[p][0] for p in l["designers"])
        w.comment("%s (%s)%s" % (l["label"], l["year"] if l["year"] else "year not recorded",
                                 " -- " + who if who else ""))
        descs = [lit(l["desc"], "en")]
        if l["i18n"]:
            descs += [lit(text, tag) for tag, text in I18N[l["i18n"]].items()]
        w.block("ex:" + l["key"], [
            ("a", ["ex:" + l["cls"]]),
            ("rdfs:label", [lit(l["label"], "en")]),
            ("skos:altLabel", [lit(a) for a in l["alt"]]),
            ("schema:description", descs),
            ("rdfs:comment", [lit(l["comment"], "en")] if l["comment"] else []),
            ("ex:firstAppeared", [str(l["year"])] if l["year"] else []),
            ("ex:firstReleaseDate", [date_lit(l["first_release"])] if l["first_release"] else []),
            ("ex:version1Date", [date_lit(l["v1"])] if l["v1"] else []),
            ("ex:designer", ["person:" + p for p in l["designers"]]),
            ("ex:developer", ["org:" + o for o in l["developers"]]),
            ("ex:paradigm", ["ex:" + p for p in l["paradigms"]]),
            ("ex:typing", ["ex:" + TYPING[l["typing"]][0]] if l["typing"] else []),
            ("ex:typeStrength", ["ex:" + STRENGTH[l["strength"]][0]] if l["strength"] else []),
            ("ex:typeInference", ["true"] if l["inference"] else []),
            ("ex:fileExtension", [lit(e) for e in l["ext"]]),
            ("schema:url", [iri(l["url"])] if l["url"] else []),
            ("dcterms:license", [iri(SPDX + x) for x in l["licenses"]]),
            ("ex:influencedBy", ["ex:" + k for k in l["influenced_by"]]),
        ])
    w.comment("%d triples." % w.triples)
    return w.text(), w.triples


# ---------------------------------------------------------------------------
# survey.trig (FICTIONAL)
# ---------------------------------------------------------------------------


def make_survey_trig() -> tuple[str, int]:
    w = TurtleWriter()
    w.comment(
        "survey.trig -- FICTIONAL DATA for the sparqlite tutorial.\n"
        "Generated by tutorial/make_tutorial_data.py; edit the script, not this file.\n"
        "\n"
        "Every number and review in this file is MADE UP to demonstrate named graphs,\n"
        "GRAPH / FROM / FROM NAMED and aggregates. Nothing here is a real survey result.\n"
        "\n"
        "Layout:\n"
        "  default graph: one description of each named graph (with ex:fictional true)\n"
        "  <%ssurvey-2025> and\n"
        "  <%ssurvey-2026>: per language\n"
        "      ex:usageShare      share of respondents using it (xsd:decimal, 0..1)\n"
        "      ex:lovedPercent    %% of its users who want to keep using it (xsd:decimal)\n"
        "      ex:medianSalaryUSD median salary (xsd:integer; missing when too few answered)\n"
        "  <%seditorial>: reviews as blank nodes, plus ex:editorsPick flags"
        % (GRAPH_NS, GRAPH_NS, GRAPH_NS))
    w.line()
    w.prefixes([p for p in PREFIXES if p[0] not in ("person", "org", "skos")])

    w.comment("--- Default graph: metadata about the named graphs ---")
    w.line()
    for slug, year, respondents, issued, _ in SURVEYS:
        w.block(iri(GRAPH_NS + slug), [
            ("a", ["ex:Survey"]),
            ("rdfs:label", [lit("Developer survey %d (fictional)" % year, "en")]),
            ("rdfs:comment", [lit("FICTIONAL: invented for the sparqlite tutorial; "
                                  "not real survey results.", "en")]),
            ("ex:fictional", ["true"]),
            ("ex:surveyYear", [str(year)]),
            ("ex:respondents", [str(respondents)]),
            ("dcterms:issued", [date_lit(issued)]),
        ])
    w.block(iri(GRAPH_NS + "editorial"), [
        ("a", ["ex:EditorialGraph"]),
        ("rdfs:label", [lit("Editorial reviews (fictional)", "en")]),
        ("rdfs:comment", [lit("FICTIONAL: invented reviews by invented editors, "
                              "for the sparqlite tutorial.", "en")]),
        ("ex:fictional", ["true"]),
    ])

    for slug, year, _, _, rows in SURVEYS:
        w.comment("--- FICTIONAL survey %d ---" % year)
        w.line("GRAPH %s {" % iri(GRAPH_NS + slug))
        w.indent = "    "
        for key, (share, loved, salary) in rows.items():
            w.block("ex:" + key, [
                ("ex:usageShare", ["%.3f" % share]),
                ("ex:lovedPercent", ["%.1f" % loved]),
                ("ex:medianSalaryUSD", [str(salary)] if salary is not None else []),
            ])
        w.indent = ""
        w.line("}")
        w.line()

    w.comment("--- FICTIONAL editorial reviews (blank nodes) ---")
    w.line("GRAPH %s {" % iri(GRAPH_NS + "editorial"))
    w.indent = "    "
    for key, rating, reviewer, date, texts in REVIEWS:
        # Written as an inline blank node: [ ... ] .
        objs = [("a", ["ex:Review"]), ("ex:about", ["ex:" + key]), ("ex:rating", [str(rating)]),
                ("ex:reviewer", [lit(reviewer)]), ("dcterms:date", [date_lit(date)]),
                ("schema:reviewBody", [lit(t, tag) for tag, t in texts.items()])]
        w.line("[")
        for i, (p, os_) in enumerate(objs):
            w.triples += len(os_)
            w.line("    %s %s%s" % (p, " , ".join(os_), " ;" if i < len(objs) - 1 else ""))
        w.line("] .")
        w.line()
    for key in EDITORS_PICKS:
        w.block("ex:" + key, [("ex:editorsPick", ["true"])])
    w.indent = ""
    w.line("}")
    w.line()
    w.comment("%d quads." % w.triples)
    return w.text(), w.triples


# ---------------------------------------------------------------------------
# job_postings (FICTIONAL relational table)
# ---------------------------------------------------------------------------

JOB_COLUMNS = ["id", "title", "company", "language", "language_iri", "city", "country",
               "remote", "salary_usd", "posted"]


def make_job_rows():
    rng = Rng(JOB_SEED)
    langs = Weighted([j[2] for j in JOB_LANGUAGES])
    levels = Weighted([lv[2] for lv in LEVELS])
    rows = []
    for i in range(1, JOB_COUNT + 1):
        label, key, _, factor, roles = JOB_LANGUAGES[langs.pick(rng)]
        level, level_factor, _ = LEVELS[levels.pick(rng)]
        role = rng.choice(roles)
        _, city, country, base, _ = CITIES[rng.below(len(CITIES))]
        noise = 0.9 + 0.2 * rng.random()
        salary = int(round(base * factor * level_factor * noise / 500.0)) * 500
        remote = 1 if rng.random() < 0.2 else 0
        posted = JOB_FIRST_DAY + dt.timedelta(days=rng.below(JOB_DAYS))
        company = rng.choice(ROLE_COMPANIES.get(role, COMPANIES))
        rows.append((i, "%s%s (%s)" % (level, role, label), company, label,
                     EX + (key if key else label), city, country, remote, salary,
                     posted.isoformat()))
    rows.sort(key=lambda r: (r[9], r[0]))
    return [(n,) + r[1:] for n, r in enumerate(rows, 1)]  # ids in posting-date order


def sql_quote(v) -> str:
    if v is None:
        return "NULL"
    if isinstance(v, int):
        return str(v)
    return "'" + str(v).replace("'", "''") + "'"


def make_job_files(rows) -> tuple[str, str]:
    buf = io.StringIO()
    wr = csv.writer(buf, lineterminator="\n")
    wr.writerow(JOB_COLUMNS)
    wr.writerows(rows)
    csv_text = buf.getvalue()

    out = io.StringIO()
    out.write(
        "-- job_postings.sql -- FICTIONAL job adverts for the sparqlite tutorial.\n"
        "-- Generated by tutorial/make_tutorial_data.py; edit the script, not this file.\n"
        "-- Every company, posting and salary here is MADE UP; any resemblance to a real\n"
        "-- company is coincidental. Cities are real; salaries are annual, in US dollars.\n"
        "-- language_iri matches the language IRIs in languages.ttl, except 'ABAP', which\n"
        "-- is deliberately missing from the graph (try a LEFT JOIN).\n"
        "-- Safe to run more than once: it drops and recreates the table.\n"
        "DROP TABLE IF EXISTS job_postings;\n"
        "CREATE TABLE job_postings(\n"
        "  id           INTEGER PRIMARY KEY,\n"
        "  title        TEXT NOT NULL,\n"
        "  company      TEXT NOT NULL,\n"
        "  language     TEXT NOT NULL,   -- display name, e.g. 'C#'\n"
        "  language_iri TEXT NOT NULL,   -- e.g. 'http://example.org/pl/CSharp'\n"
        "  city         TEXT NOT NULL,\n"
        "  country      TEXT NOT NULL,   -- ISO 3166-1 alpha-2\n"
        "  remote       INTEGER NOT NULL CHECK (remote IN (0, 1)),\n"
        "  salary_usd   INTEGER NOT NULL,\n"
        "  posted       TEXT NOT NULL    -- ISO 8601 date\n"
        ");\n"
        "INSERT INTO job_postings(%s) VALUES\n" % ", ".join(JOB_COLUMNS))
    out.write(",\n".join("  (" + ", ".join(sql_quote(v) for v in r) + ")" for r in rows))
    out.write(";\n"
              "CREATE INDEX job_postings_language_iri ON job_postings(language_iri);\n")
    return csv_text, out.getvalue()


# ---------------------------------------------------------------------------
# people.nt (SYNTHETIC social network)
# ---------------------------------------------------------------------------


def write_people_nt(path: str, n: int, seed: int) -> int:
    """About 8 triples per person; returns the number of triples written."""
    rng = Rng(seed)
    rdf_type = iri(RDF + "type")
    foaf_person = iri(FOAF + "Person")
    foaf_name = iri(FOAF + "name")
    foaf_knows = iri(FOAF + "knows")
    lives_in = iri(EX + "livesIn")
    likes = iri(EX + "likes")
    born = iri(EX + "birthYear")
    label = iri(RDFS + "label")
    country = iri(SCHEMA + "addressCountry")
    schema_city = iri(SCHEMA + "City")
    xsd_int = "^^" + iri(XSD + "integer")

    city_pick = Weighted([c[4] for c in CITIES])
    # Liked languages: weighted by the (fictional) 2026 usage share, plus a long tail.
    share = dict(SURVEYS[-1][4])
    lang_keys = [l["key"] for l in LANGUAGES if l["year"] and l["cls"] == "ProgrammingLanguage"]
    like_pick = Weighted([share[k][0] if k in share else 0.002 for k in lang_keys])

    city_of = [city_pick.pick(rng) for _ in range(n)]
    members = [[] for _ in CITIES]
    for i, c in enumerate(city_of):
        members[c].append(i)
    # Popularity: a random permutation, so popular people are spread over the ids.
    popular = list(range(n))
    for i in range(n - 1, 0, -1):
        j = rng.below(i + 1)
        popular[i], popular[j] = popular[j], popular[i]

    count = 0
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(
            "# people.nt -- SYNTHETIC social network for the sparqlite tutorial (%d people, seed %d).\n"
            "# Generated by tutorial/make_tutorial_data.py --people %d --seed %d.\n"
            "# Names are random combinations of common first and last names; nobody here is real.\n"
            "# Cities are real; how many people live in each is only roughly proportional to its size.\n"
            "#\n"
            "# Shape, with people: <%s> city: <%s>\n"
            "#   ex: <%s> foaf: <%s>\n"
            "#   city:london a schema:City ; rdfs:label \"London\" ; schema:addressCountry \"GB\" .\n"
            "#   people:p1 a foaf:Person ; foaf:name \"...\" ;\n"
            "#       ex:livesIn city:london ;\n"
            "#       ex:birthYear 1978 ;          # xsd:integer\n"
            "#       ex:likes ex:Python ;         # 1-2 languages from languages.ttl\n"
            "#       foaf:knows people:p42 .      # 1-5 people; about half live in the same city\n"
            % (n, seed, n, seed, PEOPLE, CITY, EX, FOAF))
        for slug, name, cc, _, _ in CITIES:
            c_iri = iri(CITY + slug)
            f.write("%s %s %s .\n%s %s %s .\n%s %s %s .\n" % (
                c_iri, rdf_type, schema_city, c_iri, label, lit(name), c_iri, country, lit(cc)))
            count += 3
        out = []
        for i in range(n):
            s = iri("%sp%d" % (PEOPLE, i + 1))
            name = "%s %s" % (rng.choice(FIRST_NAMES), rng.choice(LAST_NAMES))
            year = 1950 + int(28 * (rng.random() + rng.random()))  # 1950..2005, peak ~1978
            out.append("%s %s %s .\n" % (s, rdf_type, foaf_person))
            out.append("%s %s %s .\n" % (s, foaf_name, lit(name)))
            out.append("%s %s %s .\n" % (s, lives_in, iri(CITY + CITIES[city_of[i]][0])))
            out.append('%s %s "%d"%s .\n' % (s, born, year, xsd_int))
            count += 4
            liked = {like_pick.pick(rng)}
            if rng.random() < 0.4:
                liked.add(like_pick.pick(rng))
            for k in sorted(liked):
                out.append("%s %s %s .\n" % (s, likes, iri(EX + lang_keys[k])))
                count += 1
            degree = 1 + rng.below(5)  # 1..5, mean 3
            friends = set()
            tries = 0
            while len(friends) < degree and tries < 20 and n > 1:
                tries += 1
                if rng.random() < 0.5:
                    pool = members[city_of[i]]
                    j = pool[rng.below(len(pool))]
                else:
                    r = rng.random()
                    j = popular[min(int(n * r * r * r), n - 1)]  # skewed towards popular people
                if j != i:
                    friends.add(j)
            for j in sorted(friends):
                out.append("%s %s %s .\n" % (s, foaf_knows, iri("%sp%d" % (PEOPLE, j + 1))))
                count += 1
            if len(out) >= 50000:
                f.write("".join(out))
                out.clear()
        f.write("".join(out))
    return count


# ---------------------------------------------------------------------------
# load.sql (for the sqlite3 shell)
# ---------------------------------------------------------------------------


def make_load_sql(with_people: bool) -> str:
    files = [("languages.ttl", "turtle"), ("survey.trig", "trig")]
    if with_people:
        files.append(("people.nt", "ntriples"))
    guard_files = "\n  AND ".join("readfile('%s') IS NOT NULL" % f
                                   for f, _ in files + [("job_postings.sql", None)])
    lines = [
        "-- load.sql -- loads the sparqlite tutorial dataset with the sqlite3 shell.",
        "-- Generated by tutorial/make_tutorial_data.py --emit-sql; re-run the script to change it.",
        "--",
        "-- readfile() and .read resolve file names against the shell's CURRENT DIRECTORY,",
        "-- so run this from inside the data directory, after loading the extension:",
        "--",
        "--   cd tutorial/data",
        "--   sqlite3 ../tutorial.db",
        "--   sqlite> .load ../../ext/target/release/libsparqlite",
        "--   sqlite> .read load.sql",
        "--",
        "-- or as a single command:",
        "--",
        "--   cd tutorial/data && sqlite3 ../tutorial.db \\",
        "--       -cmd '.load ../../ext/target/release/libsparqlite' '.read load.sql'",
        "--",
        "-- (The extension path depends on where you built or unpacked it; .load accepts",
        "-- the file name with or without the .so / .dylib / .dll suffix.)",
        "--",
        "-- .bail on stops at the first error. Running the script again on the same",
        "-- database (after a failure, or just to start over) first removes the previous",
        "-- copy with SPARQL CLEAR ALL, so give the tutorial its own database file.",
        "",
        ".bail on",
        "",
        "-- Stop early, with a readable message, if the extension or the files are missing.",
        "CREATE TEMP TABLE IF NOT EXISTS tutorial_guard(ok INTEGER",
        "  CONSTRAINT \"sparqlite is not loaded - run .load /path/to/libsparqlite first\" CHECK (ok),",
        "  files INTEGER",
        "  CONSTRAINT \"data files not found - run .read load.sql from inside the tutorial/data directory\" CHECK (files));",
        "INSERT INTO temp.tutorial_guard(ok, files) VALUES (",
        "  (SELECT count(*) > 0 FROM pragma_function_list WHERE name = 'sparql_load'), 1);",
        "INSERT INTO temp.tutorial_guard(ok, files) VALUES (1,\n  %s);" % guard_files,
        "DROP TABLE temp.tutorial_guard;",
        "",
        "SELECT 'sparql_init: schema version ' || sparql_init();",
        "-- tutorial_info marks a database this script has (started to) load. If it is",
        "-- already there, an earlier run happened: start again from an empty store.",
        "CREATE TABLE IF NOT EXISTS tutorial_info(key TEXT PRIMARY KEY, value);",
        "SELECT 'removed previous copy: ' || sparql_update('CLEAR ALL') || ' quads'",
        "  WHERE EXISTS (SELECT 1 FROM tutorial_info);",
        "SELECT 'unused dictionary terms removed: ' || sparql_vacuum_terms()",
        "  WHERE EXISTS (SELECT 1 FROM tutorial_info);",
        "DELETE FROM tutorial_info;",
        "INSERT INTO tutorial_info(key, value) VALUES ('status', 'loading');",
        "",
    ]
    for f, fmt in files:
        lines.append("SELECT '%s: ' || sparql_load(readfile('%s'), '%s') || ' new quads';" % (f, f, fmt))
    lines += [
        "",
        "-- An ordinary relational table, for joining SPARQL results with SQL.",
        ".read job_postings.sql",
        "",
        "INSERT OR REPLACE INTO tutorial_info(key, value) VALUES ('status', 'complete'),",
        "  ('dataset', 'sparqlite tutorial'), ('generator_version', '%s'),"
        % GENERATOR_VERSION,
        "  ('files', '%s'), ('loaded_by', 'load.sql');" % ",".join(f for f, _ in files + [("job_postings.sql", None)]),
        "",
        "-- Statistics for SQLite's planner and for sparqlite's planner.",
        "ANALYZE;",
        "SELECT 'sparql_analyze: ' || sparql_analyze();",
        "",
        "SELECT 'quads: ' || (SELECT count(*) FROM rdf_quad)",
        "    || ', dictionary terms: ' || (SELECT count(*) FROM rdf_term WHERE id >= 1024)",
        "    || ', named graphs: ' || (SELECT count(*) FROM rdf_graph)",
        "    || ', job postings: ' || (SELECT count(*) FROM job_postings);",
        ".bail off",
        "",
    ]
    return "\n".join(lines)


# ---------------------------------------------------------------------------
# Loading through Python's sqlite3 module
# ---------------------------------------------------------------------------


def nice_path(path: str) -> str:
    """A relative path when that is short and readable, else the absolute one."""
    try:
        rel = os.path.relpath(path)
    except ValueError:  # another drive on Windows
        return os.path.abspath(path)
    return rel if not rel.startswith(os.pardir) else os.path.abspath(path)


def default_extension() -> str | None:
    env = os.environ.get("SPARQLITE_EXT")
    if env:
        return env
    rel = os.path.join(SCRIPT_DIR, os.pardir, "ext", "target", "release")
    for name in ("libsparqlite.so", "libsparqlite.dylib", "sparqlite.dll"):
        p = os.path.normpath(os.path.join(rel, name))
        if os.path.exists(p):
            return p
    return None


def is_tutorial_db(path: str) -> bool:
    try:
        con = sqlite3.connect("file:%s?mode=ro" % path, uri=True)
        try:
            return con.execute("SELECT count(*) FROM sqlite_master WHERE name = 'tutorial_info'"
                               ).fetchone()[0] == 1
        finally:
            con.close()
    except sqlite3.Error:
        return False


def build_database(db_path: str, ext_path: str, out_dir: str, files, force: bool) -> None:
    if os.path.exists(db_path):
        if os.path.exists(db_path + "-wal") or os.path.exists(db_path + "-journal"):
            raise SystemExit("error: %s looks open or unfinished (a -wal or -journal file exists); "
                             "close other connections first" % db_path)
        if not force and not is_tutorial_db(db_path):
            raise SystemExit("error: %s exists and was not created by this script; "
                             "choose another path or pass --force to replace it" % db_path)

    probe = sqlite3.connect(":memory:")
    if not hasattr(probe, "enable_load_extension"):
        raise SystemExit(
            "error: this Python's sqlite3 module cannot load extensions (it was built\n"
            "without SQLITE_ENABLE_LOAD_EXTENSION; macOS's system Python is one example).\n"
            "Use the sqlite3 shell instead:\n"
            "  python3 %s --emit-sql\n"
            "  cd %s && sqlite3 ../tutorial.db -cmd '.load /path/to/libsparqlite' '.read load.sql'"
            % (nice_path(__file__), nice_path(out_dir)))
    probe.close()

    db_dir = os.path.dirname(os.path.abspath(db_path))
    fd, tmp = tempfile.mkstemp(prefix=".tutorial-", suffix=".db", dir=db_dir)
    os.close(fd)
    os.unlink(tmp)
    con = sqlite3.connect(tmp, isolation_level=None)
    ok = False
    try:
        con.enable_load_extension(True)
        try:
            con.load_extension(ext_path)
        except sqlite3.Error as e:
            raise SystemExit("error: could not load the extension %r: %s\n"
                             "(build it with: cargo build --release --manifest-path ext/Cargo.toml)"
                             % (ext_path, e))
        con.enable_load_extension(False)
        print("loaded %s" % con.execute("SELECT sparql_version()").fetchone()[0])
        con.execute("PRAGMA page_size = 8192")
        con.execute("SELECT sparql_init()")
        con.execute("BEGIN")
        for name, fmt in files:
            with open(os.path.join(out_dir, name), "rb") as f:
                data = f.read()
            t0 = time.perf_counter()
            n = con.execute("SELECT sparql_load(?, ?)", (data, fmt)).fetchone()[0]
            print("  %-18s %8d new quads  (%.2fs)" % (name, n, time.perf_counter() - t0))
        with open(os.path.join(out_dir, "job_postings.sql"), encoding="utf-8") as f:
            job_sql = f.read()
        # executescript() would COMMIT first; run the statements inside our transaction.
        for stmt in split_sql(job_sql):
            con.execute(stmt)
        con.execute("CREATE TABLE tutorial_info(key TEXT PRIMARY KEY, value)")
        con.executemany("INSERT INTO tutorial_info(key, value) VALUES (?, ?)", [
            ("status", "complete"), ("dataset", "sparqlite tutorial"),
            ("generator_version", GENERATOR_VERSION),
            ("files", ",".join([n for n, _ in files] + ["job_postings.sql"])),
            ("loaded_by", "make_tutorial_data.py"),
        ])
        con.execute("COMMIT")
        con.execute("ANALYZE")
        con.execute("SELECT sparql_analyze()")
        summary(con)
        ok = True
    finally:
        con.close()
        if not ok and os.path.exists(tmp):
            os.unlink(tmp)
    os.replace(tmp, db_path)
    print("wrote %s" % db_path)


def split_sql(text: str):
    """Split a script of simple statements (no triggers) on ';' at line ends."""
    stmt = []
    for line in text.splitlines():
        if line.startswith("--") and not stmt:
            continue
        stmt.append(line)
        if line.rstrip().endswith(";") and sqlite3.complete_statement("\n".join(stmt)):
            yield "\n".join(stmt)
            stmt = []
    if "".join(stmt).strip():
        yield "\n".join(stmt)


def summary(con) -> None:
    q = lambda sql: con.execute(sql).fetchone()[0]  # noqa: E731
    n_lang = q("SELECT sparql_value('PREFIX ex: <%s> PREFIX rdfs: <%s> "
               "SELECT (COUNT(?l) AS ?n) WHERE { ?l a/rdfs:subClassOf* ex:ComputerLanguage }')"
               % (EX, RDFS))
    print("summary: %d quads, %d dictionary terms, %d named graphs, %d languages, "
          "%d job postings" % (
              q("SELECT count(*) FROM rdf_quad"), q("SELECT count(*) FROM rdf_term WHERE id >= 1024"),
              q("SELECT count(*) FROM rdf_graph"), n_lang, q("SELECT count(*) FROM job_postings")))


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------


def write_text(path: str, text: str) -> None:
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--out", metavar="DIR", default=DEFAULT_OUT,
                    help="directory for the data files (default: tutorial/data next to this script)")
    ap.add_argument("--people", metavar="N", type=int, default=0,
                    help="also write people.nt, a synthetic social network of N people "
                         "(about 8.4 triples each: 100000 people is about 840K triples, "
                         "88 MB); default 0 = off")
    ap.add_argument("--seed", type=int, default=42,
                    help="random seed for --people (default 42); everything else is fixed")
    ap.add_argument("--emit-sql", action="store_true",
                    help="also write load.sql, which loads the files from the sqlite3 shell "
                         "(run it from inside the data directory; see the top of load.sql)")
    ap.add_argument("--db", metavar="PATH",
                    help="create PATH and load everything into it through Python's sqlite3 "
                         "module (rebuilt from scratch on every run)")
    ap.add_argument("--ext", metavar="PATH",
                    help="the sparqlite extension for --db, e.g. ext/target/release/libsparqlite "
                         "(default: $SPARQLITE_EXT, else ext/target/release/ next to this script)")
    ap.add_argument("--force", action="store_true",
                    help="with --db: replace PATH even if this script did not create it")
    args = ap.parse_args(argv)
    if args.people < 0:
        ap.error("--people must be >= 0")

    validate()
    out_dir = os.path.abspath(args.out)
    os.makedirs(out_dir, exist_ok=True)

    ttl, n_ttl = make_languages_ttl()
    write_text(os.path.join(out_dir, "languages.ttl"), ttl)
    print("wrote %-18s %7d triples (%d languages, %d people, %d organisations)" % (
        "languages.ttl", n_ttl, len(LANGUAGES), len(PERSONS), len(ORGS)))
    trig, n_trig = make_survey_trig()
    write_text(os.path.join(out_dir, "survey.trig"), trig)
    print("wrote %-18s %7d quads   (FICTIONAL; 3 named graphs + their metadata)" % (
        "survey.trig", n_trig))
    rows = make_job_rows()
    csv_text, sql_text = make_job_files(rows)
    write_text(os.path.join(out_dir, "job_postings.csv"), csv_text)
    write_text(os.path.join(out_dir, "job_postings.sql"), sql_text)
    print("wrote %-18s %7d rows    (FICTIONAL), and job_postings.sql" % (
        "job_postings.csv", len(rows)))

    files = [("languages.ttl", "turtle"), ("survey.trig", "trig")]
    people_path = os.path.join(out_dir, "people.nt")
    if args.people:
        t0 = time.perf_counter()
        n_people = write_people_nt(people_path, args.people, args.seed)
        print("wrote %-18s %7d triples (SYNTHETIC; %d people, %.1fs)" % (
            "people.nt", n_people, args.people, time.perf_counter() - t0))
        files.append(("people.nt", "ntriples"))
    elif os.path.exists(people_path):
        print("note: ignoring the existing people.nt (pass --people N to regenerate and load it)")

    if args.emit_sql:
        write_text(os.path.join(out_dir, "load.sql"), make_load_sql(bool(args.people)))
        print("wrote load.sql        -- then: cd %s && sqlite3 ../tutorial.db "
              "-cmd '.load /path/to/libsparqlite' '.read load.sql'" % nice_path(out_dir))

    if args.db:
        ext = args.ext or default_extension()
        if not ext:
            ap.error("--db needs --ext PATH (no build found in ext/target/release/)")
        build_database(args.db, ext, out_dir, files, args.force)
    elif args.ext:
        ap.error("--ext only makes sense together with --db")
    return 0


if __name__ == "__main__":
    sys.exit(main())
