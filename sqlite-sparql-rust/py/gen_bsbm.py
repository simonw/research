#!/usr/bin/env python3
"""Deterministic generator for the *bsbm-lite* benchmark dataset (docs/SPEC.md §12.4).

A scaled-down Berlin SPARQL Benchmark shape, driven by ``--products P``:

* a product-type hierarchy under ``bsbm:Product`` (4 levels, 3x4x4x5 = 303 types,
  each ``rdfs:subClassOf`` its parent); products are typed with a leaf type (90%) or
  a level-3 type (10%) only, so ``rdf:type/rdfs:subClassOf* bsbm:Product`` needs the
  closure;
* ``max(32, P/4)`` product features (Zipf popularity), ``max(2, P/40)`` producers,
  ``max(2, P/80)`` vendors and ``max(4, P/2)`` reviewers;
* products: label, ``@en`` comment, producer, 4-10 features, 1-3 integer numeric
  properties, 1-2 textual properties, publication ``xsd:date``;
* ``10 P`` offers: product, vendor, price, ``validFrom``/``validTo`` ``xsd:dateTime``
  (no time zone), delivery days.  85% of prices are ``xsd:decimal`` with exactly two
  decimals and non-zero cents (so ~9% are non-canonical such as ``"12.50"``), 15% are
  ``xsd:integer``; no decimal price ever equals an integer price, which keeps
  ``ORDER BY ?price ?offer`` a total order on every engine;
* ``5 P`` reviews (power-law over products): product, reviewer, title, review
  ``xsd:dateTime``, ratings 1-10 (each of 4 present with probability 3/4).

The default ``--products 7800`` gives about 1M triples.  Same seed => same bytes.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import os
import random
import sys
import time
from collections.abc import Callable
from typing import TextIO

from bench_lib.generation import (GenStats, PowerLawRanks, distinct_draws, pick, write_atomically,
                                  zipf_cumulative)

RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
RDFS = "http://www.w3.org/2000/01/rdf-schema#"
XSD = "http://www.w3.org/2001/XMLSchema#"
FOAF = "http://xmlns.com/foaf/0.1/"
DC = "http://purl.org/dc/elements/1.1/"
REV = "http://purl.org/stuff/rev#"
BSBM = "http://example.org/bsbm/vocabulary/"
INST = "http://example.org/bsbm/instances/"
COUNTRY = "http://downlode.org/rdf/iso-3166/countries#"

DEFAULT_PRODUCTS = 7800
DEFAULT_SEED = 42
TYPE_BRANCHING = (3, 4, 4, 5)
OFFERS_PER_PRODUCT = 10
REVIEWS_PER_PRODUCT = 5
INTEGER_PRICE_SHARE = 0.15
RATING_PRESENT = 0.75

COUNTRIES = ("US", "GB", "DE", "FR", "JP", "CN", "RU", "ES", "AT", "KR")
WORDS = (
    "alpha", "amber", "anchor", "apex", "arc", "aria", "atlas", "aurora", "axis", "beacon",
    "bolt", "breeze", "bright", "bronze", "canyon", "carbon", "cascade", "cedar", "chrome",
    "cinder", "cipher", "citadel", "clarity", "cobalt", "comet", "coral", "cosmic", "crest",
    "crystal", "dawn", "delta", "drift", "dune", "dynamo", "echo", "eclipse", "ember",
    "emerald", "epoch", "falcon", "fern", "fjord", "flare", "flux", "forge", "frost", "fusion",
    "galaxy", "garnet", "glacier", "glow", "granite", "gravity", "harbor", "haven", "helix",
    "horizon", "indigo", "iris", "ivory", "jade", "jet", "jubilee", "karma", "kernel", "kinetic",
    "lagoon", "laser", "lattice", "legend", "lumen", "lunar", "magma", "maple", "marble",
    "matrix", "meadow", "meridian", "mesa", "meteor", "mirage", "monarch", "mosaic", "nebula",
    "nimbus", "noble", "nova", "oasis", "obsidian", "onyx", "orbit", "orchid", "pacific",
    "paragon", "pearl", "phoenix", "pinnacle", "pixel", "plasma", "polar", "prism", "pulse",
    "quantum", "quartz", "quest", "radiant", "raven", "ripple", "river", "rocket", "ruby",
    "saga", "sapphire", "scarlet", "sentinel", "shadow", "sierra", "silver", "solar", "sonic",
    "spark", "spectrum", "sphere", "spire", "stellar", "storm", "summit", "tango", "terra",
    "thunder", "titan", "topaz", "tundra", "twilight", "ultra", "umbra", "valley", "vector",
    "velvet", "vertex", "vista", "vivid", "vortex", "wave", "willow", "zenith", "zephyr",
)

OFFER_FIRST = _dt.datetime(2008, 1, 1)
OFFER_WINDOW_S = 180 * 86400
REVIEW_FIRST = _dt.datetime(2007, 6, 1)
REVIEW_WINDOW_S = 395 * 86400
PUBLISHED_FIRST = _dt.date(2000, 1, 1)
PUBLISHED_DAYS = 9 * 365


def _iri(ns: str, local: str) -> str:
    return f"<{ns}{local}>"


def _lit(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def _typed(lex: str, dt: str) -> str:
    return f'"{lex}"^^<{XSD}{dt}>'


class _Writer:
    """Buffers N-Triples lines and counts them."""

    def __init__(self, out: TextIO) -> None:
        self._out = out
        self._buf: list[str] = []
        self.triples = 0
        self.chars = 0

    def triple(self, s: str, p: str, o: str) -> None:
        self._buf.append(f"{s} {p} {o} .\n")
        if len(self._buf) >= 65536:
            self.flush()

    def flush(self) -> None:
        chunk = "".join(self._buf)
        self._out.write(chunk)
        self.chars += len(chunk)
        self.triples += len(self._buf)
        self._buf.clear()


def _type_hierarchy() -> tuple[list[tuple[int, int, int]], list[int], list[int]]:
    """``(type, parent, level)`` triples (parent 0 = ``bsbm:Product``), leaf and level-3 ids."""
    types: list[tuple[int, int, int]] = []
    frontier = [0]
    next_id = 1
    for level, branching in enumerate(TYPE_BRANCHING, start=1):
        children = []
        for parent in frontier:
            for _ in range(branching):
                types.append((next_id, parent, level))
                children.append(next_id)
                next_id += 1
        frontier = children
    leaves = [t for t, _, lvl in types if lvl == len(TYPE_BRANCHING)]
    mids = [t for t, _, lvl in types if lvl == len(TYPE_BRANCHING) - 1]
    return types, leaves, mids


def generate(out: TextIO, products: int = DEFAULT_PRODUCTS, seed: int = DEFAULT_SEED) -> tuple[int, int]:
    """Writes the dataset to ``out``; returns ``(triples, characters written)``."""
    if products < 1:
        raise ValueError("need at least one product")
    rng = random.Random(seed)
    rand = rng.random
    w = _Writer(out)

    def randint(lo: int, hi: int) -> int:
        return lo + int(rand() * (hi - lo + 1))

    def words(lo: int, hi: int) -> str:
        return " ".join(WORDS[int(rand() * len(WORDS))] for _ in range(randint(lo, hi)))

    def country() -> str:
        return _iri(COUNTRY, COUNTRIES[int(rand() * len(COUNTRIES))])

    def instant(first: _dt.datetime, window_s: int) -> _dt.datetime:
        return first + _dt.timedelta(seconds=int(rand() * window_s))

    a = _iri(RDF, "type")
    label = _iri(RDFS, "label")
    n_features = max(32, products // 4)
    n_producers = max(2, products // 40)
    n_vendors = max(2, products // 80)
    n_reviewers = max(4, products // 2)

    # Product types.
    types, leaves, mids = _type_hierarchy()
    product_root = _iri(BSBM, "Product")
    for t, parent, _ in types:
        s = _iri(INST, f"ProductType{t}")
        w.triple(s, a, _iri(BSBM, "ProductType"))
        w.triple(s, label, _lit(f"{words(1, 2)} type"))
        w.triple(s, _iri(RDFS, "subClassOf"),
                 product_root if parent == 0 else _iri(INST, f"ProductType{parent}"))

    for f in range(1, n_features + 1):
        s = _iri(INST, f"ProductFeature{f}")
        w.triple(s, a, _iri(BSBM, "ProductFeature"))
        w.triple(s, label, _lit(words(1, 3)))

    def organisation(kind: str, n: int) -> None:
        for k in range(1, n + 1):
            s = _iri(INST, f"{kind}{k}")
            w.triple(s, a, _iri(BSBM, kind))
            w.triple(s, label, _lit(f"{words(1, 2)} {kind.lower()}"))
            w.triple(s, _iri(BSBM, "country"), country())
            w.triple(s, _iri(FOAF, "homepage"), f"<http://www.{kind}{k}.example.com/>")

    organisation("Producer", n_producers)
    organisation("Vendor", n_vendors)

    for r in range(1, n_reviewers + 1):
        s = _iri(INST, f"Reviewer{r}")
        w.triple(s, a, _iri(FOAF, "Person"))
        w.triple(s, _iri(FOAF, "name"), _lit(words(2, 2).title()))
        w.triple(s, _iri(BSBM, "country"), country())

    # Products.
    feature_cum = zipf_cumulative(n_features, 0.9)

    def feature() -> int:
        return pick(feature_cum, rand()) + 1

    base_price: list[float] = [0.0]
    for p in range(1, products + 1):
        s = _iri(INST, f"Product{p}")
        ptype = leaves[int(rand() * len(leaves))] if rand() < 0.9 else mids[int(rand() * len(mids))]
        w.triple(s, a, _iri(INST, f"ProductType{ptype}"))
        w.triple(s, label, _lit(words(2, 4)))
        w.triple(s, _iri(RDFS, "comment"), _lit(words(6, 15)) + "@en")
        w.triple(s, _iri(BSBM, "producer"), _iri(INST, f"Producer{randint(1, n_producers)}"))
        for f in sorted(distinct_draws(min(randint(4, 10), n_features), feature)):
            w.triple(s, _iri(BSBM, "productFeature"), _iri(INST, f"ProductFeature{f}"))
        w.triple(s, _iri(BSBM, "productPropertyNumeric1"), _typed(str(randint(1, 2000)), "integer"))
        if rand() < 0.9:
            w.triple(s, _iri(BSBM, "productPropertyNumeric2"), _typed(str(randint(1, 2000)), "integer"))
        if rand() < 0.75:
            w.triple(s, _iri(BSBM, "productPropertyNumeric3"), _typed(str(randint(1, 2000)), "integer"))
        w.triple(s, _iri(BSBM, "productPropertyTextual1"), _lit(words(1, 3)))
        if rand() < 0.8:
            w.triple(s, _iri(BSBM, "productPropertyTextual2"), _lit(words(1, 3)))
        published = PUBLISHED_FIRST + _dt.timedelta(days=int(rand() * PUBLISHED_DAYS))
        w.triple(s, _iri(DC, "date"), _typed(published.isoformat(), "date"))
        base_price.append(5.0 * 2000.0 ** rand())  # log-uniform in [5, 10000)

    # Offers.
    for o in range(1, OFFERS_PER_PRODUCT * products + 1):
        s = _iri(INST, f"Offer{o}")
        product = randint(1, products)
        w.triple(s, a, _iri(BSBM, "Offer"))
        w.triple(s, _iri(BSBM, "product"), _iri(INST, f"Product{product}"))
        w.triple(s, _iri(BSBM, "vendor"), _iri(INST, f"Vendor{randint(1, n_vendors)}"))
        w.triple(s, _iri(BSBM, "price"), _price(base_price[product] * (0.8 + 0.4 * rand()), rand))
        valid_from = instant(OFFER_FIRST, OFFER_WINDOW_S)
        valid_to = valid_from + _dt.timedelta(seconds=int((1 + rand() * 119) * 86400))
        w.triple(s, _iri(BSBM, "validFrom"), _typed(valid_from.isoformat(), "dateTime"))
        w.triple(s, _iri(BSBM, "validTo"), _typed(valid_to.isoformat(), "dateTime"))
        w.triple(s, _iri(BSBM, "deliveryDays"), _typed(str(randint(1, 21)), "integer"))

    # Reviews: popular products (scattered over the id space) get more reviews.
    popular = list(range(1, products + 1))
    rng.shuffle(popular)
    ranks = PowerLawRanks(products, 0.6)
    for r in range(1, REVIEWS_PER_PRODUCT * products + 1):
        s = _iri(INST, f"Review{r}")
        w.triple(s, a, _iri(BSBM, "Review"))
        w.triple(s, _iri(BSBM, "reviewFor"), _iri(INST, f"Product{popular[ranks.sample(rand())]}"))
        w.triple(s, _iri(REV, "reviewer"), _iri(INST, f"Reviewer{randint(1, n_reviewers)}"))
        w.triple(s, _iri(DC, "title"), _lit(words(3, 7)))
        w.triple(s, _iri(BSBM, "reviewDate"),
                 _typed(instant(REVIEW_FIRST, REVIEW_WINDOW_S).isoformat(), "dateTime"))
        for k in range(1, 5):
            if rand() < RATING_PRESENT:
                w.triple(s, _iri(BSBM, f"rating{k}"), _typed(str(randint(1, 10)), "integer"))

    w.flush()
    return w.triples, w.chars


def _price(value: float, rand: Callable[[], float]) -> str:
    """An offer price literal: ``xsd:integer`` (15%) or two-decimal ``xsd:decimal``
    with non-zero cents, so integer and decimal prices never compare equal."""
    if rand() < INTEGER_PRICE_SHARE:
        return _typed(str(max(1, round(value))), "integer")
    whole = int(value)
    cents = int((value - whole) * 100) or 1
    return _typed(f"{whole}.{cents:02d}", "decimal")


def generate_file(path: str, products: int = DEFAULT_PRODUCTS, seed: int = DEFAULT_SEED) -> GenStats:
    start = time.perf_counter()
    triples, _ = write_atomically(path, lambda fh: generate(fh, products, seed))
    return GenStats(products, triples, os.path.getsize(path), time.perf_counter() - start)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Generate the bsbm-lite benchmark dataset (N-Triples).")
    ap.add_argument("--products", type=int, default=DEFAULT_PRODUCTS,
                    help=f"number of products (default {DEFAULT_PRODUCTS} = ~1M triples)")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED, help=f"RNG seed (default {DEFAULT_SEED})")
    ap.add_argument("--out", required=True, help="output N-Triples file ('-' for stdout)")
    args = ap.parse_args(argv)
    if args.out == "-":
        start = time.perf_counter()
        triples, chars = generate(sys.stdout, args.products, args.seed)
        stats = GenStats(args.products, triples, chars, time.perf_counter() - start)
    else:
        stats = generate_file(args.out, args.products, args.seed)
    print(f"gen_bsbm: {stats.entities} products, {stats.triples} triples, "
          f"{stats.bytes / 1e6:.1f} MB in {stats.seconds:.2f} s -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
