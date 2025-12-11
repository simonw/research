"""
Conflict-free Replicated Data Types (CRDTs) Implementation

This module implements CRDTs for an offline-first notes application:

1. LWW-Register (Last-Writer-Wins Register)
   - Simple values where the most recent write wins
   - Uses Lamport timestamps with site ID for total ordering

2. RGA (Replicated Growable Array)
   - Ordered sequence for text content
   - Each character has a unique ID (timestamp, site_id, counter)
   - Supports concurrent insertions at any position
   - Deletions create tombstones (not removed from structure)

3. LWW-Map
   - Map where each key is an LWW-Register
   - Useful for note metadata

Key properties of CRDTs:
- Commutativity: operations can be applied in any order
- Associativity: grouping of operations doesn't matter
- Idempotency: applying same operation twice has no effect
- Convergence: all replicas eventually reach same state
"""

from dataclasses import dataclass, field
from typing import Any, Optional, Dict, List, Tuple
from enum import Enum
import json


# =============================================================================
# Unique ID Generation
# =============================================================================

@dataclass(frozen=True)
class UniqueId:
    """
    Globally unique, totally ordered identifier.

    Ordering: (timestamp, counter, site_id)
    - timestamp: Lamport timestamp (logical clock)
    - counter: sequence number within timestamp
    - site_id: unique identifier for each client/replica (tie-breaker)
    """
    timestamp: int
    counter: int
    site_id: str

    def __lt__(self, other: 'UniqueId') -> bool:
        if self.timestamp != other.timestamp:
            return self.timestamp < other.timestamp
        if self.counter != other.counter:
            return self.counter < other.counter
        return self.site_id < other.site_id

    def __le__(self, other: 'UniqueId') -> bool:
        return self == other or self < other

    def __gt__(self, other: 'UniqueId') -> bool:
        return other < self

    def __ge__(self, other: 'UniqueId') -> bool:
        return self == other or self > other

    def __hash__(self) -> int:
        return hash((self.timestamp, self.counter, self.site_id))

    def __str__(self) -> str:
        return f"{self.timestamp}.{self.counter}@{self.site_id}"

    @classmethod
    def from_string(cls, s: str) -> 'UniqueId':
        ts_counter, site_id = s.split('@')
        ts, counter = ts_counter.split('.')
        return cls(int(ts), int(counter), site_id)

    def to_dict(self) -> dict:
        return {'ts': self.timestamp, 'c': self.counter, 's': self.site_id}

    @classmethod
    def from_dict(cls, d: dict) -> 'UniqueId':
        return cls(d['ts'], d['c'], d['s'])


# Special IDs for RGA boundaries
ROOT_ID = UniqueId(0, 0, '')


class Clock:
    """Lamport logical clock for generating unique IDs."""

    def __init__(self, site_id: str):
        self.site_id = site_id
        self.timestamp = 0
        self.counter = 0

    def tick(self) -> UniqueId:
        """Generate a new unique ID."""
        self.counter += 1
        return UniqueId(self.timestamp, self.counter, self.site_id)

    def update(self, remote_timestamp: int) -> None:
        """Update clock based on received timestamp (Lamport clock rule)."""
        if remote_timestamp >= self.timestamp:
            self.timestamp = remote_timestamp + 1
            self.counter = 0

    def increment(self) -> int:
        """Increment local timestamp and return it."""
        self.timestamp += 1
        self.counter = 0
        return self.timestamp


# =============================================================================
# LWW-Register (Last-Writer-Wins Register)
# =============================================================================

@dataclass
class LWWRegister:
    """
    Last-Writer-Wins Register CRDT.

    Stores a single value with a timestamp. When merging, the value with
    the highest timestamp wins. Ties are broken by site_id.
    """
    value: Any = None
    timestamp: int = 0
    site_id: str = ""

    def set(self, value: Any, timestamp: int, site_id: str) -> bool:
        """
        Set the value if the timestamp is newer.
        Returns True if the value was updated.
        """
        if self._is_newer(timestamp, site_id):
            self.value = value
            self.timestamp = timestamp
            self.site_id = site_id
            return True
        return False

    def _is_newer(self, timestamp: int, site_id: str) -> bool:
        """Check if the given timestamp/site_id is newer than current."""
        if timestamp > self.timestamp:
            return True
        if timestamp == self.timestamp and site_id > self.site_id:
            return True
        return False

    def merge(self, other: 'LWWRegister') -> 'LWWRegister':
        """Merge with another register, returning a new register."""
        # Check if other is newer than self
        if other.timestamp > self.timestamp:
            return LWWRegister(other.value, other.timestamp, other.site_id)
        if other.timestamp == self.timestamp and other.site_id > self.site_id:
            return LWWRegister(other.value, other.timestamp, other.site_id)
        return LWWRegister(self.value, self.timestamp, self.site_id)

    def to_dict(self) -> dict:
        return {'v': self.value, 'ts': self.timestamp, 's': self.site_id}

    @classmethod
    def from_dict(cls, d: dict) -> 'LWWRegister':
        return cls(d['v'], d['ts'], d['s'])


# =============================================================================
# RGA (Replicated Growable Array) for Text
# =============================================================================

@dataclass
class RGANode:
    """
    A node in the RGA representing a single character.
    """
    id: UniqueId
    value: str
    deleted: bool = False

    def to_dict(self) -> dict:
        return {'id': self.id.to_dict(), 'v': self.value, 'd': self.deleted}

    @classmethod
    def from_dict(cls, d: dict) -> 'RGANode':
        return cls(UniqueId.from_dict(d['id']), d['v'], d.get('d', False))


class RGA:
    """
    Replicated Growable Array - a CRDT for ordered sequences (text).

    This implementation builds a tree structure based on predecessor references
    and linearizes it using topological sort. Siblings (nodes with same predecessor)
    are ordered by ID (descending - higher ID comes first).

    Properties:
    - Each character has a unique ID
    - Insert specifies position via the ID of the predecessor
    - Concurrent inserts at same position are ordered by ID (descending)
    - Deletes mark characters as tombstones
    - All operations are commutative and idempotent
    """

    def __init__(self, site_id: str = ""):
        self.site_id = site_id
        self.clock = Clock(site_id)
        # Map from ID to node for quick lookup
        self._index: Dict[UniqueId, RGANode] = {}
        # Map from ID to predecessor ID
        self._predecessors: Dict[UniqueId, Optional[UniqueId]] = {}
        # Cached linear order (invalidated on insert)
        self._cached_order: Optional[List[RGANode]] = None

    def _get_nodes(self) -> List[RGANode]:
        """Get nodes in linearized order."""
        if self._cached_order is not None:
            return self._cached_order

        if not self._index:
            self._cached_order = []
            return []

        # Build children map: parent_id -> list of children sorted by ID (desc)
        children: Dict[Optional[UniqueId], List[UniqueId]] = {}
        for node_id in self._index:
            parent = self._predecessors.get(node_id)
            if parent not in children:
                children[parent] = []
            children[parent].append(node_id)

        # Sort children by ID descending (higher ID first)
        for parent in children:
            children[parent].sort(reverse=True)

        # Iterative DFS to linearize (avoids stack overflow for large documents)
        result = []
        # Stack contains: (node_id, child_index)
        stack = [(None, 0)]

        while stack:
            node_id, child_idx = stack.pop()

            if child_idx == 0 and node_id is not None and node_id in self._index:
                # First visit to this node - add to result
                result.append(self._index[node_id])

            node_children = children.get(node_id, [])

            if child_idx < len(node_children):
                # Push back current node with next child index
                stack.append((node_id, child_idx + 1))
                # Push child to visit
                stack.append((node_children[child_idx], 0))

        self._cached_order = result
        return result

    @property
    def _nodes(self) -> List[RGANode]:
        """Property for backward compatibility."""
        return self._get_nodes()

    def _rebuild_positions(self) -> None:
        """Invalidate cached order."""
        self._cached_order = None

    @property
    def _positions(self) -> Dict[UniqueId, int]:
        """Get position map (computed on demand)."""
        nodes = self._get_nodes()
        return {node.id: i for i, node in enumerate(nodes)}

    def insert(self, index: int, char: str) -> Tuple[UniqueId, Optional[UniqueId]]:
        """
        Insert a character at the given visible index.
        Returns (new_id, after_id) for the operation.
        """
        nodes = self._get_nodes()

        # Find the predecessor's ID
        if index <= 0:
            after_id = None
        else:
            # Find the ID of the character at visible index (index-1)
            visible_index = 0
            after_id = None
            for node in nodes:
                if not node.deleted:
                    if visible_index == index - 1:
                        after_id = node.id
                        break
                    visible_index += 1

            if after_id is None and index > 0:
                # Index beyond end - use last visible character
                for node in reversed(nodes):
                    if not node.deleted:
                        after_id = node.id
                        break

        new_id = self.clock.tick()
        self._apply_insert(new_id, char, after_id)
        return new_id, after_id

    def delete(self, index: int) -> Optional[UniqueId]:
        """
        Delete the character at the given visible index.
        Returns the ID of the deleted character, or None if invalid.
        """
        nodes = self._get_nodes()
        visible_index = 0
        for node in nodes:
            if not node.deleted:
                if visible_index == index:
                    node.deleted = True
                    self._cached_order = None  # Invalidate cache
                    return node.id
                visible_index += 1
        return None

    def apply_insert(self, char_id: UniqueId, char: str, after_id: Optional[UniqueId]) -> bool:
        """
        Apply an insert operation from a remote site.
        Returns True if the operation was applied (not duplicate).
        """
        if char_id in self._index:
            return False  # Already have this node (idempotent)

        self.clock.update(char_id.timestamp)
        self._apply_insert(char_id, char, after_id)
        return True

    def apply_delete(self, char_id: UniqueId) -> bool:
        """
        Apply a delete operation from a remote site.
        Returns True if the operation was applied.
        """
        if char_id in self._index:
            node = self._index[char_id]
            if not node.deleted:
                node.deleted = True
                self._cached_order = None  # Invalidate cache
                return True
        return False

    def _apply_insert(self, new_id: UniqueId, char: str, after_id: Optional[UniqueId]) -> None:
        """Internal method to apply an insert."""
        node = RGANode(id=new_id, value=char, deleted=False)

        # Store node and predecessor
        self._predecessors[new_id] = after_id
        self._index[new_id] = node

        # Invalidate cached order
        self._cached_order = None

    def to_string(self) -> str:
        """Convert the RGA to a string (visible characters only)."""
        return ''.join(node.value for node in self._get_nodes() if not node.deleted)

    def __len__(self) -> int:
        """Return the number of visible characters."""
        return sum(1 for node in self._get_nodes() if not node.deleted)

    def __str__(self) -> str:
        return self.to_string()

    def to_dict(self) -> dict:
        """Serialize the RGA to a dictionary."""
        nodes = self._get_nodes()
        return {
            'site_id': self.site_id,
            'clock_ts': self.clock.timestamp,
            'clock_c': self.clock.counter,
            'nodes': [node.to_dict() for node in nodes],
            'preds': {str(k): (str(v) if v else None)
                      for k, v in self._predecessors.items()}
        }

    @classmethod
    def from_dict(cls, d: dict) -> 'RGA':
        """Deserialize an RGA from a dictionary."""
        rga = cls(d['site_id'])
        rga.clock.timestamp = d['clock_ts']
        rga.clock.counter = d['clock_c']

        for k, v in d.get('preds', {}).items():
            key = UniqueId.from_string(k)
            value = UniqueId.from_string(v) if v else None
            rga._predecessors[key] = value

        for node_dict in d['nodes']:
            node = RGANode.from_dict(node_dict)
            rga._index[node.id] = node

        rga._cached_order = None  # Force rebuild on next access
        return rga

    def merge(self, other: 'RGA') -> 'RGA':
        """
        Merge another RGA into this one.
        Returns a new RGA with all operations from both.
        """
        merged = RGA(f"{self.site_id}+{other.site_id}")
        merged.clock.timestamp = max(self.clock.timestamp, other.clock.timestamp)

        # Collect all unique nodes
        all_ids = set(self._index.keys()) | set(other._index.keys())

        for node_id in all_ids:
            self_node = self._index.get(node_id)
            other_node = other._index.get(node_id)

            if self_node and other_node:
                # Both have it - merge deleted status (deleted wins)
                deleted = self_node.deleted or other_node.deleted
                char = self_node.value
                after_id = self._predecessors.get(node_id) or \
                           other._predecessors.get(node_id)
            elif self_node:
                deleted = self_node.deleted
                char = self_node.value
                after_id = self._predecessors.get(node_id)
            else:
                deleted = other_node.deleted
                char = other_node.value
                after_id = other._predecessors.get(node_id)

            # Create node in merged RGA
            node = RGANode(id=node_id, value=char, deleted=deleted)
            merged._predecessors[node_id] = after_id
            merged._index[node_id] = node

        merged._cached_order = None  # Force rebuild
        return merged

    def get_all_operations(self) -> List[dict]:
        """Get all insert operations for syncing."""
        ops = []
        for node in self._get_nodes():
            after_id = self._predecessors.get(node.id)
            ops.append({
                'type': 'insert',
                'id': node.id.to_dict(),
                'value': node.value,
                'after': after_id.to_dict() if after_id else None,
                'deleted': node.deleted
            })
        return ops


# =============================================================================
# LWW-Map (Map with LWW-Register values)
# =============================================================================

class LWWMap:
    """
    A map where each key is associated with an LWW-Register.
    """

    def __init__(self):
        self.registers: Dict[str, LWWRegister] = {}

    def set(self, key: str, value: Any, timestamp: int, site_id: str) -> bool:
        """Set a key's value."""
        if key not in self.registers:
            self.registers[key] = LWWRegister()
        return self.registers[key].set(value, timestamp, site_id)

    def get(self, key: str, default: Any = None) -> Any:
        """Get a key's value."""
        if key in self.registers:
            return self.registers[key].value
        return default

    def merge(self, other: 'LWWMap') -> 'LWWMap':
        """Merge with another map."""
        merged = LWWMap()
        all_keys = set(self.registers.keys()) | set(other.registers.keys())

        for key in all_keys:
            if key in self.registers and key in other.registers:
                merged.registers[key] = self.registers[key].merge(other.registers[key])
            elif key in self.registers:
                merged.registers[key] = LWWRegister(
                    self.registers[key].value,
                    self.registers[key].timestamp,
                    self.registers[key].site_id
                )
            else:
                merged.registers[key] = LWWRegister(
                    other.registers[key].value,
                    other.registers[key].timestamp,
                    other.registers[key].site_id
                )

        return merged

    def to_dict(self) -> dict:
        return {key: reg.to_dict() for key, reg in self.registers.items()}

    @classmethod
    def from_dict(cls, d: dict) -> 'LWWMap':
        lww_map = cls()
        for key, reg_dict in d.items():
            lww_map.registers[key] = LWWRegister.from_dict(reg_dict)
        return lww_map


# =============================================================================
# CRDT Note - Combines RGA (content) with LWW-Register (title)
# =============================================================================

class CRDTNote:
    """
    A note represented as CRDTs.
    - id: unique note identifier
    - title: LWW-Register for title
    - content: RGA for character-by-character editing
    - metadata: LWW-Map for other fields (created_at, etc.)
    - deleted: LWW-Register for soft delete
    """

    def __init__(self, note_id: str, site_id: str):
        self.id = note_id
        self.site_id = site_id
        self.clock = Clock(site_id)
        self.title = LWWRegister()
        self.content = RGA(site_id)
        self.metadata = LWWMap()
        self.deleted = LWWRegister(value=False, timestamp=0, site_id="")

    def set_title(self, title: str) -> int:
        """Set the note title. Returns the timestamp used."""
        ts = self.clock.increment()
        self.title.set(title, ts, self.site_id)
        return ts

    def get_title(self) -> str:
        """Get the note title."""
        return self.title.value or ""

    def insert_char(self, index: int, char: str) -> dict:
        """Insert a single character at index. Returns the operation."""
        new_id, after_id = self.content.insert(index, char)
        return {
            'op': 'insert',
            'note_id': self.id,
            'id': new_id.to_dict(),
            'value': char,
            'after': after_id.to_dict() if after_id else None
        }

    def delete_char(self, index: int) -> Optional[dict]:
        """Delete a single character at index. Returns the operation."""
        char_id = self.content.delete(index)
        if char_id:
            return {
                'op': 'delete',
                'note_id': self.id,
                'id': char_id.to_dict()
            }
        return None

    def get_content(self) -> str:
        """Get the note content."""
        return self.content.to_string()

    def apply_remote_insert(self, char_id_dict: dict, char: str,
                            after_id_dict: Optional[dict]) -> bool:
        """Apply a remote insert operation."""
        char_id = UniqueId.from_dict(char_id_dict)
        after_id = UniqueId.from_dict(after_id_dict) if after_id_dict else None
        return self.content.apply_insert(char_id, char, after_id)

    def apply_remote_delete(self, char_id_dict: dict) -> bool:
        """Apply a remote delete operation."""
        char_id = UniqueId.from_dict(char_id_dict)
        return self.content.apply_delete(char_id)

    def apply_remote_title(self, title: str, timestamp: int, site_id: str) -> bool:
        """Apply a remote title update."""
        return self.title.set(title, timestamp, site_id)

    def mark_deleted(self) -> int:
        """Mark the note as deleted. Returns the timestamp used."""
        ts = self.clock.increment()
        self.deleted.set(True, ts, self.site_id)
        return ts

    def is_deleted(self) -> bool:
        """Check if the note is deleted."""
        return self.deleted.value == True

    def merge(self, other: 'CRDTNote') -> 'CRDTNote':
        """Merge with another note."""
        merged = CRDTNote(self.id, f"{self.site_id}+{other.site_id}")
        merged.clock.timestamp = max(self.clock.timestamp, other.clock.timestamp)
        merged.title = self.title.merge(other.title)
        merged.content = self.content.merge(other.content)
        merged.metadata = self.metadata.merge(other.metadata)
        merged.deleted = self.deleted.merge(other.deleted)
        return merged

    def to_dict(self) -> dict:
        return {
            'id': self.id,
            'site_id': self.site_id,
            'clock_ts': self.clock.timestamp,
            'title': self.title.to_dict(),
            'content': self.content.to_dict(),
            'metadata': self.metadata.to_dict(),
            'deleted': self.deleted.to_dict()
        }

    @classmethod
    def from_dict(cls, d: dict) -> 'CRDTNote':
        note = cls(d['id'], d['site_id'])
        note.clock.timestamp = d['clock_ts']
        note.title = LWWRegister.from_dict(d['title'])
        note.content = RGA.from_dict(d['content'])
        note.metadata = LWWMap.from_dict(d['metadata'])
        note.deleted = LWWRegister.from_dict(d['deleted'])
        return note


# =============================================================================
# Quick Test
# =============================================================================

if __name__ == '__main__':
    print("=== Testing RGA Basic Operations ===")
    rga = RGA("site1")

    # Insert "Hello"
    for i, c in enumerate("Hello"):
        rga.insert(i, c)
    print(f"After inserting 'Hello': '{rga}'")
    assert str(rga) == "Hello", f"Expected 'Hello', got '{rga}'"

    # Insert at beginning
    rga.insert(0, 'X')
    print(f"After insert(0, 'X'): '{rga}'")
    assert str(rga) == "XHello", f"Expected 'XHello', got '{rga}'"

    # Insert in middle
    rga.insert(3, 'Y')
    print(f"After insert(3, 'Y'): '{rga}'")
    assert str(rga) == "XHeYllo", f"Expected 'XHeYllo', got '{rga}'"

    # Delete
    rga.delete(0)
    print(f"After delete(0): '{rga}'")
    assert str(rga) == "HeYllo", f"Expected 'HeYllo', got '{rga}'"

    print("\n=== Testing Concurrent Inserts ===")
    # Two sites, same initial state
    rga1 = RGA("A")
    rga2 = RGA("B")

    # Both insert "X" at position 0
    id1, after1 = rga1.insert(0, 'X')
    id2, after2 = rga2.insert(0, 'Y')

    print(f"RGA1 (site A) after insert X: '{rga1}'")
    print(f"RGA2 (site B) after insert Y: '{rga2}'")

    # Exchange operations
    rga1.apply_insert(id2, 'Y', after2)
    rga2.apply_insert(id1, 'X', after1)

    print(f"RGA1 after receiving Y: '{rga1}'")
    print(f"RGA2 after receiving X: '{rga2}'")
    print(f"Converged: {str(rga1) == str(rga2)}")

    # Both should have same order (determined by site_id since same timestamp)
    assert str(rga1) == str(rga2), f"Not converged: '{rga1}' != '{rga2}'"
    print(f"Final order: '{rga1}'")

    print("\n=== Testing CRDT Note ===")
    note = CRDTNote("note1", "client1")
    note.set_title("My Note")

    # Insert content character by character
    for i, c in enumerate("Hello World"):
        note.insert_char(i, c)

    print(f"Title: {note.get_title()}")
    print(f"Content: {note.get_content()}")

    print("\n=== All basic tests passed! ===")
