#!/usr/bin/env python3
"""Deterministic generator for the *social* benchmark dataset (docs/SPEC.md §12.4).

Every person ``:p{i}`` (``i`` in ``0..N-1``) gets exactly eight triples::

    :p{i} rdf:type :C{i % 10}
    :p{i} :name    "Person {i}"
    :p{i} :age     "{a}"^^xsd:integer        a in 0..99, population-pyramid skew
    :p{i} :knows   :p{j}   (x3)              distinct, never i; power-law popularity
    :p{i} :city    :city{k}                  Zipf(s=1) over --cities cities
    :p{i} :joined  "YYYY-MM-DD"^^xsd:date    uniform over 2005-2024

so 200K persons give 1.6M triples and 625K give 5M (``:prefix`` is
``http://example.org/social/``).  The output is N-Triples in subject order.  The same
``--seed`` always produces byte-identical output.

Because every person has out-degree exactly 3, chain-2 (``?a :knows ?b . ?b :knows
?c``) always has ``9 * N`` solutions, independent of the popularity skew.
"""

from __future__ import annotations

import argparse
import datetime as _dt
import math
import os
import random
import sys
import time
from typing import TextIO

from bench_lib.generation import (GenStats, PowerLawRanks, cumulative, distinct_draws, pick,
                                  write_atomically, zipf_cumulative)

NS = "http://example.org/social/"
RDF_TYPE = "<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"
XSD = "http://www.w3.org/2001/XMLSchema#"

DEFAULT_PERSONS = 200_000
DEFAULT_SEED = 42
DEFAULT_CITIES = 1000
KNOWS_PER_PERSON = 3
TRIPLES_PER_PERSON = 5 + KNOWS_PER_PERSON
CLASSES = 10

# In-degree follows a continuous power law over popularity ranks (rank 0 receives
# ~1.4% of all :knows edges at 200K persons); out-degree is fixed.
POPULARITY_EXPONENT = 0.8
CITY_ZIPF_EXPONENT = 1.0

JOINED_FIRST = _dt.date(2005, 1, 1)
JOINED_DAYS = (_dt.date(2024, 12, 31) - JOINED_FIRST).days + 1

_FLUSH_LINES = 65536


def age_weights() -> list[float]:
    """Cumulative weights for ages 0..99: flat to 59, then exponential decay.

    About 0.7% of persons are older than 90, so ``FILTER(?a > 90)`` is selective.
    """
    return cumulative(1.0 if a < 60 else math.exp(-(a - 60) / 12.0) for a in range(100))


def generate(out: TextIO, persons: int = DEFAULT_PERSONS, seed: int = DEFAULT_SEED,
             cities: int = DEFAULT_CITIES) -> tuple[int, int]:
    """Writes the dataset to ``out``; returns ``(triples, characters written)``."""
    if persons < KNOWS_PER_PERSON + 1:
        raise ValueError(f"need at least {KNOWS_PER_PERSON + 1} persons")
    if cities < 1:
        raise ValueError("need at least one city")
    rng = random.Random(seed)
    rand = rng.random

    # Popularity rank -> person, so the popular persons are scattered over the id space.
    popular = list(range(persons))
    rng.shuffle(popular)
    ranks = PowerLawRanks(persons, POPULARITY_EXPONENT)
    ages = age_weights()
    city_cum = zipf_cumulative(cities, CITY_ZIPF_EXPONENT)

    def knows_target() -> int:
        return popular[ranks.sample(rand())]

    person = [f"<{NS}p{i}>" for i in range(persons)]
    klass = [f" {RDF_TYPE} <{NS}C{c}> .\n" for c in range(CLASSES)]
    city = [f" <{NS}city> <{NS}city{k + 1}> .\n" for k in range(cities)]
    age_po = [f' <{NS}age> "{a}"^^<{XSD}integer> .\n' for a in range(100)]
    name_p = f' <{NS}name> "Person '
    knows_p = f" <{NS}knows> "
    joined_p = f' <{NS}joined> "'
    joined_dt = f'"^^<{XSD}date> .\n'

    triples = 0
    written = 0
    buf: list[str] = []
    for i in range(persons):
        s = person[i]
        age = pick(ages, rand())
        home = pick(city_cum, rand())
        joined = JOINED_FIRST + _dt.timedelta(days=int(rand() * JOINED_DAYS))
        buf.append(s + klass[i % CLASSES])
        buf.append(f'{s}{name_p}{i}" .\n')
        buf.append(s + age_po[age])
        for j in distinct_draws(KNOWS_PER_PERSON, knows_target, exclude=i):
            buf.append(f"{s}{knows_p}{person[j]} .\n")
        buf.append(s + city[home])
        buf.append(f"{s}{joined_p}{joined.isoformat()}{joined_dt}")
        triples += TRIPLES_PER_PERSON
        if len(buf) >= _FLUSH_LINES:
            chunk = "".join(buf)
            out.write(chunk)
            written += len(chunk)
            buf.clear()
    chunk = "".join(buf)
    out.write(chunk)
    written += len(chunk)
    return triples, written


def generate_file(path: str, persons: int = DEFAULT_PERSONS, seed: int = DEFAULT_SEED,
                  cities: int = DEFAULT_CITIES) -> GenStats:
    start = time.perf_counter()
    triples, _ = write_atomically(path, lambda fh: generate(fh, persons, seed, cities))
    return GenStats(persons, triples, os.path.getsize(path), time.perf_counter() - start)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Generate the social benchmark dataset (N-Triples).")
    ap.add_argument("--persons", type=int, default=DEFAULT_PERSONS,
                    help=f"number of persons (default {DEFAULT_PERSONS} = 1.6M triples; 625000 = 5M)")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED, help=f"RNG seed (default {DEFAULT_SEED})")
    ap.add_argument("--cities", type=int, default=DEFAULT_CITIES,
                    help=f"number of cities (default {DEFAULT_CITIES})")
    ap.add_argument("--out", required=True, help="output N-Triples file ('-' for stdout)")
    args = ap.parse_args(argv)
    if args.out == "-":
        start = time.perf_counter()
        triples, chars = generate(sys.stdout, args.persons, args.seed, args.cities)
        stats = GenStats(args.persons, triples, chars, time.perf_counter() - start)
    else:
        stats = generate_file(args.out, args.persons, args.seed, args.cities)
    print(f"gen_social: {stats.entities} persons, {stats.triples} triples, "
          f"{stats.bytes / 1e6:.1f} MB in {stats.seconds:.2f} s -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
