import unittest

from bench_lib.checksum import ResultSet, multiset_checksum, sequence_checksum


def rs(vars_, rows, form="select"):
    return ResultSet(form=form, vars=list(vars_), rows=[tuple(r) for r in rows])


class ChecksumTest(unittest.TestCase):
    rows = [("<a>", "integer:1"), ("<b>", "integer:2"), ("<b>", "integer:2")]

    def test_order_insensitive(self):
        a = rs(["x", "y"], self.rows)
        b = rs(["x", "y"], list(reversed(self.rows)))
        self.assertEqual(multiset_checksum(a), multiset_checksum(b))
        self.assertNotEqual(sequence_checksum(a), sequence_checksum(b))

    def test_column_order_insensitive(self):
        a = rs(["x", "y"], self.rows)
        b = rs(["y", "x"], [(r[1], r[0]) for r in self.rows])
        self.assertEqual(multiset_checksum(a), multiset_checksum(b))
        self.assertEqual(sequence_checksum(a), sequence_checksum(b))

    def test_multiplicity_matters(self):
        a = rs(["x", "y"], self.rows)
        b = rs(["x", "y"], self.rows[:2])
        c = rs(["x", "y"], self.rows[:2] + [self.rows[0]])
        self.assertNotEqual(multiset_checksum(a), multiset_checksum(b))
        self.assertNotEqual(multiset_checksum(a), multiset_checksum(c))
        self.assertTrue(multiset_checksum(a).startswith("3:"))

    def test_values_and_names_matter(self):
        a = rs(["x", "y"], self.rows)
        self.assertNotEqual(multiset_checksum(a), multiset_checksum(rs(["x", "z"], self.rows)))
        changed = [self.rows[0], self.rows[1], ("<b>", "integer:3")]
        self.assertNotEqual(multiset_checksum(a), multiset_checksum(rs(["x", "y"], changed)))
        # Cells are separated: ("ab", "c") must not collide with ("a", "bc").
        self.assertNotEqual(multiset_checksum(rs(["x", "y"], [("ab", "c")])),
                            multiset_checksum(rs(["x", "y"], [("a", "bc")])))

    def test_empty_and_ask(self):
        self.assertEqual(multiset_checksum(rs(["x"], [])), multiset_checksum(rs(["y", "z"], [])))
        self.assertEqual(multiset_checksum(ResultSet("ask", boolean=True)), "ask:true")
        self.assertEqual(sequence_checksum(ResultSet("ask", boolean=False)), "ask:false")
        self.assertEqual(len(ResultSet("ask", boolean=False)), 1)

    def test_duplicate_variables_rejected(self):
        with self.assertRaises(ValueError):
            multiset_checksum(rs(["x", "x"], [("a", "b")]))


if __name__ == "__main__":
    unittest.main()
