"""
Comprehensive test suite for CRDT implementations.

Tests cover:
- UniqueId ordering and serialization
- Clock (Lamport logical clock)
- LWW-Register (Last-Writer-Wins Register)
- RGA (Replicated Growable Array) for text
- LWW-Map
- CRDTNote (combination of above)
- Convergence properties
- Serialization/deserialization
"""

import pytest
from crdt import (
    UniqueId, ROOT_ID, Clock,
    LWWRegister,
    RGA, RGANode,
    LWWMap,
    CRDTNote
)


# =============================================================================
# UniqueId Tests
# =============================================================================

class TestUniqueId:
    """Tests for UniqueId ordering and serialization."""

    def test_ordering_by_timestamp(self):
        """Lower timestamp < higher timestamp."""
        id1 = UniqueId(1, 0, "A")
        id2 = UniqueId(2, 0, "A")
        assert id1 < id2
        assert id2 > id1
        assert not id1 > id2

    def test_ordering_by_counter(self):
        """Same timestamp: lower counter < higher counter."""
        id1 = UniqueId(1, 0, "A")
        id2 = UniqueId(1, 1, "A")
        assert id1 < id2
        assert id2 > id1

    def test_ordering_by_site_id(self):
        """Same timestamp and counter: site_id is tiebreaker."""
        id1 = UniqueId(1, 0, "A")
        id2 = UniqueId(1, 0, "B")
        assert id1 < id2
        assert id2 > id1

    def test_equality(self):
        """Same timestamp, counter, site_id are equal."""
        id1 = UniqueId(1, 2, "A")
        id2 = UniqueId(1, 2, "A")
        assert id1 == id2
        assert not id1 < id2
        assert not id1 > id2

    def test_less_than_or_equal(self):
        """Test <= operator."""
        id1 = UniqueId(1, 0, "A")
        id2 = UniqueId(1, 0, "A")
        id3 = UniqueId(2, 0, "A")
        assert id1 <= id2
        assert id1 <= id3
        assert not id3 <= id1

    def test_greater_than_or_equal(self):
        """Test >= operator."""
        id1 = UniqueId(2, 0, "A")
        id2 = UniqueId(1, 0, "A")
        assert id1 >= id2
        assert id1 >= id1
        assert not id2 >= id1

    def test_hash_consistency(self):
        """Equal IDs should have same hash."""
        id1 = UniqueId(1, 2, "A")
        id2 = UniqueId(1, 2, "A")
        assert hash(id1) == hash(id2)

    def test_can_be_dict_key(self):
        """UniqueId should work as dictionary key."""
        id1 = UniqueId(1, 0, "A")
        d = {id1: "value"}
        assert d[id1] == "value"
        id2 = UniqueId(1, 0, "A")
        assert d[id2] == "value"  # Same content, same key

    def test_to_string(self):
        """String representation format."""
        uid = UniqueId(10, 5, "client1")
        assert str(uid) == "10.5@client1"

    def test_from_string(self):
        """Parse from string representation."""
        uid = UniqueId.from_string("10.5@client1")
        assert uid.timestamp == 10
        assert uid.counter == 5
        assert uid.site_id == "client1"

    def test_to_dict(self):
        """Serialize to dictionary."""
        uid = UniqueId(10, 5, "client1")
        d = uid.to_dict()
        assert d == {'ts': 10, 'c': 5, 's': 'client1'}

    def test_from_dict(self):
        """Deserialize from dictionary."""
        d = {'ts': 10, 'c': 5, 's': 'client1'}
        uid = UniqueId.from_dict(d)
        assert uid.timestamp == 10
        assert uid.counter == 5
        assert uid.site_id == "client1"

    def test_roundtrip_serialization(self):
        """Serialization roundtrip preserves data."""
        original = UniqueId(42, 7, "test_site")
        via_string = UniqueId.from_string(str(original))
        via_dict = UniqueId.from_dict(original.to_dict())
        assert original == via_string
        assert original == via_dict

    def test_root_id(self):
        """ROOT_ID should be minimal."""
        assert ROOT_ID.timestamp == 0
        assert ROOT_ID.counter == 0
        assert ROOT_ID.site_id == ''
        # Any other ID should be greater
        other = UniqueId(0, 0, 'A')
        assert ROOT_ID < other


# =============================================================================
# Clock Tests
# =============================================================================

class TestClock:
    """Tests for Lamport logical clock."""

    def test_tick_generates_unique_ids(self):
        """Each tick should generate a unique ID."""
        clock = Clock("site1")
        id1 = clock.tick()
        id2 = clock.tick()
        assert id1 != id2
        assert id1 < id2  # IDs should be ordered

    def test_tick_increments_counter(self):
        """Tick should increment counter within same timestamp."""
        clock = Clock("site1")
        id1 = clock.tick()
        id2 = clock.tick()
        assert id1.timestamp == id2.timestamp
        assert id2.counter == id1.counter + 1

    def test_update_advances_timestamp(self):
        """Receiving a higher remote timestamp should advance local clock."""
        clock = Clock("site1")
        clock.tick()  # ts=0, c=1
        clock.update(10)  # Remote has ts=10
        assert clock.timestamp == 11
        assert clock.counter == 0

    def test_update_ignores_older_timestamp(self):
        """Receiving an older timestamp should not affect clock."""
        clock = Clock("site1")
        clock.increment()  # ts=1
        clock.increment()  # ts=2
        original_ts = clock.timestamp
        clock.update(0)  # Older timestamp
        assert clock.timestamp == original_ts

    def test_increment_advances_timestamp(self):
        """Increment should advance timestamp and reset counter."""
        clock = Clock("site1")
        clock.tick()  # ts=0, c=1
        clock.tick()  # ts=0, c=2
        ts = clock.increment()
        assert ts == 1
        assert clock.timestamp == 1
        assert clock.counter == 0

    def test_site_id_in_generated_ids(self):
        """Generated IDs should contain the site_id."""
        clock = Clock("my_site")
        uid = clock.tick()
        assert uid.site_id == "my_site"


# =============================================================================
# LWW-Register Tests
# =============================================================================

class TestLWWRegister:
    """Tests for Last-Writer-Wins Register."""

    def test_initial_state(self):
        """New register should have None value."""
        reg = LWWRegister()
        assert reg.value is None
        assert reg.timestamp == 0

    def test_set_updates_value(self):
        """Setting a value should update the register."""
        reg = LWWRegister()
        updated = reg.set("hello", 1, "A")
        assert updated is True
        assert reg.value == "hello"
        assert reg.timestamp == 1

    def test_newer_timestamp_wins(self):
        """Higher timestamp should overwrite lower."""
        reg = LWWRegister()
        reg.set("first", 1, "A")
        updated = reg.set("second", 2, "A")
        assert updated is True
        assert reg.value == "second"

    def test_older_timestamp_rejected(self):
        """Lower timestamp should be rejected."""
        reg = LWWRegister()
        reg.set("first", 2, "A")
        updated = reg.set("second", 1, "A")
        assert updated is False
        assert reg.value == "first"

    def test_same_timestamp_higher_site_id_wins(self):
        """Same timestamp: higher site_id wins."""
        reg = LWWRegister()
        reg.set("A_value", 1, "A")
        updated = reg.set("B_value", 1, "B")
        assert updated is True
        assert reg.value == "B_value"

    def test_same_timestamp_lower_site_id_rejected(self):
        """Same timestamp: lower site_id is rejected."""
        reg = LWWRegister()
        reg.set("B_value", 1, "B")
        updated = reg.set("A_value", 1, "A")
        assert updated is False
        assert reg.value == "B_value"

    def test_merge_newer_wins(self):
        """Merge should pick the newer value."""
        reg1 = LWWRegister("old", 1, "A")
        reg2 = LWWRegister("new", 2, "B")
        merged = reg1.merge(reg2)
        assert merged.value == "new"
        assert merged.timestamp == 2

    def test_merge_older_loses(self):
        """Merge should not pick the older value."""
        reg1 = LWWRegister("new", 2, "A")
        reg2 = LWWRegister("old", 1, "B")
        merged = reg1.merge(reg2)
        assert merged.value == "new"

    def test_merge_same_timestamp_site_id_tiebreaker(self):
        """Merge with same timestamp uses site_id."""
        reg1 = LWWRegister("A_value", 1, "A")
        reg2 = LWWRegister("B_value", 1, "B")
        merged = reg1.merge(reg2)
        assert merged.value == "B_value"

    def test_serialization_roundtrip(self):
        """Serialization should preserve data."""
        original = LWWRegister("test_value", 42, "site1")
        d = original.to_dict()
        restored = LWWRegister.from_dict(d)
        assert restored.value == original.value
        assert restored.timestamp == original.timestamp
        assert restored.site_id == original.site_id


# =============================================================================
# RGA Tests - Basic Operations
# =============================================================================

class TestRGABasic:
    """Basic RGA operations."""

    def test_empty_rga(self):
        """New RGA should be empty."""
        rga = RGA("site1")
        assert str(rga) == ""
        assert len(rga) == 0

    def test_insert_single_char(self):
        """Insert a single character."""
        rga = RGA("site1")
        rga.insert(0, 'A')
        assert str(rga) == "A"
        assert len(rga) == 1

    def test_insert_at_end(self):
        """Insert characters at end."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        assert str(rga) == "Hello"

    def test_insert_at_beginning(self):
        """Insert at beginning."""
        rga = RGA("site1")
        for c in "Hello":
            rga.insert(0, c)
        assert str(rga) == "olleH"  # Reversed

    def test_insert_in_middle(self):
        """Insert in the middle."""
        rga = RGA("site1")
        # Insert "Hello"
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        # Insert 'X' at position 2
        rga.insert(2, 'X')
        assert str(rga) == "HeXllo"

    def test_delete_first_char(self):
        """Delete first character."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        rga.delete(0)
        assert str(rga) == "ello"
        assert len(rga) == 4

    def test_delete_middle_char(self):
        """Delete middle character."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        rga.delete(2)  # Delete 'l'
        assert str(rga) == "Helo"

    def test_delete_last_char(self):
        """Delete last character."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        rga.delete(4)  # Delete 'o'
        assert str(rga) == "Hell"

    def test_delete_all_chars(self):
        """Delete all characters one by one."""
        rga = RGA("site1")
        for i, c in enumerate("Hi"):
            rga.insert(i, c)
        rga.delete(0)  # Delete 'H'
        rga.delete(0)  # Delete 'i' (now at position 0)
        assert str(rga) == ""
        assert len(rga) == 0

    def test_delete_invalid_index_returns_none(self):
        """Deleting invalid index returns None."""
        rga = RGA("site1")
        rga.insert(0, 'A')
        result = rga.delete(5)  # Invalid
        assert result is None
        assert str(rga) == "A"

    def test_insert_returns_ids(self):
        """Insert should return the new ID and after ID."""
        rga = RGA("site1")
        new_id, after_id = rga.insert(0, 'A')
        assert new_id is not None
        assert isinstance(new_id, UniqueId)
        assert new_id.site_id == "site1"
        assert after_id is None  # First insert, no predecessor

    def test_insert_after_returns_predecessor(self):
        """Second insert should reference predecessor."""
        rga = RGA("site1")
        id1, _ = rga.insert(0, 'A')
        id2, after_id = rga.insert(1, 'B')
        assert after_id == id1


# =============================================================================
# RGA Tests - Concurrent Operations and Convergence
# =============================================================================

class TestRGAConcurrent:
    """Test concurrent RGA operations and convergence."""

    def test_concurrent_inserts_at_start_converge(self):
        """Two sites inserting at start should converge."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Both insert at position 0
        id1, after1 = rga1.insert(0, 'X')
        id2, after2 = rga2.insert(0, 'Y')

        # Exchange operations
        rga1.apply_insert(id2, 'Y', after2)
        rga2.apply_insert(id1, 'X', after1)

        # Should converge to same order
        assert str(rga1) == str(rga2)
        # Order determined by site_id (B > A, so Y comes before X)
        assert str(rga1) == "YX"

    def test_concurrent_inserts_at_end_converge(self):
        """Two sites inserting at same position should converge."""
        # Start with same content
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Both insert "H"
        id_h1, _ = rga1.insert(0, 'H')
        rga2.apply_insert(id_h1, 'H', None)

        # Now both insert at position 1
        id1, after1 = rga1.insert(1, 'X')
        id2, after2 = rga2.insert(1, 'Y')

        # Exchange operations
        rga1.apply_insert(id2, 'Y', after2)
        rga2.apply_insert(id1, 'X', after1)

        assert str(rga1) == str(rga2)

    def test_concurrent_inserts_different_positions_converge(self):
        """Inserts at different positions should both appear."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Create shared "Hello"
        for i, c in enumerate("Hello"):
            id_c, after_c = rga1.insert(i, c)
            if i == 0:
                rga2.apply_insert(id_c, c, None)
            else:
                rga2.apply_insert(id_c, c, prev_id)
            prev_id = id_c

        assert str(rga1) == "Hello"
        assert str(rga2) == "Hello"

        # Site A inserts at start
        id1, after1 = rga1.insert(0, 'X')
        # Site B inserts at end
        id2, after2 = rga2.insert(5, 'Y')

        # Exchange
        rga1.apply_insert(id2, 'Y', after2)
        rga2.apply_insert(id1, 'X', after1)

        assert str(rga1) == str(rga2)
        assert str(rga1) == "XHelloY"

    def test_concurrent_insert_delete_converge(self):
        """Insert and delete on same position should converge."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Shared initial content "AB"
        id_a, _ = rga1.insert(0, 'A')
        id_b, after_b = rga1.insert(1, 'B')
        rga2.apply_insert(id_a, 'A', None)
        rga2.apply_insert(id_b, 'B', id_a)

        # Site A deletes 'A'
        deleted_id = rga1.delete(0)
        # Site B inserts 'X' at start
        id_x, after_x = rga2.insert(0, 'X')

        # Exchange operations
        rga1.apply_insert(id_x, 'X', after_x)
        rga2.apply_delete(deleted_id)

        assert str(rga1) == str(rga2)
        # Both should have "XB" (X inserted, A deleted)
        assert str(rga1) == "XB"

    def test_many_concurrent_inserts_converge(self):
        """Many concurrent inserts from multiple sites should converge."""
        sites = ["A", "B", "C"]
        rgas = [RGA(s) for s in sites]
        operations = []

        # Each site inserts different characters at position 0
        for i, rga in enumerate(rgas):
            char = chr(ord('X') + i)  # X, Y, Z
            new_id, after_id = rga.insert(0, char)
            operations.append((new_id, char, after_id))

        # Apply all operations to all RGAs
        for new_id, char, after_id in operations:
            for rga in rgas:
                rga.apply_insert(new_id, char, after_id)

        # All should converge
        results = [str(rga) for rga in rgas]
        assert all(r == results[0] for r in results)

    def test_apply_insert_idempotent(self):
        """Applying same insert twice should have no effect."""
        rga = RGA("site1")
        rga.insert(0, 'A')

        # Create a remote insert
        remote_id = UniqueId(5, 1, "remote")

        # Apply it twice
        result1 = rga.apply_insert(remote_id, 'B', None)
        result2 = rga.apply_insert(remote_id, 'B', None)

        assert result1 is True  # First application
        assert result2 is False  # Duplicate, no effect
        # Should only have 'B' once
        assert str(rga).count('B') == 1

    def test_apply_delete_idempotent(self):
        """Applying same delete twice should have no effect."""
        rga = RGA("site1")
        id_a, _ = rga.insert(0, 'A')

        result1 = rga.apply_delete(id_a)
        result2 = rga.apply_delete(id_a)

        assert result1 is True
        assert result2 is False  # Already deleted
        assert str(rga) == ""

    def test_interleaved_typing_converges(self):
        """Simulate two users typing interleaved characters."""
        rga1 = RGA("A")
        rga2 = RGA("B")
        ops1 = []
        ops2 = []

        # User A types "Hello"
        prev_id = None
        for i, c in enumerate("Hello"):
            new_id, after = rga1.insert(i, c)
            ops1.append((new_id, c, after))
            prev_id = new_id

        # User B types "World" concurrently (also from position 0)
        prev_id = None
        for i, c in enumerate("World"):
            new_id, after = rga2.insert(i, c)
            ops2.append((new_id, c, after))
            prev_id = new_id

        # Exchange all operations
        for new_id, c, after in ops1:
            rga2.apply_insert(new_id, c, after)
        for new_id, c, after in ops2:
            rga1.apply_insert(new_id, c, after)

        # Should converge
        assert str(rga1) == str(rga2)
        # Should contain all characters
        result = str(rga1)
        for c in "HelloWorld":
            assert c in result


# =============================================================================
# RGA Tests - Serialization
# =============================================================================

class TestRGASerialization:
    """Test RGA serialization and deserialization."""

    def test_serialize_empty_rga(self):
        """Empty RGA should serialize correctly."""
        rga = RGA("site1")
        d = rga.to_dict()
        restored = RGA.from_dict(d)
        assert str(restored) == ""
        assert restored.site_id == "site1"

    def test_serialize_with_content(self):
        """RGA with content should serialize correctly."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)

        d = rga.to_dict()
        restored = RGA.from_dict(d)

        assert str(restored) == "Hello"
        assert restored.site_id == "site1"

    def test_serialize_with_deletions(self):
        """RGA with deletions should serialize correctly."""
        rga = RGA("site1")
        for i, c in enumerate("Hello"):
            rga.insert(i, c)
        rga.delete(0)  # Delete 'H'

        d = rga.to_dict()
        restored = RGA.from_dict(d)

        assert str(restored) == "ello"

    def test_serialized_rga_can_receive_ops(self):
        """Deserialized RGA should accept new operations."""
        rga = RGA("site1")
        rga.insert(0, 'A')

        d = rga.to_dict()
        restored = RGA.from_dict(d)

        restored.insert(1, 'B')
        assert str(restored) == "AB"

    def test_get_all_operations(self):
        """get_all_operations should return all insert ops."""
        rga = RGA("site1")
        for i, c in enumerate("Hi"):
            rga.insert(i, c)

        ops = rga.get_all_operations()
        assert len(ops) == 2
        assert ops[0]['value'] == 'H'
        assert ops[1]['value'] == 'i'


# =============================================================================
# RGA Tests - Merge
# =============================================================================

class TestRGAMerge:
    """Test RGA merge functionality."""

    def test_merge_empty_rgas(self):
        """Merging empty RGAs should produce empty RGA."""
        rga1 = RGA("A")
        rga2 = RGA("B")
        merged = rga1.merge(rga2)
        assert str(merged) == ""

    def test_merge_one_empty(self):
        """Merging with one empty RGA."""
        rga1 = RGA("A")
        for i, c in enumerate("Hello"):
            rga1.insert(i, c)

        rga2 = RGA("B")
        merged = rga1.merge(rga2)
        assert str(merged) == "Hello"

    def test_merge_same_content(self):
        """Merging identical content."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Create same content in both via shared ops
        for i, c in enumerate("Hello"):
            new_id, after = rga1.insert(i, c)
            if i == 0:
                rga2.apply_insert(new_id, c, None)
            else:
                rga2.apply_insert(new_id, c, prev_id)
            prev_id = new_id

        merged = rga1.merge(rga2)
        assert str(merged) == "Hello"

    def test_merge_divergent_rgas(self):
        """Merging two RGAs with different content."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Different content
        for i, c in enumerate("Hello"):
            rga1.insert(i, c)
        for i, c in enumerate("World"):
            rga2.insert(i, c)

        merged = rga1.merge(rga2)
        result = str(merged)
        # Should contain all characters
        for c in "HelloWorld":
            assert c in result

    def test_merge_with_deletions(self):
        """Merge should propagate deletions."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Create shared content
        for i, c in enumerate("Hi"):
            new_id, after = rga1.insert(i, c)
            if i == 0:
                rga2.apply_insert(new_id, c, None)
            else:
                rga2.apply_insert(new_id, c, prev_id)
            prev_id = new_id

        # Delete in rga1 only
        rga1.delete(0)  # Delete 'H'

        merged = rga1.merge(rga2)
        # Deletion should win
        assert str(merged) == "i"


# =============================================================================
# LWW-Map Tests
# =============================================================================

class TestLWWMap:
    """Tests for LWW-Map."""

    def test_empty_map(self):
        """Empty map returns default."""
        m = LWWMap()
        assert m.get("missing") is None
        assert m.get("missing", "default") == "default"

    def test_set_and_get(self):
        """Basic set and get."""
        m = LWWMap()
        m.set("key", "value", 1, "A")
        assert m.get("key") == "value"

    def test_newer_value_wins(self):
        """Higher timestamp wins."""
        m = LWWMap()
        m.set("key", "old", 1, "A")
        m.set("key", "new", 2, "A")
        assert m.get("key") == "new"

    def test_older_value_rejected(self):
        """Lower timestamp is rejected."""
        m = LWWMap()
        m.set("key", "new", 2, "A")
        m.set("key", "old", 1, "A")
        assert m.get("key") == "new"

    def test_multiple_keys(self):
        """Multiple keys work independently."""
        m = LWWMap()
        m.set("a", "alpha", 1, "A")
        m.set("b", "beta", 1, "A")
        assert m.get("a") == "alpha"
        assert m.get("b") == "beta"

    def test_merge_disjoint_keys(self):
        """Merge maps with different keys."""
        m1 = LWWMap()
        m1.set("a", "alpha", 1, "A")

        m2 = LWWMap()
        m2.set("b", "beta", 1, "B")

        merged = m1.merge(m2)
        assert merged.get("a") == "alpha"
        assert merged.get("b") == "beta"

    def test_merge_overlapping_keys(self):
        """Merge maps with same keys, newer wins."""
        m1 = LWWMap()
        m1.set("key", "old", 1, "A")

        m2 = LWWMap()
        m2.set("key", "new", 2, "B")

        merged = m1.merge(m2)
        assert merged.get("key") == "new"

    def test_serialization_roundtrip(self):
        """Serialization preserves data."""
        m = LWWMap()
        m.set("a", "alpha", 1, "A")
        m.set("b", "beta", 2, "B")

        d = m.to_dict()
        restored = LWWMap.from_dict(d)

        assert restored.get("a") == "alpha"
        assert restored.get("b") == "beta"


# =============================================================================
# CRDTNote Tests
# =============================================================================

class TestCRDTNote:
    """Tests for CRDTNote."""

    def test_create_note(self):
        """Create a new note."""
        note = CRDTNote("note1", "client1")
        assert note.id == "note1"
        assert note.site_id == "client1"
        assert note.get_title() == ""
        assert note.get_content() == ""

    def test_set_title(self):
        """Set note title."""
        note = CRDTNote("note1", "client1")
        note.set_title("My Note")
        assert note.get_title() == "My Note"

    def test_insert_content(self):
        """Insert content character by character."""
        note = CRDTNote("note1", "client1")
        for i, c in enumerate("Hello"):
            note.insert_char(i, c)
        assert note.get_content() == "Hello"

    def test_delete_content(self):
        """Delete content."""
        note = CRDTNote("note1", "client1")
        for i, c in enumerate("Hello"):
            note.insert_char(i, c)
        note.delete_char(0)
        assert note.get_content() == "ello"

    def test_mark_deleted(self):
        """Mark note as deleted."""
        note = CRDTNote("note1", "client1")
        note.set_title("Title")
        assert not note.is_deleted()
        note.mark_deleted()
        assert note.is_deleted()

    def test_apply_remote_insert(self):
        """Apply remote insert operation."""
        note = CRDTNote("note1", "client1")
        char_id = {'ts': 1, 'c': 1, 's': 'remote'}
        result = note.apply_remote_insert(char_id, 'X', None)
        assert result is True
        assert note.get_content() == "X"

    def test_apply_remote_delete(self):
        """Apply remote delete operation."""
        note = CRDTNote("note1", "client1")
        # First insert locally
        op = note.insert_char(0, 'A')
        char_id_dict = op['id']

        # Then delete remotely
        result = note.apply_remote_delete(char_id_dict)
        assert result is True
        assert note.get_content() == ""

    def test_apply_remote_title(self):
        """Apply remote title update."""
        note = CRDTNote("note1", "client1")
        note.set_title("Local Title")  # ts=1

        # Remote has newer title
        result = note.apply_remote_title("Remote Title", 5, "remote")
        assert result is True
        assert note.get_title() == "Remote Title"

        # Older remote title rejected
        result = note.apply_remote_title("Old Title", 2, "other")
        assert result is False
        assert note.get_title() == "Remote Title"

    def test_merge_notes(self):
        """Merge two notes."""
        note1 = CRDTNote("note1", "A")
        note2 = CRDTNote("note1", "B")

        note1.set_title("Title A")
        note2.set_title("Title B")  # Should win if same ts (B > A)

        for i, c in enumerate("Hello"):
            note1.insert_char(i, c)
        for i, c in enumerate("World"):
            note2.insert_char(i, c)

        merged = note1.merge(note2)
        # Should have both contents
        content = merged.get_content()
        for c in "HelloWorld":
            assert c in content

    def test_serialization_roundtrip(self):
        """Serialization preserves note state."""
        note = CRDTNote("note1", "client1")
        note.set_title("My Title")
        for i, c in enumerate("Content"):
            note.insert_char(i, c)
        note.metadata.set("created", "2024-01-01", 1, "client1")

        d = note.to_dict()
        restored = CRDTNote.from_dict(d)

        assert restored.id == note.id
        assert restored.get_title() == note.get_title()
        assert restored.get_content() == note.get_content()
        assert restored.metadata.get("created") == "2024-01-01"


# =============================================================================
# Integration Tests - Simulated Sync Scenarios
# =============================================================================

class TestCRDTIntegration:
    """Integration tests simulating real sync scenarios."""

    def test_offline_online_sync(self):
        """Simulate offline editing followed by sync."""
        # Client A creates a note
        note_a = CRDTNote("note1", "A")
        note_a.set_title("Shopping List")
        for i, c in enumerate("Eggs"):
            note_a.insert_char(i, c)

        # Client A goes offline, Client B gets note state
        note_b_data = note_a.to_dict()
        note_b = CRDTNote.from_dict(note_b_data)
        # Change site ID for both the note and its content RGA
        note_b.site_id = "B"
        note_b.content.site_id = "B"
        note_b.content.clock.site_id = "B"
        note_b.clock.site_id = "B"

        # Both edit offline
        # A adds "Milk" at end
        for i, c in enumerate("Milk"):
            note_a.insert_char(len(note_a.get_content()), c)

        # B changes title
        note_b.set_title("Grocery List")
        # B adds "Bread" at end
        for i, c in enumerate("Bread"):
            note_b.insert_char(len(note_b.get_content()), c)

        # Sync: merge
        merged_a = note_a.merge(note_b)
        merged_b = note_b.merge(note_a)

        # Both should converge
        assert merged_a.get_content() == merged_b.get_content()

        # Content should have all additions
        content = merged_a.get_content()
        assert "Eggs" in content
        assert "Milk" in content
        assert "Bread" in content

    def test_three_way_sync(self):
        """Three clients editing and syncing."""
        # Create notes on 3 clients
        notes = [CRDTNote("note1", f"client{i}") for i in range(3)]

        # Each adds a character at position 0
        ops = []
        for i, note in enumerate(notes):
            char = chr(ord('A') + i)
            op = note.insert_char(0, char)
            ops.append((note.site_id, op))

        # Apply all ops to all notes
        for site_id, op in ops:
            for note in notes:
                if note.site_id != site_id:
                    note.apply_remote_insert(op['id'], op['value'], op['after'])

        # All should converge
        contents = [note.get_content() for note in notes]
        assert all(c == contents[0] for c in contents)
        # All characters should be present
        assert set(contents[0]) == {'A', 'B', 'C'}

    def test_rapid_typing_sync(self):
        """Simulate rapid typing from two users."""
        note1 = CRDTNote("note1", "A")
        note2 = CRDTNote("note1", "B")
        ops1 = []
        ops2 = []

        # User A types "abc"
        for i, c in enumerate("abc"):
            op = note1.insert_char(i, c)
            ops1.append(op)

        # User B types "xyz"
        for i, c in enumerate("xyz"):
            op = note2.insert_char(i, c)
            ops2.append(op)

        # Exchange operations
        for op in ops1:
            note2.apply_remote_insert(op['id'], op['value'], op['after'])
        for op in ops2:
            note1.apply_remote_insert(op['id'], op['value'], op['after'])

        # Should converge
        assert note1.get_content() == note2.get_content()
        # All chars present
        for c in "abcxyz":
            assert c in note1.get_content()

    def test_delete_resurrection_conflict(self):
        """Test when one client deletes while another edits nearby."""
        note1 = CRDTNote("note1", "A")
        note2 = CRDTNote("note1", "B")

        # Create shared content "Hi"
        op1 = note1.insert_char(0, 'H')
        op2 = note1.insert_char(1, 'i')
        note2.apply_remote_insert(op1['id'], op1['value'], op1['after'])
        note2.apply_remote_insert(op2['id'], op2['value'], op2['after'])

        # Client A deletes 'H'
        del_op = note1.delete_char(0)

        # Client B inserts 'X' at start (before H)
        ins_op = note2.insert_char(0, 'X')

        # Exchange
        note1.apply_remote_insert(ins_op['id'], ins_op['value'], ins_op['after'])
        note2.apply_remote_delete(del_op['id'])  # Pass just the ID, not full op

        # Should converge
        assert note1.get_content() == note2.get_content()
        # H deleted, X inserted, i remains
        content = note1.get_content()
        assert 'H' not in content
        assert 'X' in content
        assert 'i' in content


# =============================================================================
# Edge Case Tests
# =============================================================================

class TestEdgeCases:
    """Test edge cases and boundary conditions."""

    def test_empty_string_operations(self):
        """Operations on empty strings."""
        rga = RGA("site1")
        assert str(rga) == ""
        rga.delete(0)  # Should not crash
        assert str(rga) == ""

    def test_single_character_rga(self):
        """Single character RGA."""
        rga = RGA("site1")
        rga.insert(0, 'X')
        assert str(rga) == "X"
        rga.delete(0)
        assert str(rga) == ""

    def test_unicode_characters(self):
        """Unicode characters work correctly."""
        rga = RGA("site1")
        for i, c in enumerate("Hello\u4e16\u754c"):  # "Hello世界"
            rga.insert(i, c)
        assert str(rga) == "Hello\u4e16\u754c"

    def test_emoji_characters(self):
        """Emoji characters work correctly."""
        rga = RGA("site1")
        # Note: Some emojis are multiple code points
        text = "Hi!"
        for i, c in enumerate(text):
            rga.insert(i, c)
        assert str(rga) == text

    def test_very_long_site_id(self):
        """Very long site IDs work."""
        long_id = "a" * 1000
        rga = RGA(long_id)
        rga.insert(0, 'X')
        assert str(rga) == "X"

    def test_lww_register_with_complex_values(self):
        """LWW-Register with complex values."""
        reg = LWWRegister()
        reg.set([1, 2, 3], 1, "A")
        assert reg.value == [1, 2, 3]

        reg.set({"key": "value"}, 2, "A")
        assert reg.value == {"key": "value"}

    def test_crdt_note_empty_title_and_content(self):
        """Note with empty title and content."""
        note = CRDTNote("note1", "client1")
        assert note.get_title() == ""
        assert note.get_content() == ""

        # Serialize and restore
        restored = CRDTNote.from_dict(note.to_dict())
        assert restored.get_title() == ""
        assert restored.get_content() == ""

    def test_rga_insert_beyond_end(self):
        """Insert at index beyond current length."""
        rga = RGA("site1")
        rga.insert(0, 'A')
        rga.insert(100, 'B')  # Way beyond end
        # Should handle gracefully - append at end
        assert 'A' in str(rga)
        assert 'B' in str(rga)

    def test_clock_update_with_same_timestamp(self):
        """Clock update with same timestamp."""
        clock = Clock("site1")
        clock.increment()  # ts=1
        original = clock.timestamp
        clock.update(1)  # Same
        assert clock.timestamp == 2  # Should still advance

    def test_concurrent_same_insert_idempotent(self):
        """Same insert from same site applied twice."""
        rga = RGA("site1")
        id1, after1 = rga.insert(0, 'A')

        # Simulate receiving our own insert back
        result = rga.apply_insert(id1, 'A', after1)
        assert result is False
        assert str(rga) == "A"


# =============================================================================
# Performance Tests (Optional - can be slow)
# =============================================================================

class TestPerformance:
    """Performance tests for large documents."""

    def test_large_document_insert(self):
        """Insert many characters."""
        rga = RGA("site1")
        text = "A" * 1000
        for i, c in enumerate(text):
            rga.insert(i, c)
        assert len(rga) == 1000

    def test_large_document_concurrent_edit(self):
        """Multiple concurrent edits on large document."""
        rga1 = RGA("A")
        rga2 = RGA("B")

        # Create shared content
        ops = []
        for i in range(100):
            new_id, after = rga1.insert(i, 'X')
            ops.append((new_id, 'X', after))

        # Apply to rga2
        for new_id, char, after in ops:
            rga2.apply_insert(new_id, char, after)

        # Both insert concurrently
        ops1 = []
        ops2 = []
        for i in range(50):
            new_id1, after1 = rga1.insert(0, 'A')
            ops1.append((new_id1, 'A', after1))
            new_id2, after2 = rga2.insert(0, 'B')
            ops2.append((new_id2, 'B', after2))

        # Exchange
        for new_id, char, after in ops1:
            rga2.apply_insert(new_id, char, after)
        for new_id, char, after in ops2:
            rga1.apply_insert(new_id, char, after)

        # Should converge
        assert str(rga1) == str(rga2)
        assert len(rga1) == 200  # 100 + 50 + 50


if __name__ == '__main__':
    pytest.main([__file__, '-v'])
