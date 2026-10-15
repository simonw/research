//! Dictionary rows read directly from the `rdf_term` table b-tree (decode cache misses).
//!
//! `rdf_term` is a rowid table: `id INTEGER PRIMARY KEY` is the rowid (verified by `roots`).
//! Its b-tree has interior pages (0x05) whose cells hold a left child and a rowid key (every
//! rowid of the left child is `<=` that key; the right-most child holds the larger ones), and
//! leaf pages (0x0D) whose cells hold the payload size, the rowid and the record. A record is
//! `(NULL, tag, lex, dt, lang, h)`: the rowid alias is stored as NULL.
//!
//! Lookups run for ascending row numbers (`Dictionary::decode` sorts and deduplicates its
//! misses). The root-to-leaf path of the last lookup is kept with the rowid interval of each
//! level, so the next row climbs only as far as needed (usually not at all, or one level).
//! The reader keeps that path between calls for as long as the snapshot is unchanged, which
//! helps single-id misses (`decode_one`) most.
//! Rowids are dense, so each page is searched by interpolation between its first and last
//! key, then galloping and bisection: a few probes per page instead of a binary search.
//!
//! Only what the SQL path would read identically is served here. A row the b-tree does not
//! hold, a payload that spills to overflow pages (a lexical form longer than about the page
//! size), or a record of any other shape (a non-integer tag or datatype, a BLOB, invalid UTF-8,
//! missing columns) is left to the pooled SQL statement, which returns the same value or
//! reports the same error. A page that is not a table b-tree page, or any other inconsistency,
//! ends the direct reads for the call (a counted fallback); its remaining rows go through SQL.
//! Text is served only from UTF-8 databases. The preconditions are those of every cursor
//! (`BtreeReader::snapshot`).
use std::sync::Arc;

use sq_core::Result;

use super::Fallback;
use super::format::{HEADER_LEN, MAX_DEPTH, be16, be32, varint};
use super::reader::{BtreeReader, ReaderMode, Snapshot};

/// Rows read between two interrupt polls.
const POLL_ROWS: usize = 4096;

const fn corrupt(what: &'static str) -> Fallback {
    Fallback::Corrupt(what)
}

/// The `tag, lex, dt, lang` columns of one `rdf_term` row, borrowed from its page.
pub(crate) struct TermRow<'p> {
    pub tag: i64,
    pub lex: &'p str,
    pub dt: Option<i64>,
    pub lang: Option<&'p str>,
}

/// One page of a rowid table b-tree.
struct TablePage {
    page: Arc<[u8]>,
    leaf: bool,
    cells: usize,
    /// Offset of the cell pointer array.
    ptrs: usize,
    /// Start of the cell content area (after the cell pointer array).
    content: usize,
    /// Keys of the first and the last cell (0 on an empty page).
    first: i64,
    last: i64,
    /// Right-most child (interior pages).
    right: u32,
    /// Usable size of the page.
    usable: usize,
}

impl TablePage {
    fn load(reader: &BtreeReader, snap: &Snapshot, pgno: u32) -> Result<TablePage, Fallback> {
        let page = reader.page(snap, pgno)?;
        let usable = snap.geom.usable;
        let b = page.get(..usable).ok_or(corrupt("page is shorter than the page size"))?;
        let hdr = if pgno == 1 { HEADER_LEN } else { 0 };
        let leaf = match b.get(hdr).copied() {
            Some(0x0D) => true,
            Some(0x05) => false,
            Some(0x02 | 0x0A) => return Err(Fallback::Unsupported("index b-tree page where a table b-tree was expected")),
            _ => return Err(corrupt("not a table b-tree page")),
        };
        let cells = usize::from(be16(b, hdr + 3).ok_or(corrupt("cell count"))?);
        let ptrs = hdr + if leaf { 8 } else { 12 };
        if ptrs + 2 * cells > usable {
            return Err(corrupt("cell pointer array overflows the page"));
        }
        let right = if leaf { 0 } else { be32(b, hdr + 8).ok_or(corrupt("right-most pointer"))? };
        if !leaf && right == 0 {
            return Err(corrupt("interior page without a right-most child"));
        }
        let mut p = TablePage { page, leaf, cells, ptrs, content: ptrs + 2 * cells, first: 0, last: 0, right, usable };
        if cells > 0 {
            p.first = p.key(0)?;
            p.last = p.key(cells - 1)?;
        }
        Ok(p)
    }

    fn bytes(&self) -> &[u8] {
        &self.page[..self.usable]
    }

    /// Offset of cell `i`, inside the cell content area.
    #[inline]
    fn cell(&self, i: usize) -> Result<usize, Fallback> {
        if i >= self.cells {
            return Err(corrupt("cell index out of range"));
        }
        let off = usize::from(be16(self.bytes(), self.ptrs + 2 * i).ok_or(corrupt("cell index out of range"))?);
        if off < self.content || off >= self.usable {
            return Err(corrupt("cell pointer outside the cell content area"));
        }
        Ok(off)
    }

    /// Rowid key of cell `i`: after the left child pointer (interior) or the payload size (leaf).
    #[inline]
    fn key(&self, i: usize) -> Result<i64, Fallback> {
        let b = self.bytes();
        let at = self.cell(i)?;
        let at = if self.leaf {
            match b.get(at) {
                Some(&x) if x < 0x80 => at + 1,
                _ => at + varint(b, at)?.1,
            }
        } else {
            at + 4
        };
        Ok(short_varint(b, at)? as i64)
    }

    /// Cell index where linear interpolation between the first and the last key puts `row`,
    /// for `first < row <= last` (so the page has at least 2 cells): in `1..cells`.
    #[inline]
    fn interpolate(&self, row: i64) -> usize {
        let span = (i128::from(self.last) - i128::from(self.first)).max(1);
        let guess = (i128::from(row) - i128::from(self.first)) * (self.cells as i128 - 1) / span;
        usize::try_from(guess).unwrap_or(0).clamp(1, self.cells - 1)
    }

    /// The leaf cell whose rowid is `row`, if there is one: the interpolated cell first
    /// (rowids are usually dense), otherwise a search.
    fn find(&self, row: i64) -> Result<Option<usize>, Fallback> {
        if self.cells == 0 || row < self.first || row > self.last {
            return Ok(None);
        }
        if row == self.first {
            return Ok(Some(0));
        }
        let g = self.interpolate(row);
        if self.key(g)? == row {
            return Ok(Some(g));
        }
        let i = self.search(row)?;
        Ok((i < self.cells && self.key(i)? == row).then_some(i))
    }

    /// First cell whose key is `>= row` (`cells` when there is none). Rowids are usually
    /// dense, so the search starts where linear interpolation between the first and the last
    /// key puts `row`, gallops from there and finishes with a binary search: two or three
    /// probes instead of a dozen on a dense page.
    fn search(&self, row: i64) -> Result<usize, Fallback> {
        let n = self.cells;
        if n == 0 || row <= self.first {
            return Ok(0);
        }
        if row > self.last {
            return Ok(n);
        }
        // Here key(0) < row <= key(n - 1), so n >= 2 and the answer lies in 1..n.
        let g = self.interpolate(row);
        // Gallop to a bracket 1 <= lo <= hi <= n - 1 with key(lo - 1) < row <= key(hi), then
        // bisect.
        let (mut lo, mut hi);
        if self.key(g)? >= row {
            hi = g;
            let mut step = 1;
            loop {
                let p = hi.saturating_sub(step);
                if p == 0 {
                    lo = 1;
                    break;
                }
                if self.key(p)? < row {
                    lo = p + 1;
                    break;
                }
                hi = p;
                step *= 2;
            }
        } else {
            lo = g + 1;
            let mut step = 1;
            loop {
                let p = (g + step).min(n - 1);
                if self.key(p)? >= row {
                    hi = p;
                    break;
                }
                lo = p + 1;
                step *= 2;
            }
        }
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.key(mid)? < row {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        Ok(lo)
    }

    /// Left child of interior cell `i`, or the right-most child when `i` is the cell count.
    fn child(&self, i: usize) -> Result<u32, Fallback> {
        if i == self.cells {
            return Ok(self.right);
        }
        let pgno = be32(self.bytes(), self.cell(i)?).ok_or(corrupt("left child pointer"))?;
        if pgno == 0 {
            return Err(corrupt("child pointer to page 0"));
        }
        Ok(pgno)
    }

    /// The record of leaf cell `i`; `None` when it spills to overflow pages.
    fn record(&self, i: usize) -> Result<Option<&[u8]>, Fallback> {
        let b = self.bytes();
        let at = self.cell(i)?;
        let (size, n1) = varint(b, at)?;
        let (_, n2) = varint(b, at + n1)?;
        // A table leaf keeps payloads of up to usable - 35 bytes on the page.
        if size > (self.usable - 35) as u64 {
            return Ok(None);
        }
        let start = at + n1 + n2;
        b.get(start..start + size as usize).map(Some).ok_or(corrupt("cell payload overflows the page"))
    }
}

/// A varint of up to 3 bytes (rowids below 2^21) decoded inline; longer ones by `varint`.
#[inline(always)]
fn short_varint(b: &[u8], at: usize) -> Result<u64, Fallback> {
    match b.get(at..) {
        Some(&[x, ..]) if x < 0x80 => Ok(u64::from(x)),
        Some(&[x, y, ..]) if y < 0x80 => Ok((u64::from(x & 0x7f) << 7) | u64::from(y)),
        Some(&[x, y, z, ..]) if z < 0x80 => Ok((u64::from(x & 0x7f) << 14) | (u64::from(y & 0x7f) << 7) | u64::from(z)),
        _ => Ok(varint(b, at)?.0),
    }
}

/// Body width of a record column of serial type `st`; `None` for the reserved types 10, 11.
fn width(st: u64) -> Option<usize> {
    Some(match st {
        0 | 8 | 9 => 0,
        1..=4 => st as usize,
        5 => 6,
        6 | 7 => 8,
        10 | 11 => return None,
        n => usize::try_from((n - 12) / 2).ok()?,
    })
}

/// Integer value of a column of serial type `st` (1–6, 8, 9) whose body bytes are `v`.
fn int_value(st: u64, v: &[u8]) -> Option<i64> {
    match st {
        1..=6 => {
            let sign = if v.first()? & 0x80 != 0 { -1i64 } else { 0 };
            Some(v.iter().fold(sign, |acc, &b| (acc << 8) | i64::from(b)))
        }
        8 => Some(0),
        9 => Some(1),
        _ => None,
    }
}

/// The `tag, lex, dt, lang` columns of an `rdf_term` record, or `None` when the record has any
/// other shape (the SQL path then reads the row).
fn term_row(rec: &[u8]) -> Option<TermRow<'_>> {
    let (hlen, mut at) = varint(rec, 0).ok()?;
    let hlen = usize::try_from(hlen).ok()?;
    let header = rec.get(..hlen)?;
    let mut body = hlen;
    let mut column = || -> Option<(u64, &[u8])> {
        let (st, n) = varint(header, at).ok().filter(|_| at < hlen)?;
        at += n;
        let w = width(st)?;
        let v = rec.get(body..body + w)?;
        body += w;
        Some((st, v))
    };
    let is_text = |st: u64| st >= 13 && st % 2 == 1;
    // `id`, the rowid alias, is stored as NULL.
    if column()?.0 != 0 {
        return None;
    }
    let (st, v) = column()?;
    let tag = int_value(st, v)?;
    let (st, v) = column()?;
    let lex = std::str::from_utf8(v).ok().filter(|_| is_text(st))?;
    let (st, v) = column()?;
    let dt = if st == 0 { None } else { Some(int_value(st, v)?) };
    let (st, v) = column()?;
    let lang = match st {
        0 => None,
        st if is_text(st) => Some(std::str::from_utf8(v).ok()?),
        _ => return None,
    };
    Some(TermRow { tag, lex, dt, lang })
}

/// The root-to-leaf path of the last dictionary read and the snapshot it belongs to, kept by
/// the reader between calls (`decode_one` misses arrive one at a time).
#[derive(Default)]
pub(crate) struct TermPath {
    snap: Option<Snapshot>,
    root: u32,
    levels: Vec<Level>,
}

/// One level of the root-to-leaf path: a page and the rowid interval `(lo, hi]` its
/// ancestors place its subtree in (unbounded at the edges of the b-tree).
struct Level {
    page: TablePage,
    lo: Option<i64>,
    hi: Option<i64>,
}

impl Level {
    fn covers(&self, row: i64) -> bool {
        self.lo.is_none_or(|l| row > l) && self.hi.is_none_or(|h| row <= h)
    }
}

/// The path to the last leaf read. Rows are looked up in ascending order, so the next one
/// usually lies in the same leaf or a nearby one: the path climbs only to the deepest level
/// whose interval holds it (the root holds every row), then descends.
struct Path<'r> {
    reader: &'r BtreeReader,
    snap: &'r Snapshot,
    root: u32,
    levels: Vec<Level>,
}

impl Path<'_> {
    /// The cell of `row` in the leaf whose interval holds it, if the leaf has one.
    fn find(&mut self, row: i64) -> Result<Option<usize>, Fallback> {
        while self.levels.last().is_some_and(|l| !l.covers(row)) {
            self.levels.pop();
        }
        if self.levels.is_empty() {
            let page = TablePage::load(self.reader, self.snap, self.root)?;
            self.levels.push(Level { page, lo: None, hi: None });
        }
        loop {
            let top = self.levels.last().ok_or(corrupt("empty b-tree path"))?;
            let page = &top.page;
            if page.leaf {
                return page.find(row);
            }
            let i = page.search(row)?;
            if self.levels.len() >= MAX_DEPTH {
                return Err(corrupt("b-tree deeper than SQLite allows"));
            }
            let lo = if i > 0 { Some(page.key(i - 1)?) } else { top.lo };
            let hi = if i < page.cells { Some(page.key(i)?) } else { top.hi };
            if let (Some(l), Some(h)) = (lo, hi)
                && l >= h
            {
                return Err(corrupt("interior keys out of order"));
            }
            let child = TablePage::load(self.reader, self.snap, page.child(i)?)?;
            self.levels.push(Level { page: child, lo, hi });
        }
    }

    /// The leaf the last `find` ended in.
    fn leaf(&self) -> Result<&TablePage, Fallback> {
        self.levels.last().map(|l| &l.page).ok_or(corrupt("empty b-tree path"))
    }
}

/// Reads the `rdf_term` rows numbered `rows` (ascending and distinct) from the table b-tree.
/// `found(i, row)` receives every row read directly. Returns the indexes into `rows` that must
/// be read through SQL instead (see the module documentation), or `None` when the reader
/// cannot serve the call at all (switched off, a §13 precondition, a table layout or text
/// encoding it does not read): then every row goes through SQL. `poll` is the interrupt check.
pub(crate) fn read_terms(
    reader: &BtreeReader,
    rows: &[i64],
    poll: &mut dyn FnMut() -> Result<()>,
    found: &mut dyn FnMut(usize, TermRow<'_>) -> Result<()>,
) -> Result<Option<Vec<usize>>> {
    if reader.mode() == ReaderMode::Sql || rows.is_empty() {
        return Ok(None);
    }
    let snap = match reader.snapshot() {
        Ok(s) => s,
        Err(why) => {
            reader.note(why, false);
            return Ok(None);
        }
    };
    let root = snap.roots.term();
    if root == 0 || !snap.utf8 {
        let why = if root == 0 { "rdf_term does not have the expected layout" } else { "the database text encoding is not UTF-8" };
        reader.note(Fallback::Unsupported(why), false);
        return Ok(None);
    }
    let mut sql = Vec::new();
    // The kept path is reused when it belongs to this snapshot (same file, data version and
    // root page); its pages are then current.
    let kept = reader.take_term_path();
    let levels = if kept.snap.is_some_and(|s| s.same(&snap)) && kept.root == root { kept.levels } else { Vec::with_capacity(8) };
    let mut path = Path { reader, snap: &snap, root, levels };
    let mut served = 0u64;
    for (i, &row) in rows.iter().enumerate() {
        if i % POLL_ROWS == POLL_ROWS - 1 {
            poll()?;
        }
        let record = match path.find(row) {
            Ok(Some(cell)) => path.leaf().and_then(|leaf| leaf.record(cell)),
            Ok(None) => Ok(None),
            Err(why) => Err(why),
        };
        match record {
            Ok(Some(rec)) => match term_row(rec) {
                Some(t) => {
                    found(i, t)?;
                    served += 1;
                }
                None => sql.push(i),
            },
            Ok(None) => sql.push(i),
            Err(why) => {
                reader.note(why, true);
                sql.extend(i..rows.len());
                path.levels.clear();
                break;
            }
        }
    }
    reader.keep_term_path(TermPath { snap: Some(snap), root, levels: path.levels });
    reader.count(|c| c.term_rows += served);
    Ok(Some(sql))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_type_widths_and_integers() {
        for (st, w) in [(0u64, 0usize), (1, 1), (2, 2), (3, 3), (4, 4), (5, 6), (6, 8), (7, 8), (8, 0), (9, 0), (12, 0), (13, 0), (14, 1), (15, 1), (101, 44)] {
            assert_eq!(width(st), Some(w), "{st}");
        }
        assert_eq!((width(10), width(11)), (None, None));
        assert_eq!(int_value(1, &[0xff]), Some(-1));
        assert_eq!(int_value(2, &[0x01, 0x00]), Some(256));
        assert_eq!(int_value(3, &[0x80, 0, 0]), Some(-8_388_608));
        assert_eq!(int_value(5, &[0, 0, 0, 1, 0, 0]), Some(65536));
        assert_eq!(int_value(6, &i64::MIN.to_be_bytes()), Some(i64::MIN));
        assert_eq!((int_value(8, &[]), int_value(9, &[])), (Some(0), Some(1)));
        assert_eq!(int_value(7, &[0; 8]), None);
    }

    /// SQLite varint encoding of a non-negative value below 2^56.
    fn put_varint(v: u64, out: &mut Vec<u8>) {
        let mut groups = vec![(v & 0x7f) as u8];
        let mut x = v >> 7;
        while x > 0 {
            groups.push((x & 0x7f) as u8 | 0x80);
            x >>= 7;
        }
        groups.reverse();
        out.extend_from_slice(&groups);
    }

    /// A 4096-byte table page holding `keys` (leaf cells with a 1-byte payload, or interior
    /// cells with child pointers 2, 3, ...).
    fn table_page(leaf: bool, keys: &[i64]) -> TablePage {
        let usable = 4096;
        let mut p = vec![0u8; usable];
        p[0] = if leaf { 0x0D } else { 0x05 };
        p[3..5].copy_from_slice(&(keys.len() as u16).to_be_bytes());
        if !leaf {
            p[8..12].copy_from_slice(&999u32.to_be_bytes());
        }
        let ptrs = if leaf { 8 } else { 12 };
        let mut content = usable;
        for (i, k) in keys.iter().enumerate() {
            let mut cell = Vec::new();
            if leaf {
                cell.push(1);
            } else {
                cell.extend_from_slice(&(2 + i as u32).to_be_bytes());
            }
            put_varint(*k as u64, &mut cell);
            if leaf {
                cell.push(0);
            }
            content -= cell.len();
            p[content..content + cell.len()].copy_from_slice(&cell);
            p[ptrs + 2 * i..ptrs + 2 * i + 2].copy_from_slice(&(content as u16).to_be_bytes());
        }
        let page: Arc<[u8]> = p.into();
        let (first, last) = (keys.first().copied().unwrap_or(0), keys.last().copied().unwrap_or(0));
        TablePage { page, leaf, cells: keys.len(), ptrs, content: ptrs + 2 * keys.len(), first, last, right: if leaf { 0 } else { 999 }, usable }
    }

    #[test]
    fn interpolation_search_matches_partition_point() {
        let mut x = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for round in 0..400 {
            let n = [0usize, 1, 2, 3, 50, 200][round % 6];
            let mut keys: Vec<i64> = match round % 4 {
                0 => (0..n as i64).map(|i| 1024 + i).collect(),                        // dense
                1 => (0..n).map(|_| (next() % 1_000_000) as i64 + 1).collect(),        // sparse
                2 => (0..n).map(|i| if i < n / 2 { i as i64 + 1 } else { 900_000 + i as i64 }).collect(), // clustered
                _ => (0..n as i64).map(|i| 1 + i * i * 37).collect(),                  // skewed
            };
            keys.sort_unstable();
            keys.dedup();
            for leaf in [true, false] {
                let page = table_page(leaf, &keys);
                let mut probes: Vec<i64> = keys.iter().flat_map(|k| [k - 1, *k, k + 1]).collect();
                probes.extend([0, 1, i64::from(i32::MAX), 2_000_000]);
                for row in probes {
                    let want = keys.partition_point(|k| *k < row);
                    assert_eq!(page.search(row).unwrap(), want, "{keys:?} row {row} leaf {leaf}");
                    let exact = (want < keys.len() && keys[want] == row).then_some(want);
                    assert_eq!(page.find(row).unwrap(), exact, "{keys:?} row {row} leaf {leaf}");
                }
                for (i, k) in keys.iter().enumerate() {
                    assert_eq!(page.key(i).unwrap(), *k);
                }
                assert!(page.key(keys.len()).is_err());
            }
        }
    }

    /// A record with the given serial types and body.
    fn record(types: &[u8], body: &[u8]) -> Vec<u8> {
        let mut r = vec![types.len() as u8 + 1];
        r.extend_from_slice(types);
        r.extend_from_slice(body);
        r
    }

    #[test]
    fn term_records() {
        // (NULL, 3, 'abc', NULL, NULL, h)
        let rec = record(&[0, 1, 13 + 6, 0, 0, 1], &[3, b'a', b'b', b'c', 7]);
        let t = term_row(&rec).unwrap();
        assert_eq!((t.tag, t.lex, t.dt, t.lang), (3, "abc", None, None));
        // (NULL, 4, 'x', NULL, 'en', h)
        let rec = record(&[0, 1, 15, 0, 17, 1], &[4, b'x', b'e', b'n', 9]);
        let t = term_row(&rec).unwrap();
        assert_eq!((t.tag, t.lex, t.lang), (4, "x", Some("en")));
        // (NULL, 5, '', 1029, NULL): h missing is fine, dt is read.
        let rec = record(&[0, 1, 13, 2, 0], &[5, 0x04, 0x05]);
        assert_eq!(term_row(&rec).unwrap().dt, Some(0x0405));
        // Other shapes go through SQL: id not NULL, REAL tag, BLOB lex, invalid UTF-8, text dt,
        // integer lang, a truncated body, too few columns.
        for rec in [
            record(&[1, 1, 15, 0, 0], &[9, 3, b'a']),
            record(&[0, 7, 15, 0, 0], &[0, 0, 0, 0, 0, 0, 0, 0, b'a']),
            record(&[0, 1, 14, 0, 0], &[3, b'a']),
            record(&[0, 1, 15, 0, 0], &[3, 0xff]),
            record(&[0, 1, 15, 15, 0], &[3, b'a', b'b']),
            record(&[0, 1, 15, 0, 1], &[3, b'a', 1]),
            record(&[0, 1, 19, 0, 0], &[3, b'a']),
            record(&[0, 1, 15, 0], &[3, b'a']),
            vec![],
            vec![0x80],
        ] {
            assert!(term_row(&rec).is_none(), "{rec:?}");
        }
    }
}
