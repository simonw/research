import collections
import contextlib
import hashlib
import io
import os
import re
import tempfile
import unittest

import gen_bsbm
import gen_social
from bench_lib.datasets import count_triples, open_dataset
from bench_lib.generation import PowerLawRanks, cumulative, distinct_draws, pick, zipf_cumulative

SOCIAL = "http://example.org/social/"
XSD = "http://www.w3.org/2001/XMLSchema#"
LINE = re.compile(r'(<[^>]+>) (<[^>]+>) (<[^>]+>|"(?:[^"\\]|\\.)*"(?:\^\^<[^>]+>|@[\w-]+)?) \.')


def social_text(persons, seed=1, cities=50):
    buf = io.StringIO()
    triples, chars = gen_social.generate(buf, persons, seed, cities)
    text = buf.getvalue()
    return text, triples, chars


def bsbm_text(products, seed=1):
    buf = io.StringIO()
    triples, chars = gen_bsbm.generate(buf, products, seed)
    return buf.getvalue(), triples, chars


def parse(text):
    out = []
    for line in text.splitlines():
        m = LINE.fullmatch(line)
        assert m, line
        out.append(m.groups())
    return out


class SamplingTest(unittest.TestCase):
    def test_pick_and_cumulative(self):
        cum = cumulative([1, 0, 3])
        self.assertEqual([pick(cum, u) for u in (0.0, 0.2, 0.25, 0.9, 0.999999)], [0, 0, 2, 2, 2])
        with self.assertRaises(ValueError):
            cumulative([0, 0])
        with self.assertRaises(ValueError):
            cumulative([1, -1])
        z = zipf_cumulative(3, 1.0)
        self.assertAlmostEqual(z[-1], 1 + 1 / 2 + 1 / 3)

    def test_power_law_ranks(self):
        ranks = PowerLawRanks(1000, 0.8)
        self.assertEqual(ranks.sample(0.0), 0)
        self.assertEqual(ranks.sample(0.999999999), 999)
        draws = [ranks.sample(i / 10000) for i in range(10000)]
        self.assertTrue(all(0 <= d < 1000 for d in draws))
        counts = collections.Counter(draws)
        self.assertGreater(counts[0], counts[500] * 10)
        with self.assertRaises(ValueError):
            PowerLawRanks(10, 1.0)

    def test_distinct_draws(self):
        values = iter([1, 1, 2, 5, 2, 3])
        self.assertEqual(distinct_draws(3, lambda: next(values), exclude=5), [1, 2, 3])


class SocialGeneratorTest(unittest.TestCase):
    def test_deterministic(self):
        a, *_ = social_text(300, seed=5)
        b, *_ = social_text(300, seed=5)
        c, *_ = social_text(300, seed=6)
        self.assertEqual(a, b)
        self.assertNotEqual(a, c)

    def test_shape(self):
        persons = 500
        text, triples, chars = social_text(persons, cities=40)
        self.assertEqual(triples, 8 * persons)
        self.assertEqual(chars, len(text))
        rows = parse(text)
        self.assertEqual(len(rows), triples)
        by_pred = collections.defaultdict(list)
        for s, p, o in rows:
            by_pred[p].append((s, o))
        knows = by_pred[f"<{SOCIAL}knows>"]
        self.assertEqual(len(knows), 3 * persons)
        out = collections.defaultdict(set)
        for s, o in knows:
            self.assertNotEqual(s, o)
            out[s].add(o)
        self.assertTrue(all(len(v) == 3 for v in out.values()))
        self.assertEqual(len(out), persons)
        ages = [int(o.split('"')[1]) for _, o in by_pred[f"<{SOCIAL}age>"]]
        self.assertTrue(all(0 <= a <= 99 for a in ages))
        self.assertTrue(all(o.endswith(f"^^<{XSD}integer>") for _, o in by_pred[f"<{SOCIAL}age>"]))
        self.assertLess(sum(a > 90 for a in ages), persons * 0.05)
        types = dict(by_pred["<http://www.w3.org/1999/02/22-rdf-syntax-ns#type>"])
        self.assertEqual(types[f"<{SOCIAL}p13>"], f"<{SOCIAL}C3>")
        names = dict(by_pred[f"<{SOCIAL}name>"])
        self.assertEqual(names[f"<{SOCIAL}p123>"], '"Person 123"')
        cities = collections.Counter(o for _, o in by_pred[f"<{SOCIAL}city>"])
        self.assertEqual(cities.most_common(1)[0][0], f"<{SOCIAL}city1>")
        self.assertTrue(all(re.fullmatch(rf'"\d{{4}}-\d\d-\d\d"\^\^<{XSD}date>', o)
                            for _, o in by_pred[f"<{SOCIAL}joined>"]))
        self.assertEqual(len(set(rows)), len(rows))

    def test_rejects_tiny(self):
        with self.assertRaises(ValueError):
            social_text(3)

    def test_file_output_and_dataset(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "s.nt")
            stats = gen_social.generate_file(path, 200, 3, 20)
            self.assertEqual(stats.triples, 1600)
            self.assertEqual(count_triples(path), 1600)
            ds = open_dataset("social", path)
            self.assertEqual((ds.triples, ds.bytes), (1600, os.path.getsize(path)))
            self.assertEqual(os.listdir(tmp), ["s.nt"])  # no temporary file left behind
            # The fingerprint is the content hash: same bytes elsewhere, same fingerprint.
            with open(path, "rb") as fh:
                content = fh.read()
            self.assertEqual(ds.fingerprint, hashlib.sha256(content).hexdigest())
            copy = os.path.join(tmp, "copy.nt")
            with open(copy, "wb") as fh:
                fh.write(content)
            self.assertEqual(open_dataset("social", copy).fingerprint, ds.fingerprint)
            self.assertEqual(ds.to_json()["sha256"], ds.fingerprint)
            with contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(gen_social.main(["--persons", "10", "--out", path]), 0)
            self.assertIn("80 triples", err.getvalue())
            self.assertEqual(count_triples(path), 80)
        with self.assertRaises(FileNotFoundError):
            open_dataset("social", "/nonexistent/x.nt")


class BsbmGeneratorTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text, cls.triples, _ = bsbm_text(60, seed=9)
        cls.rows = parse(cls.text)

    def test_deterministic(self):
        self.assertEqual(bsbm_text(60, seed=9)[0], self.text)
        self.assertNotEqual(bsbm_text(60, seed=10)[0], self.text)

    def test_counts_and_uniqueness(self):
        self.assertEqual(len(self.rows), self.triples)
        self.assertEqual(len(set(self.rows)), len(self.rows))

    def test_type_hierarchy_reaches_product(self):
        sub = {s: o for s, p, o in self.rows if p.endswith("rdf-schema#subClassOf>")}
        self.assertEqual(len(sub), 3 + 12 + 48 + 240)
        root = f"<{gen_bsbm.BSBM}Product>"
        for t in sub:
            seen = 0
            while t != root:
                t = sub[t]
                seen += 1
                self.assertLess(seen, 6)
        product_types = [o for s, p, o in self.rows
                         if p.endswith("#type>") and "/Product" in s and "ProductType" not in s
                         and "ProductFeature" not in s]
        self.assertEqual(len(product_types), 60)
        self.assertTrue(all(t in sub for t in product_types))

    def test_prices(self):
        prices = [o for s, p, o in self.rows if p == f"<{gen_bsbm.BSBM}price>"]
        self.assertEqual(len(prices), 600)
        decimals = [o for o in prices if o.endswith("decimal>")]
        integers = [o for o in prices if o.endswith("integer>")]
        self.assertEqual(len(decimals) + len(integers), len(prices))
        self.assertTrue(integers and decimals)
        for o in decimals:
            lex = o.split('"')[1]
            self.assertRegex(lex, r"^\d+\.\d\d$")
            self.assertNotEqual(lex[-2:], "00")  # never equal to an integer price

    def test_datetimes_have_no_timezone(self):
        for _s, _p, o in self.rows:
            if o.endswith(f"^^<{XSD}dateTime>"):
                self.assertRegex(o.split('"')[1], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d$")

    def test_default_size_is_about_one_million(self):
        per_product = self.triples / 60
        self.assertAlmostEqual(per_product * gen_bsbm.DEFAULT_PRODUCTS / 1e6, 1.0, delta=0.15)


if __name__ == "__main__":
    unittest.main()
