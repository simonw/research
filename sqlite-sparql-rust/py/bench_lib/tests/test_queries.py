import os
import re
import tempfile
import unittest

from bench_lib.queries import (COUNT_VARIABLE, count_query, load_queries, parse_query_file,
                               parse_selection, query_form, query_projection, select_queries)

QUERY_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                         "queries")


class ShippedQueriesTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.queries = load_queries(QUERY_DIR)

    def test_all_seventeen(self):
        self.assertEqual([q.id for q in self.queries], [f"Q{i}" for i in range(1, 18)])

    def test_datasets(self):
        bsbm = {q.id for q in self.queries if q.dataset == "bsbm"}
        self.assertEqual(bsbm, {"Q2", "Q14", "Q17"})
        self.assertTrue(all(q.dataset in ("social", "bsbm") for q in self.queries))

    def test_ordered_flags_match_order_by_limit(self):
        for q in self.queries:
            body = re.sub(r"#[^\n]*", "", q.text).upper()
            has_topk = "ORDER BY" in body and "LIMIT" in body
            self.assertEqual(q.ordered, has_topk, q.id)
        self.assertEqual({q.id for q in self.queries if q.ordered}, {"Q8", "Q9", "Q17"})

    def test_projection_matches_pyoxigraph(self):
        """The SQLite adapters fetch t0..t{n-1} from the parsed projection, so it must
        be exactly the engine's (spargebra's) projection, in order."""
        import pyoxigraph
        empty = pyoxigraph.Store()
        for q in self.queries:
            expected = tuple(v.value for v in empty.query(q.text).variables)
            self.assertEqual(q.projection, expected, q.id)

    def test_count_rewrite_counts_the_same_solutions(self):
        """Count mode of the reference engines runs ``count_query``; it must count
        exactly the rows the query returns, on every benchmark query."""
        import pyoxigraph
        store = pyoxigraph.Store()
        data = "".join(
            f"<http://example.org/social/p{i}> <http://example.org/social/{p}> {o} .\n"
            for i in range(12) for p, o in (
                ("knows", f"<http://example.org/social/p{(i * 7 + 1) % 12}>"),
                ("knows", f"<http://example.org/social/p{(i + 5) % 12}>"),
                ("name", f'"Person {i}"'),
                ("age", f'"{90 + i % 5}"^^<http://www.w3.org/2001/XMLSchema#integer>'),
                ("city", f"<http://example.org/social/city{i % 3}>")))
        data += "".join(f"<http://example.org/social/p{i}> "
                        f"<http://www.w3.org/1999/02/22-rdf-syntax-ns#type> "
                        f"<http://example.org/social/C{i % 10}> .\n" for i in range(12))
        store.load(data.encode(), format=pyoxigraph.RdfFormat.N_TRIPLES)
        for q in self.queries:
            counted = q.count_text
            self.assertIsNotNone(counted, q.id)
            self.assertTrue(counted.startswith(q.text[:q.text.index("SELECT")]), q.id)
            (solution,) = store.query(counted)
            self.assertEqual(int(solution[0].value), len(list(store.query(q.text))), q.id)

    def test_all_select_with_explicit_prefixes(self):
        for q in self.queries:
            self.assertEqual(q.form, "select", q.id)
            bare = re.sub(r"<[^>]*>|#[^\n]*|\"[^\"]*\"", "", q.text)
            used = set(re.findall(r"(?<![\w<:/])([a-z]*):[A-Za-z]", bare))
            declared = set(re.findall(r"PREFIX\s+([a-z]*):", q.text))
            self.assertLessEqual(used, declared, q.id)


class ParsingTest(unittest.TestCase):
    def write(self, text):
        fd, path = tempfile.mkstemp(suffix=".rq")
        with os.fdopen(fd, "w") as fh:
            fh.write(text)
        self.addCleanup(os.unlink, path)
        return path

    def test_header(self):
        q = parse_query_file(self.write("# Q42: the answer\n# dataset: bsbm\n# ordered: yes\n"
                                        "# about it\nASK { ?s ?p ?o }\n"))
        self.assertEqual((q.id, q.title, q.dataset, q.ordered, q.form, q.number),
                         ("Q42", "the answer", "bsbm", True, "ask", 42))
        self.assertEqual(q.label, "Q42 the answer")

    def test_bad_headers(self):
        for text in ["SELECT * {}", "# Q1: x\nSELECT * {}", "# Q1: x\n# dataset: other\nSELECT * {}",
                     "# Q1: x\n# dataset: social\n# ordered: maybe\nSELECT * {}", ""]:
            with self.assertRaises(ValueError, msg=text):
                parse_query_file(self.write(text))

    def test_query_form_ignores_comments_iris_and_strings(self):
        self.assertEqual(query_form("# SELECT\nPREFIX ask: <http://e/select#>\nCONSTRUCT {} WHERE {}"),
                         "construct")
        self.assertEqual(query_form('ASK { ?s ?p "SELECT" }'), "ask")
        self.assertEqual(query_form("prefix : <http://e/>\n describe :x"), "describe")
        self.assertEqual(query_form("PREFIX : <http://e/> SELECT ?ask WHERE { ?ask :select :ask }"), "select")
        with self.assertRaises(ValueError):
            query_form("# nothing here")

    def test_projection(self):
        cases = {
            "SELECT ?a $b WHERE {}": ("a", "b"),
            "select distinct ?a (COUNT(DISTINCT ?b) AS ?n) {}": ("a", "n"),
            "SELECT REDUCED (CONCAT(?a, \"(AS ?x)\") AS ?c) ?d FROM <http://g> WHERE {}": ("c", "d"),
            "PREFIX ex: <http://e/SELECT> SELECT (ex:AS(?a) AS ?b) {}": ("b",),
            "# SELECT ?no\nSELECT ?yes { ?yes ?p ?o }": ("yes",),
            "SELECT * WHERE {}": None,
            "SELECT ?a * {}": None,
            "SELECT (?a AS) {}": None,
            "SELECT (?a) {}": None,
            "SELECT ?a ?a {}": None,
            "SELECT ?a": None,
            "SELECT ?a ) {}": None,
            "ASK { ?a ?b ?c }": None,
            "CONSTRUCT { ?a ?b ?c } WHERE { ?a ?b ?c }": None,
        }
        for text, expected in cases.items():
            self.assertEqual(query_projection(text), expected, text)

    def test_count_query(self):
        q = "PREFIX : <http://e/>\n# SELECT ?bench_count in a comment\nSELECT ?n WHERE { ?n :p ?o }"
        self.assertEqual(count_query(q),
                         "PREFIX : <http://e/>\n# SELECT ?bench_count in a comment\n"
                         f"SELECT (COUNT(*) AS ?{COUNT_VARIABLE}1) WHERE {{\n"
                         "SELECT ?n WHERE { ?n :p ?o }\n}\n")
        # A trailing comment cannot swallow the closing brace.
        self.assertTrue(count_query("SELECT * { ?s ?p ?o } # end").endswith("# end\n}\n"))
        # Keywords inside IRIs, strings and comments are not the query form.
        q = 'BASE <http://e/SELECT> PREFIX x: <http://e/a#SELECT> SELECT ?s { ?s ?p "ASK" }'
        self.assertTrue(count_query(q).startswith(
            "BASE <http://e/SELECT> PREFIX x: <http://e/a#SELECT> SELECT (COUNT(*)"))
        self.assertIsNotNone(count_query("SELECT (EXISTS { ?a ?b ?c } AS ?e) WHERE {}"))
        self.assertIsNotNone(count_query("SELECT ?x WHERE { ?x ?p ?o } VALUES ?x { <http://e/> }"))
        for text in ["ASK { ?s ?p ?o }", "CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }",
                     "DESCRIBE <http://e/x>", "SELECT ?s FROM <http://g> WHERE { ?s ?p ?o }",
                     "select (1 AS ?one) from named <http://g> { }", "# no query"]:
            self.assertIsNone(count_query(text), text)

    def test_selection(self):
        self.assertEqual(parse_selection("Q1, q3-Q5,7"), {1, 3, 4, 5, 7})
        for bad in ["Q5-Q3", "x", "1-2-3"]:
            with self.assertRaises(ValueError, msg=bad):
                parse_selection(bad)
        queries = load_queries(QUERY_DIR)
        self.assertEqual([q.id for q in select_queries(queries, "Q2,Q17")], ["Q2", "Q17"])
        self.assertEqual(len(select_queries(queries, None)), 17)
        with self.assertRaises(ValueError):
            select_queries(queries, "Q99")


if __name__ == "__main__":
    unittest.main()
