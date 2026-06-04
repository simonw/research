Building on offline-first principles, this notes sync system enables robust note creation and editing without active internet connectivity, using IndexedDB and service workers on the client side. It employs operation-based sync and vector clocks for fine-grained conflict detection and resolution, and features a three-way character-level merge algorithm inspired by Apple Notes. Server-side logic is powered by Python Starlette and SQLite, with advanced CRDT constructs ensuring that concurrent edits from multiple clients merge seamlessly and converge correctly. A Datasette plugin extends API access and automates database table management, facilitating both testing and integration.  
Explore the [CRDT module](crdt.py) and [Datasette plugin](https://datasette.io/plugins/datasette-notes-sync) for key architectural components.

**Key Findings:**
- Achieves automatic, character-level merging for non-overlapping edits; overlaps prompt user intervention.
- CRDT implementation ensures commutativity, associativity, idempotency, and convergence of all note states.
- Test suite confirms sync, merge, offline persistence, and conflict-handling behavior across edge cases.
