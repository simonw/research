# Experimental report: compressed snapshot histories in SQLite

## Conclusion

The core idea works **extremely well for storage**. A compressor can act as an
implicit delta encoder when complete snapshots are placed next to each other.
In the main 1,000-revision workload, the historical text represented 20.4 MB of
raw snapshots. A single Zstandard-compressed JSON array occupied **80.3 KB**,
plus **10.7 KB** for the uncompressed JSON timestamp array.

The literal single-BLOB implementation has one serious scaling problem: every
edit decompresses and recompresses the entire logical history. Its final file is
tiny, but cumulative CPU and SQLite write amplification grow roughly
quadratically with the number of revisions.

The strongest design found by these prototypes is:

> Keep the current value as ordinary `TEXT`. Store previous values in sealed
> chunks, each of which is a Zstandard-compressed JSON array. Rewrite only the
> active chunk. Seal a chunk after roughly 64–128 revisions, or preferably when
> its uncompressed payload reaches about 2–3 MB.

For the tested 20 KB document, JSON chunks of 128 revisions used **109.9 KB** of
compressed historical text instead of 80.3 KB for one monolithic blob, but cut
total time for 1,000 edits from **26.8 seconds to 1.53 seconds** and reduced a
late edit from about **50 ms to 2.2 ms**. Chunks of 64 revisions reduced late
edits to about **1 ms** while using 154.9 KB.

## Prototype 1: the literal one-BLOB scheme

The prototype schema is:

```sql
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    current_text TEXT NOT NULL,
    current_timestamp INTEGER NOT NULL,
    history_blob BLOB NOT NULL,
    history_timestamps TEXT NOT NULL DEFAULT '[]',
    history_codec TEXT NOT NULL,
    history_format TEXT NOT NULL,
    revision_count INTEGER NOT NULL DEFAULT 0
);
```

On each edit, inside `BEGIN IMMEDIATE`:

1. Read the current text, its timestamp, and the history fields.
2. Decompress `history_blob`.
3. Append the old current text to the JSON array.
4. Recompress the array.
5. Append the old current timestamp to `history_timestamps`.
6. Install the new current text and timestamp in the same transaction.

The implementation does **not** parse and reconstruct the old JSON array during
an update. It owns the canonical serialization, so it can replace the final
`]` with `,<encoded old string>]`. This avoids allocating a Python list
containing every historical string, though compression still has to scan the
entire uncompressed payload.

The timestamp attached to a version is its “valid from” time. When that version
is replaced, its text and start timestamp move into history together.

## Prototype 2: sealed chunks

The chunked variant keeps current text in `documents` and stores history in:

```sql
CREATE TABLE history_chunks (
    document_id INTEGER NOT NULL,
    chunk_number INTEGER NOT NULL,
    first_revision INTEGER NOT NULL,
    revision_count INTEGER NOT NULL,
    history_blob BLOB NOT NULL,
    history_timestamps TEXT NOT NULL,
    PRIMARY KEY (document_id, chunk_number)
);
```

Only the final, partially filled chunk changes. Once full, a chunk is immutable.
This bounds the amount decompressed, copied, and recompressed by one edit. It
also makes a random historical read decompress only one chunk.

The prototype supports both canonical JSON arrays and a length-prefixed binary
format. JSON was retained as the primary recommendation: it was slightly
smaller after Zstandard compression in these workloads and is substantially
easier to inspect and migrate. The binary framing is somewhat faster to decode,
but did not change the architectural trade-off.

## Workload

The principal workload starts with a deterministic 20,000-byte pseudo-English
document. It applies 1,000 edits consisting mostly of short replacements,
insertions, and deletions, plus a paragraph-scale rewrite every 50 revisions.
Every edit is committed as its own SQLite transaction. The database benchmarks
used WAL mode, `synchronous=NORMAL`, and disabled automatic checkpointing so the
resulting WAL size could serve as an approximation of write amplification.

The 300-edit comparison reports medians from three trials. The focused
1,000-edit monolithic run was one trial because it is deliberately expensive;
the 64- and 128-revision JSON chunk results report medians from three trials.
The environment used Python 3.13.5 and SQLite 3.46.1. Absolute timings will vary
by machine; the growth curves and size ratios are the useful results.

## Storage result

Final compressed size as the monolithic history grows:

| Historical versions | Raw snapshots | JSON + Zstandard | JSON + zlib | Timestamp JSON |
|---:|---:|---:|---:|---:|
| 1 | 19.5 KB | 6.9 KB | 6.8 KB | 12 B |
| 10 | 195.5 KB | 7.1 KB | 8.7 KB | 111 B |
| 50 | 984.8 KB | 8.3 KB | 15.4 KB | 551 B |
| 100 | 1.9 MB | 10.1 KB | 23.5 KB | 1.1 KB |
| 300 | 5.9 MB | 25.6 KB | 56.6 KB | 3.2 KB |
| 1,000 | 20.4 MB | 80.3 KB | 176.4 KB | 10.7 KB |

At 1,000 revisions, the full-snapshot JSON history is therefore about **0.38%**
of the raw snapshot bytes. Compressing each snapshot independently does not
capture redundancy between revisions: it occupied **7.5 MB** in the same
workload.

## SQLite results at 300 edits

These are medians from three trials. “History payload” excludes the current-text
column and timestamp array. WAL bytes include individually committed edits with
no automatic checkpoint.

| Strategy | Total write time | Last-25 median edit | History payload | WAL | Compact DB | Random historical read |
|---|---:|---:|---:|---:|---:|---:|
| Current only, no history | 10.8 ms | 0.038 ms | 0 | 13.2 MB | 28 KB | — |
| Plain snapshot rows | 15.7 ms | 0.052 ms | 5.9 MB | 21.3 MB | 6.0 MB | 0.017 ms |
| Each row Zstandard-compressed | 57.9 ms | 0.183 ms | 2.1 MB | 17.7 MB | 2.4 MB | 0.073 ms |
| One JSON + Zstandard blob | 1,899.6 ms | 12.864 ms | 25.6 KB | 22.9 MB | 56 KB | 10.892 ms |
| One JSON + zlib blob | 5,076.9 ms | 31.946 ms | 56.6 KB | 32.9 MB | 88 KB | 12.151 ms |
| Framed Zstandard chunks of 32 | 147.2 ms | 0.515 ms | 82.4 KB | 16.1 MB | 124 KB | 0.244 ms |

The monolithic approach is close to ideal in final storage but not in write
behavior. Even though its final BLOB is only tens of kilobytes, it repeatedly
rewrites a growing value, and SQLite's WAL records the changed pages on every
commit.

## Focused results at 1,000 edits

| Strategy | Compressed history | Timestamp JSON | Total write time | Last-25 median edit | WAL | Compact DB | Random historical read |
|---|---:|---:|---:|---:|---:|---:|---:|
| Plain snapshot rows | 20.4 MB | 7.8 KB equivalent | 70.3 ms | 0.061 ms | 74.0 MB | 20.8 MB | 0.020 ms |
| Each row Zstandard-compressed | 7.5 MB | 7.8 KB equivalent | 218.2 ms | 0.241 ms | 61.2 MB | 8.0 MB | 0.080 ms |
| One JSON + Zstandard blob | 80.3 KB | 10.7 KB | 26.80 s | 49.904 ms | 136.7 MB | 120 KB | 38.238 ms |
| JSON + Zstandard chunks of 64 | 154.9 KB | 10.8 KB | 0.926 s | 0.957 ms | 57.2 MB | 224 KB | 1.626 ms |
| JSON + Zstandard chunks of 128 | 109.9 KB | 10.8 KB | 1.528 s | 2.196 ms | 58.4 MB | 164 KB | 3.070 ms |

A chunk size of 128 retains most of the monolithic compression ratio: its
compressed history is only about 30 KB larger. A chunk size of 64 is the better
choice when edit and historical-read latency matter more than another 45 KB.

A production implementation should use an **uncompressed-byte threshold**, not
only a revision count. For example, seal the active chunk when either:

- it contains 128 revisions, or
- its uncompressed JSON payload reaches roughly 2–3 MB.

This makes the bound work for both 2 KB notes and 200 KB documents.

## Sensitivity to the amount of change

A second experiment used deliberately high-entropy ASCII, which is difficult to
compress on its own. This isolates cross-version redundancy from the ordinary
compressibility of prose.

| Scenario, 1,000 × ~20 KB snapshots | Raw | Zstandard each row | One JSON + Zstandard | JSON chunks of 64 | JSON chunks of 128 |
|---|---:|---:|---:|---:|---:|
| Pseudo-English, small mixed edits | 20.4 MB | 7.5 MB | 80.3 KB | 154.9 KB | 109.9 KB |
| High entropy, 20 bytes replaced/edit | 19.1 MB | 15.8 MB | 45.8 KB | 290.6 KB | 160.4 KB |
| High entropy, 1% replaced/edit | 19.1 MB | 15.8 MB | 199.8 KB | 441.5 KB | 312.9 KB |
| High entropy, 10% replaced/edit | 19.1 MB | 15.8 MB | 1.6 MB | 1.8 MB | 1.7 MB |
| Independent high-entropy snapshots | 19.1 MB | 15.8 MB | 16.0 MB | 16.0 MB | 16.0 MB |

This confirms the intended mechanism: storage grows roughly with the amount of
new information introduced by edits. It is not relying on the source document
being English. There is no magic when successive versions are unrelated; the
scheme then approaches ordinary snapshot storage.

## Practical recommendations

### Use the one-BLOB design when

- histories are normally a few hundred revisions;
- edits are relatively infrequent;
- historical reads are rare and can decode the entire history once;
- minimal schema and astonishingly small files matter more than bounded write
  cost.

For that case, the original proposal is not merely plausible—it is very good.
Use Zstandard rather than zlib, keep the current text outside the blob, append
to canonical serialized JSON without parsing it, and update everything in one
transaction.

### Use chunked JSON when

- saving on every keystroke or autosave interval;
- documents can accumulate thousands of versions;
- random access to old versions matters;
- memory and write amplification need a hard bound.

A chunk table is a small increase in schema complexity and a dramatic
improvement in operational behavior.

### Additional production details

- Skip unchanged values unless a no-op save itself has semantic meaning.
- Store a codec and format version so rows can be migrated later.
- Keep timestamps per chunk rather than one document-wide array, so metadata
  has the same update boundary as text.
- Use `BEGIN IMMEDIATE`, or an optimistic `UPDATE ... WHERE revision_count = ?`,
  to prevent two writers from losing a revision.
- Validate that text count, timestamp count, and recorded revision count agree
  whenever a chunk is decoded.
- Cache a decoded chunk in the application while a history UI is open.
- Consider including timestamps inside the compressed payload if direct
  timestamp inspection is not valuable; the same-row transaction already
  makes either representation atomic.
- Do not treat the compressed BLOB as searchable SQL text. Maintain separate
  searchable current text or explicit derived indexes if history search is a
  requirement.

## Files in this prototype

- `text_history.py`: complete monolithic and chunked SQLite stores.
- `test_text_history.py`: round-trip, persistence, chunk-boundary, corruption,
  Unicode, and rollback tests.
- `benchmark.py`: SQLite storage, write, WAL, and read benchmarks.
- `sensitivity.py`: high-entropy and edit-size experiments.
- `make_demo_databases.py`: creates small databases for direct Datasette inspection.
- `demo-databases/`: generated monolithic and chunked examples.
- `benchmark-output/`: three-trial 300-edit benchmark and machine-readable data.
- `benchmark-output-1000/`: focused monolithic-versus-chunked 1,000-edit run.
- `benchmark-output-1000-chunks/`: chunk-size comparison.
- `benchmark-output-1000-jsonchunks-3/`: three-trial JSON chunk measurements.
- `sensitivity-output/`: compression sensitivity results.
