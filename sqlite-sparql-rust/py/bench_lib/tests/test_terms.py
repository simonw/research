import unittest

from bench_lib.terms import (BNODE, RDF_LANG_STRING, UNBOUND, XSD, XSD_STRING, Term, canonical,
                             canonical_nt, from_sparql_json, parse_nt, to_nt)


class ParseNTriplesTest(unittest.TestCase):
    def test_iri_and_bnode(self):
        self.assertEqual(parse_nt("<http://e/a>"), Term.iri("http://e/a"))
        self.assertEqual(parse_nt("  <http://e/\\u00e9>  "), Term.iri("http://e/é"))
        self.assertEqual(parse_nt("_:b12"), Term.bnode("b12"))

    def test_literals(self):
        self.assertEqual(parse_nt('"plain"'), Term("literal", "plain", XSD_STRING))
        self.assertEqual(parse_nt('"chat"@FR-be'), Term("literal", "chat", RDF_LANG_STRING, "FR-be"))
        self.assertEqual(parse_nt(f'"5"^^<{XSD}integer>'), Term("literal", "5", XSD + "integer"))

    def test_escapes(self):
        t = parse_nt('"a\\tb\\nc\\"d\\\\e\\u00e9\\U0001F600"')
        self.assertEqual(t.value, 'a\tb\nc"d\\eé\U0001F600')
        self.assertEqual(parse_nt('"say \\"hi\\""@en').value, 'say "hi"')

    def test_round_trip(self):
        for text in ['<http://e/a>', '_:x', '"a\\"b\\\\c\\nd"', '"x"@en',
                     f'"1.5"^^<{XSD}decimal>', '"été"']:
            self.assertEqual(to_nt(parse_nt(text)), text)
        self.assertEqual(to_nt(Term.literal("s", XSD_STRING)), '"s"')

    def test_rejects_garbage(self):
        for bad in ["", "http://e/a", "<http://e/a", '"open', '"x"^^xsd:int', '"x"@', "_:",
                    "<a> trailing", '"bad \\q escape"']:
            with self.assertRaises(ValueError, msg=bad):
                parse_nt(bad)


class CanonicalTest(unittest.TestCase):
    def key(self, lex, dt):
        return canonical(Term.literal(lex, XSD + dt))

    def test_unbound_iri_bnode(self):
        self.assertEqual(canonical(None), UNBOUND)
        self.assertEqual(canonical_nt(None), UNBOUND)
        self.assertEqual(canonical(Term.iri("http://e/a")), "<http://e/a>")
        self.assertEqual(canonical(Term.bnode("b1")), BNODE)
        self.assertEqual(canonical(Term.bnode("f620617d")), canonical(Term.bnode("N26")))

    def test_integers_by_value_within_family(self):
        self.assertEqual(self.key("01", "integer"), self.key("1", "integer"))
        self.assertEqual(self.key("+7", "integer"), "integer:7")
        self.assertEqual(self.key("-0", "integer"), "integer:0")
        self.assertEqual(self.key("5", "int"), self.key("5", "integer"))
        self.assertNotEqual(self.key("5", "integer"), self.key("5", "decimal"))
        self.assertNotEqual(self.key("1_0", "integer"), "integer:10")  # Python-only syntax

    def test_decimals(self):
        self.assertEqual(self.key("12.50", "decimal"), self.key("12.5", "decimal"))
        self.assertEqual(self.key("12.50", "decimal"), "decimal:12.5")
        self.assertEqual(self.key("100", "decimal"), "decimal:100")
        self.assertEqual(self.key("-0.0", "decimal"), "decimal:0")
        self.assertEqual(self.key(".5", "decimal"), "decimal:0.5")
        self.assertEqual(self.key("5.", "decimal"), "decimal:5")

    def test_doubles_and_floats(self):
        self.assertEqual(self.key("1.0E2", "double"), self.key("100.0", "double"))
        self.assertEqual(self.key("1e2", "double"), "double:100.0")
        self.assertEqual(self.key("INF", "double"), "double:INF")
        self.assertEqual(self.key("-INF", "float"), "float:-INF")
        self.assertEqual(self.key("NaN", "double"), "double:NaN")
        self.assertEqual(self.key("1.1", "float"), self.key("1.10", "float"))
        self.assertNotEqual(self.key("1.1", "float"), self.key("1.1", "double"))

    def test_boolean_and_temporal(self):
        self.assertEqual(self.key("1", "boolean"), self.key("true", "boolean"))
        self.assertEqual(self.key("0", "boolean"), "boolean:false")
        self.assertEqual(self.key("2008-01-01T10:00:00.500", "dateTime"),
                         self.key("2008-01-01T10:00:00.5", "dateTime"))
        self.assertEqual(self.key("2008-01-01T10:00:00.000", "dateTime"),
                         "dateTime:2008-01-01T10:00:00")
        self.assertEqual(self.key("2008-01-01T10:00:00+00:00", "dateTime"),
                         self.key("2008-01-01T10:00:00Z", "dateTime"))
        self.assertNotEqual(self.key("2008-01-01T10:00:00Z", "dateTime"),
                            self.key("2008-01-01T10:00:00", "dateTime"))
        self.assertEqual(self.key("2008-01-01-00:00", "date"), "date:2008-01-01Z")
        self.assertEqual(self.key("10:00:00.50", "time"), "time:10:00:00.5")

    def test_end_of_day(self):
        # XSD: 24:00:00 is the first instant of the next day (pyoxigraph stores it so).
        cases = {"2008-01-01T24:00:00": "2008-01-02T00:00:00",
                 "2008-12-31T24:00:00Z": "2009-01-01T00:00:00Z",
                 "2008-02-28T24:00:00+05:00": "2008-02-29T00:00:00+05:00",
                 "2000-02-28T24:00:00": "2000-02-29T00:00:00",
                 "1900-02-28T24:00:00": "1900-03-01T00:00:00",
                 "2008-04-30T24:00:00.000": "2008-05-01T00:00:00",
                 "-0001-12-31T24:00:00": "0000-01-01T00:00:00",
                 "9999-12-31T24:00:00": "10000-01-01T00:00:00"}
        for lex, same in cases.items():
            self.assertEqual(self.key(lex, "dateTime"), self.key(same, "dateTime"), lex)
        self.assertEqual(self.key("24:00:00Z", "time"), self.key("00:00:00Z", "time"))
        # Hour 24 with a non-zero fraction is ill-typed: kept as it is.
        self.assertEqual(self.key("2008-01-01T24:00:00.5", "dateTime"),
                         f'"2008-01-01T24:00:00.5"^^<{XSD}dateTime>')

    def test_strings_and_languages(self):
        self.assertEqual(canonical(Term.literal("x")), canonical(Term.literal("x", XSD_STRING)))
        self.assertEqual(canonical(Term.literal("x", lang="EN")), canonical(Term.literal("x", lang="en")))
        self.assertNotEqual(canonical(Term.literal("x", lang="en")), canonical(Term.literal("x")))
        # A simple literal that looks like an IRI or a typed key stays distinct.
        self.assertNotEqual(canonical(Term.literal("<http://e/a>")), canonical(Term.iri("http://e/a")))
        self.assertNotEqual(canonical(Term.literal("integer:5")), self.key("5", "integer"))

    def test_ill_typed_and_unknown_keep_lexical(self):
        self.assertEqual(self.key("abc", "integer"), f'"abc"^^<{XSD}integer>')
        self.assertNotEqual(self.key("01x", "integer"), self.key("1x", "integer"))
        other = canonical(Term.literal("P1D", XSD + "duration"))
        self.assertEqual(other, f'"P1D"^^<{XSD}duration>')

    def test_cross_engine_surface_forms(self):
        # What pyoxigraph prints after canonicalising on load vs what was loaded.
        pairs = [(f'"12.50"^^<{XSD}decimal>', f'"12.5"^^<{XSD}decimal>'),
                 (f'"01"^^<{XSD}integer>', f'"1"^^<{XSD}integer>'),
                 (f'"1.0E2"^^<{XSD}double>', f'"100"^^<{XSD}double>'),
                 ('"x"@EN', '"x"@en'),
                 (f'"2008-01-01T24:00:00"^^<{XSD}dateTime>',
                  f'"2008-01-02T00:00:00"^^<{XSD}dateTime>'),
                 (f'"s"^^<{XSD}string>', '"s"')]
        for loaded, printed in pairs:
            self.assertEqual(canonical_nt(loaded), canonical_nt(printed), loaded)


class SparqlJsonTest(unittest.TestCase):
    def test_bindings(self):
        self.assertEqual(from_sparql_json({"type": "uri", "value": "http://e/a"}), Term.iri("http://e/a"))
        self.assertEqual(from_sparql_json({"type": "bnode", "value": "b0"}), Term.bnode("b0"))
        self.assertEqual(from_sparql_json({"type": "literal", "value": "x", "xml:lang": "en"}),
                         Term.literal("x", lang="en"))
        self.assertEqual(from_sparql_json({"type": "typed-literal", "value": "1",
                                           "datatype": XSD + "integer"}),
                         Term.literal("1", XSD + "integer"))
        self.assertEqual(from_sparql_json({"type": "literal", "value": "x"}), Term.literal("x"))
        with self.assertRaises(ValueError):
            from_sparql_json({"type": "triple", "value": {}})


if __name__ == "__main__":
    unittest.main()
