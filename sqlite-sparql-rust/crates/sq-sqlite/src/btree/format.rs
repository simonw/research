//! The parts of the SQLite file format (<https://www.sqlite.org/fileformat2.html>) the reader
//! needs: the database header, index b-tree pages (interior 0x02, leaf 0x0A), varints, and
//! records whose 4 columns are integers.
//!
//! Every byte read from disk is untrusted. Each access is bounds-checked against the usable
//! area of the page. Each inconsistency becomes a [`Fallback`] (the scan then continues as a
//! `SqlCursor`); none is a panic.
use std::cmp::Ordering;

use super::Fallback;

/// Length of the database header at the start of page 1.
pub(crate) const HEADER_LEN: usize = 100;
const MAGIC: &[u8; 16] = b"SQLite format 3\0";
/// The deepest b-tree SQLite itself navigates (`BTCURSOR_MAX_DEPTH`); deeper means corrupt.
pub(crate) const MAX_DEPTH: usize = 20;
/// Columns of every `rdf_quad` b-tree record (the 4 quad columns in index order).
pub(crate) const KEY_COLS: usize = 4;

/// A record of a quad b-tree: the 4 integer columns in index order. Arrays compare
/// lexicographically over signed i64, which is SQLite's order for integer keys.
pub(crate) type Key = [i64; KEY_COLS];

const fn corrupt(what: &'static str) -> Fallback {
    Fallback::Corrupt(what)
}

#[inline]
fn be16(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

#[inline]
fn be32(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The fields of the 100-byte database header that the reader uses.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct DbHeader {
    pub page_size: u32,
    /// Bytes reserved at the end of every page (byte 20).
    pub reserved: u8,
    /// File format write / read versions (bytes 18 and 19): 1 = rollback journal, 2 = WAL.
    pub write_version: u8,
    pub read_version: u8,
    /// File change counter (byte 24).
    pub change_counter: u32,
    /// In-header database size in pages, when it is valid (byte 28, valid when the
    /// version-valid-for number at byte 92 equals the change counter).
    pub page_count: Option<u32>,
    /// Schema cookie (byte 40).
    pub schema_cookie: u32,
}

impl DbHeader {
    pub fn parse(b: &[u8]) -> Result<DbHeader, Fallback> {
        let b = b.get(..HEADER_LEN).ok_or(corrupt("database header is truncated"))?;
        if b[..16] != MAGIC[..] {
            // Encrypted at the pager level (the VFS shows ciphertext), or not a database.
            return Err(Fallback::Unsupported("the file does not start with the SQLite header string"));
        }
        let raw = be16(b, 16).ok_or(corrupt("page size"))?;
        let page_size = if raw == 1 { 65536 } else { u32::from(raw) };
        if !(512..=65536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(corrupt("invalid page size"));
        }
        let reserved = b[20];
        // SQLite requires at least 480 usable bytes per page.
        if page_size - u32::from(reserved) < 480 {
            return Err(corrupt("too many reserved bytes per page"));
        }
        let change_counter = be32(b, 24).ok_or(corrupt("change counter"))?;
        let size = be32(b, 28).ok_or(corrupt("database size"))?;
        let valid_for = be32(b, 92).ok_or(corrupt("version-valid-for"))?;
        Ok(DbHeader {
            page_size,
            reserved,
            write_version: b[18],
            read_version: b[19],
            change_counter,
            page_count: (size != 0 && valid_for == change_counter).then_some(size),
            schema_cookie: be32(b, 40).ok_or(corrupt("schema cookie"))?,
        })
    }

    /// Rollback-journal mode (bytes 18/19 are 1), not WAL.
    pub fn rollback_journal(&self) -> bool {
        self.write_version == 1 && self.read_version == 1
    }

    pub fn geometry(&self) -> Geometry {
        Geometry::new(self.page_size as usize, self.reserved as usize)
    }
}

/// Page layout shared by every page of one database file.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct Geometry {
    pub page_size: usize,
    /// Page size minus the reserved bytes at the end of every page.
    pub usable: usize,
    /// Largest payload stored entirely on an index b-tree page (no overflow page).
    pub max_local: usize,
}

impl Geometry {
    pub fn new(page_size: usize, reserved: usize) -> Geometry {
        let usable = page_size - reserved;
        Geometry { page_size, usable, max_local: (usable - 12) * 64 / 255 - 23 }
    }
}

/// The b-tree page header of an index page.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub(crate) struct NodeHeader {
    pub leaf: bool,
    /// Number of cells.
    pub cells: usize,
    /// Offset of the cell pointer array.
    pub ptrs: usize,
    /// Right-most child (interior pages only; 0 on leaves).
    pub right: u32,
}

impl NodeHeader {
    /// Parses and validates the header of page `pgno` (page 1 starts after the database
    /// header). `page` is the whole page as read from the file.
    pub fn parse(page: &[u8], pgno: u32, g: &Geometry) -> Result<NodeHeader, Fallback> {
        let b = page.get(..g.usable).ok_or(corrupt("page is shorter than the page size"))?;
        let hdr = if pgno == 1 { HEADER_LEN } else { 0 };
        let leaf = match b.get(hdr).copied() {
            Some(0x0A) => true,
            Some(0x02) => false,
            Some(0x05 | 0x0D) => return Err(Fallback::Unsupported("table b-tree page where an index b-tree was expected")),
            _ => return Err(corrupt("not an index b-tree page")),
        };
        let cells = usize::from(be16(b, hdr + 3).ok_or(corrupt("cell count"))?);
        let ptrs = hdr + if leaf { 8 } else { 12 };
        if ptrs + 2 * cells > g.usable {
            return Err(corrupt("cell pointer array overflows the page"));
        }
        let right = if leaf { 0 } else { be32(b, hdr + 8).ok_or(corrupt("right-most pointer"))? };
        if !leaf && right == 0 {
            return Err(corrupt("interior page without a right-most child"));
        }
        Ok(NodeHeader { leaf, cells, ptrs, right })
    }
}

/// The cells of one page. The page and its cell pointer array are bounds-checked once, so
/// the scan loop reads every cell of a leaf through one view.
pub(crate) struct Cells<'p> {
    /// The usable area of the page.
    b: &'p [u8],
    /// The cell pointer array.
    ptrs: &'p [u8],
    /// Cells start after the pointer array (the cell content area).
    content: usize,
    /// Bytes before the payload size: the 4-byte left child pointer on interior pages.
    skip: usize,
    right: u32,
    max_local: usize,
}

impl<'p> Cells<'p> {
    #[inline]
    pub fn new(page: &'p [u8], h: &NodeHeader, g: &Geometry) -> Result<Cells<'p>, Fallback> {
        let b = page.get(..g.usable).ok_or(corrupt("page is shorter than the page size"))?;
        let content = h.ptrs + 2 * h.cells;
        let ptrs = b.get(h.ptrs..content).ok_or(corrupt("cell pointer array overflows the page"))?;
        Ok(Cells { b, ptrs, content, skip: if h.leaf { 0 } else { 4 }, right: h.right, max_local: g.max_local })
    }

    /// Offset of cell `i`, inside the cell content area.
    #[inline]
    fn offset(&self, i: usize) -> Result<usize, Fallback> {
        let p = self.ptrs.get(2 * i..2 * i + 2).ok_or(corrupt("cell index out of range"))?;
        let off = usize::from(u16::from_be_bytes([p[0], p[1]]));
        if off < self.content || off >= self.b.len() {
            return Err(corrupt("cell pointer outside the cell content area"));
        }
        Ok(off)
    }

    /// The record (payload) of cell `i`.
    #[inline]
    fn record(&self, i: usize) -> Result<&'p [u8], Fallback> {
        let at = self.offset(i)? + self.skip;
        // The payload size is a one-byte varint for every record of 4 integer columns.
        let (payload, n) = match self.b.get(at) {
            Some(&x) if x < 0x80 => (u64::from(x), 1),
            _ => varint(self.b, at)?,
        };
        if payload > self.max_local as u64 {
            // Never the case for 4 integer columns (at most 41 bytes).
            return Err(Fallback::Unsupported("record spills to an overflow page"));
        }
        let start = at + n;
        self.b.get(start..start + payload as usize).ok_or(corrupt("cell payload overflows the page"))
    }

    /// The key of cell `i`. Interior cells of index b-trees are entries too (they come
    /// between their left child and the next child in key order).
    #[inline]
    pub fn key(&self, i: usize) -> Result<Key, Fallback> {
        decode_record(self.record(i)?)
    }

    /// The key of cell `i` compared with `t`, decoding columns only up to the first
    /// difference (binary searches). Rows that are returned are always decoded and validated
    /// in full by `key`.
    #[inline]
    pub fn cmp(&self, i: usize, t: &Key) -> Result<Ordering, Fallback> {
        cmp_record(self.record(i)?, t)
    }

    /// Child `i` of an interior page: the left child of cell `i`, or the right-most child
    /// when `i` is the number of cells.
    pub fn child(&self, i: usize) -> Result<u32, Fallback> {
        if i == self.ptrs.len() / 2 {
            return Ok(self.right);
        }
        let pgno = be32(self.b, self.offset(i)?).ok_or(corrupt("left child pointer"))?;
        if pgno == 0 {
            return Err(corrupt("child pointer to page 0"));
        }
        Ok(pgno)
    }
}

/// Child `i` of an interior page (see [`Cells::child`]).
pub(crate) fn child(page: &[u8], h: &NodeHeader, i: usize, g: &Geometry) -> Result<u32, Fallback> {
    debug_assert!(!h.leaf);
    Cells::new(page, h, g)?.child(i)
}

/// The key of cell `i` (see [`Cells::key`]).
#[inline]
pub(crate) fn cell_key(page: &[u8], h: &NodeHeader, i: usize, g: &Geometry) -> Result<Key, Fallback> {
    Cells::new(page, h, g)?.key(i)
}

/// The key of cell `i` compared with `t` (see [`Cells::cmp`]).
#[inline]
pub(crate) fn cell_cmp(page: &[u8], h: &NodeHeader, i: usize, g: &Geometry, t: &Key) -> Result<Ordering, Fallback> {
    Cells::new(page, h, g)?.cmp(i, t)
}

/// A record of 4 integer columns compared with `t`, column by column.
#[inline]
fn cmp_record(rec: &[u8], t: &Key) -> Result<Ordering, Fallback> {
    // Fast path: a one-byte header of 4 integer serial types (see `decode_record`).
    if let &[5, t0, t1, t2, t3, ref body @ ..] = rec
        && [t0, t1, t2, t3].into_iter().all(|st| int_width(st).is_some())
    {
        let mut at = 0;
        for (st, want) in [t0, t1, t2, t3].into_iter().zip(t) {
            let w = int_width(st).unwrap_or(0);
            let v = match st {
                8 => 0,
                9 => 1,
                _ => be_signed(body.get(at..at + w).ok_or(corrupt("record body is truncated"))?),
            };
            at += w;
            match v.cmp(want) {
                Ordering::Equal => {}
                other => return Ok(other),
            }
        }
        return Ok(Ordering::Equal);
    }
    let (header, mut at) = record_header(rec)?;
    let mut body = header.len();
    for want in t {
        if at >= header.len() {
            return Err(Fallback::Unsupported("record has fewer than 4 columns"));
        }
        let (st, n) = varint(header, at)?;
        at += n;
        let (v, w) = int_column(st, rec.get(body..).unwrap_or_default())?;
        body += w;
        match v.cmp(want) {
            Ordering::Equal => {}
            other => return Ok(other),
        }
    }
    Ok(Ordering::Equal)
}

/// SQLite varint at `at`: 1–9 bytes, big-endian, 7 bits per byte with the high bit as the
/// continuation flag; the 9th byte contributes all 8 bits. Returns (value, length).
#[inline]
pub(crate) fn varint(b: &[u8], at: usize) -> Result<(u64, usize), Fallback> {
    let b = b.get(at..).ok_or(corrupt("varint past the end of the page"))?;
    match b.first() {
        Some(&x) if x < 0x80 => return Ok((u64::from(x), 1)),
        None => return Err(corrupt("varint past the end of the page")),
        Some(_) => {}
    }
    let mut v = 0u64;
    for (i, &x) in b.iter().take(8).enumerate() {
        v = (v << 7) | u64::from(x & 0x7f);
        if x < 0x80 {
            return Ok((v, i + 1));
        }
    }
    let last = *b.get(8).ok_or(corrupt("varint past the end of the page"))?;
    Ok(((v << 8) | u64::from(last), 9))
}

/// Value and body width of an integer column of serial type `st`, read from `body`.
#[inline]
fn int_column(st: u64, body: &[u8]) -> Result<(i64, usize), Fallback> {
    let truncated = || corrupt("record body is truncated");
    let fixed = |w: usize| body.get(..w).ok_or_else(truncated);
    Ok(match st {
        1 => (i64::from(fixed(1)?[0] as i8), 1),
        2 => (i64::from(i16::from_be_bytes([fixed(2)?[0], body[1]])), 2),
        3 => {
            let s = fixed(3)?;
            (i64::from(i32::from_be_bytes([s[0], s[1], s[2], 0]) >> 8), 3)
        }
        4 => {
            let s = fixed(4)?;
            (i64::from(i32::from_be_bytes([s[0], s[1], s[2], s[3]])), 4)
        }
        5 => {
            let s = fixed(6)?;
            (i64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], 0, 0]) >> 16, 6)
        }
        6 => {
            let s = fixed(8)?;
            (i64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]), 8)
        }
        8 => (0, 0),
        9 => (1, 0),
        0 => return Err(Fallback::Unsupported("NULL column in a quad record")),
        7 => return Err(Fallback::Unsupported("REAL column in a quad record")),
        10 | 11 => return Err(corrupt("reserved serial type")),
        _ => return Err(Fallback::Unsupported("TEXT or BLOB column in a quad record")),
    })
}

/// The record header (serial types included) and the offset of the first serial type.
#[inline]
fn record_header(rec: &[u8]) -> Result<(&[u8], usize), Fallback> {
    let (hlen, at) = varint(rec, 0)?;
    let hlen = usize::try_from(hlen).map_err(|_| corrupt("record header size"))?;
    if hlen > rec.len() || hlen < at {
        return Err(corrupt("record header size"));
    }
    Ok((&rec[..hlen], at))
}

/// Body width of an integer serial type that fits a one-byte header entry.
#[inline]
fn int_width(st: u8) -> Option<usize> {
    match st {
        1..=4 => Some(usize::from(st)),
        5 => Some(6),
        6 => Some(8),
        8 | 9 => Some(0),
        _ => None,
    }
}

/// Big-endian two's-complement integer of 1..=8 bytes.
#[inline]
fn be_signed(b: &[u8]) -> i64 {
    let mut v = i64::from(b[0] as i8);
    for &x in &b[1..] {
        v = (v << 8) | i64::from(x);
    }
    v
}

/// Decodes a record of exactly 4 integer columns.
#[inline]
pub(crate) fn decode_record(rec: &[u8]) -> Result<Key, Fallback> {
    // Fast path: a 5-byte header holding 4 one-byte integer serial types, which is the
    // shape of every record SQLite writes for 4 integer columns.
    if let &[5, t0, t1, t2, t3, ref body @ ..] = rec
        && let (Some(w0), Some(w1), Some(w2), Some(w3)) = (int_width(t0), int_width(t1), int_width(t2), int_width(t3))
        && w0 + w1 + w2 + w3 == body.len()
    {
        let mut key = [0i64; KEY_COLS];
        let mut at = 0;
        for ((col, st), w) in key.iter_mut().zip([t0, t1, t2, t3]).zip([w0, w1, w2, w3]) {
            *col = match st {
                8 => 0,
                9 => 1,
                _ => be_signed(&body[at..at + w]),
            };
            at += w;
        }
        return Ok(key);
    }
    let (header, mut at) = record_header(rec)?;
    let hlen = header.len();
    let mut body = hlen;
    let mut key = [0i64; KEY_COLS];
    for col in &mut key {
        if at >= hlen {
            return Err(Fallback::Unsupported("record has fewer than 4 columns"));
        }
        let (st, n) = varint(header, at)?;
        at += n;
        let (v, w) = int_column(st, rec.get(body..).unwrap_or_default())?;
        *col = v;
        body += w;
    }
    if at != hlen {
        return Err(Fallback::Unsupported("record has more than 4 columns"));
    }
    if body != rec.len() {
        return Err(corrupt("record body size does not match its header"));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SQLite's varint encoder (sqlite3PutVarint), for round trips.
    fn put_varint(v: u64, out: &mut Vec<u8>) {
        if v & (0xff00_0000u64 << 32) != 0 {
            let mut buf = [0u8; 9];
            buf[8] = v as u8;
            let mut x = v >> 8;
            for i in (0..8).rev() {
                buf[i] = ((x & 0x7f) | 0x80) as u8;
                x >>= 7;
            }
            out.extend_from_slice(&buf);
            return;
        }
        let mut buf = Vec::new();
        let mut x = v;
        loop {
            buf.push(((x & 0x7f) | 0x80) as u8);
            x >>= 7;
            if x == 0 {
                break;
            }
        }
        buf[0] &= 0x7f;
        buf.reverse();
        out.extend_from_slice(&buf);
    }

    /// SQLite's record encoder for integer columns (smallest serial type, 8/9 for 0/1).
    pub(crate) fn encode_record(cols: &[i64]) -> Vec<u8> {
        let mut types = Vec::new();
        let mut body = Vec::new();
        for &v in cols {
            let (st, w) = match v {
                0 => (8u64, 0usize),
                1 => (9, 0),
                v if (-128..=127).contains(&v) => (1, 1),
                v if (-32768..=32767).contains(&v) => (2, 2),
                v if (-8_388_608..=8_388_607).contains(&v) => (3, 3),
                v if (i32::MIN as i64..=i32::MAX as i64).contains(&v) => (4, 4),
                v if (-(1i64 << 47)..(1i64 << 47)).contains(&v) => (5, 6),
                _ => (6, 8),
            };
            put_varint(st, &mut types);
            body.extend_from_slice(&v.to_be_bytes()[8 - w..]);
        }
        let mut rec = Vec::new();
        put_varint((types.len() + 1) as u64, &mut rec);
        rec.extend_from_slice(&types);
        rec.extend_from_slice(&body);
        rec
    }

    #[test]
    fn varints_round_trip() {
        let mut vals = vec![0u64, 1, 0x7f, 0x80, 0x3fff, 0x4000, 0x1f_ffff, 0x20_0000, 1 << 56, (1 << 56) - 1, u64::MAX];
        let mut x = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..2000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            vals.push(x >> (x % 64));
        }
        for v in vals {
            let mut b = vec![0xaa];
            put_varint(v, &mut b);
            let (got, n) = varint(&b, 1).unwrap();
            assert_eq!((got, n), (v, b.len() - 1), "{v:#x}");
            // Truncated encodings are corrupt, never a panic.
            for cut in 1..b.len() {
                assert!(varint(&b[..cut], 1).is_err() || cut == b.len());
            }
        }
        assert!(varint(&[], 0).is_err());
        assert!(varint(&[1], 5).is_err());
    }

    #[test]
    fn records_of_every_integer_serial_type() {
        let edges = [
            0i64,
            1,
            -1,
            2,
            127,
            -128,
            128,
            -129,
            32767,
            -32768,
            32768,
            8_388_607,
            -8_388_608,
            8_388_608,
            i32::MAX as i64,
            i32::MIN as i64,
            1 << 40,
            -(1 << 47),
            (1 << 47) - 1,
            1 << 47,
            i64::MAX,
            i64::MIN,
        ];
        for a in edges {
            for b in [0i64, -5, 70_000] {
                let cols = [a, b, -a.wrapping_add(3), 1];
                let rec = encode_record(&cols);
                assert_eq!(decode_record(&rec).unwrap(), cols, "{cols:?}");
                // Every truncation is an error.
                for cut in 0..rec.len() {
                    assert!(decode_record(&rec[..cut]).is_err(), "{cols:?} cut at {cut}");
                }
            }
        }
    }

    #[test]
    fn records_with_other_shapes_fall_back() {
        assert!(matches!(decode_record(&encode_record(&[1, 2, 3])), Err(Fallback::Unsupported(_))));
        assert!(matches!(decode_record(&encode_record(&[1, 2, 3, 4, 5])), Err(Fallback::Unsupported(_))));
        // NULL, REAL, TEXT, BLOB and the reserved serial types.
        for (st, body) in [(0u8, &[][..]), (7, &[0u8; 8][..]), (13, &b"a"[..]), (14, &[1u8][..]), (10, &[][..])] {
            let mut rec = vec![5, 1, 1, 1, st, 1, 2, 3];
            rec.extend_from_slice(body);
            assert!(decode_record(&rec).is_err(), "serial type {st}");
        }
        // Body longer than the header says.
        let mut rec = encode_record(&[5, 6, 7, 8]);
        rec.push(0);
        assert!(matches!(decode_record(&rec), Err(Fallback::Corrupt(_))));
        // Header size larger than the record / smaller than its own varint.
        assert!(decode_record(&[9, 1, 1]).is_err());
        assert!(decode_record(&[0, 1, 1]).is_err());
    }

    fn header(page_size: u16, reserved: u8, versions: u8) -> Vec<u8> {
        let mut h = vec![0u8; HEADER_LEN];
        h[..16].copy_from_slice(MAGIC);
        h[16..18].copy_from_slice(&page_size.to_be_bytes());
        h[18] = versions;
        h[19] = versions;
        h[20] = reserved;
        h[24..28].copy_from_slice(&7u32.to_be_bytes());
        h[28..32].copy_from_slice(&42u32.to_be_bytes());
        h[40..44].copy_from_slice(&3u32.to_be_bytes());
        h[92..96].copy_from_slice(&7u32.to_be_bytes());
        h
    }

    #[test]
    fn database_headers() {
        let h = DbHeader::parse(&header(4096, 0, 1)).unwrap();
        assert_eq!((h.page_size, h.reserved, h.page_count, h.schema_cookie, h.change_counter), (4096, 0, Some(42), 3, 7));
        assert!(h.rollback_journal());
        assert_eq!(h.geometry(), Geometry { page_size: 4096, usable: 4096, max_local: (4096 - 12) * 64 / 255 - 23 });
        // 65536 is encoded as 1.
        let h = DbHeader::parse(&header(1, 32, 1)).unwrap();
        assert_eq!((h.page_size, h.geometry().usable), (65536, 65536 - 32));
        // WAL.
        assert!(!DbHeader::parse(&header(8192, 0, 2)).unwrap().rollback_journal());
        // Stale in-header size (version-valid-for differs from the change counter).
        let mut stale = header(4096, 0, 1);
        stale[92] = 1;
        assert_eq!(DbHeader::parse(&stale).unwrap().page_count, None);
        for bad in [header(1000, 0, 1), header(256, 0, 1), header(0, 0, 1), header(512, 40, 1)] {
            assert!(matches!(DbHeader::parse(&bad), Err(Fallback::Corrupt(_))));
        }
        let mut magic = header(4096, 0, 1);
        magic[3] ^= 1;
        assert!(matches!(DbHeader::parse(&magic), Err(Fallback::Unsupported(_))));
        assert!(DbHeader::parse(&header(4096, 0, 1)[..99]).is_err());
    }

    /// Builds an index page from records (leaf, or interior with left children + right).
    fn page(g: &Geometry, pgno: u32, leaf: bool, cells: &[(u32, Vec<u8>)], right: u32) -> Vec<u8> {
        let mut p = vec![0u8; g.page_size];
        let hdr = if pgno == 1 { HEADER_LEN } else { 0 };
        p[hdr] = if leaf { 0x0A } else { 0x02 };
        p[hdr + 3..hdr + 5].copy_from_slice(&(cells.len() as u16).to_be_bytes());
        if !leaf {
            p[hdr + 8..hdr + 12].copy_from_slice(&right.to_be_bytes());
        }
        let ptrs = hdr + if leaf { 8 } else { 12 };
        let mut content = g.usable;
        for (i, (child, rec)) in cells.iter().enumerate() {
            let mut cell = Vec::new();
            if !leaf {
                cell.extend_from_slice(&child.to_be_bytes());
            }
            put_varint(rec.len() as u64, &mut cell);
            cell.extend_from_slice(rec);
            content -= cell.len();
            p[content..content + cell.len()].copy_from_slice(&cell);
            p[ptrs + 2 * i..ptrs + 2 * i + 2].copy_from_slice(&(content as u16).to_be_bytes());
        }
        p
    }

    #[test]
    fn index_pages() {
        let g = Geometry::new(1024, 16);
        let recs: Vec<(u32, Vec<u8>)> = (0..5).map(|i| (10 + i as u32, encode_record(&[0, i, -i, i * 1000]))).collect();
        let leaf = page(&g, 2, true, &recs, 0);
        let h = NodeHeader::parse(&leaf, 2, &g).unwrap();
        assert_eq!((h.leaf, h.cells, h.ptrs, h.right), (true, 5, 8, 0));
        for i in 0..5 {
            assert_eq!(cell_key(&leaf, &h, i, &g).unwrap(), [0, i as i64, -(i as i64), i as i64 * 1000]);
        }
        assert!(cell_key(&leaf, &h, 5, &g).is_err());
        for (t, want) in [
            ([0, 2, -2, 2000], Ordering::Equal),
            ([0, 2, -2, 1999], Ordering::Greater),
            ([0, 2, -1, i64::MIN], Ordering::Less),
            ([-1, 9, 9, 9], Ordering::Greater),
            ([0, 3, i64::MIN, i64::MIN], Ordering::Less),
        ] {
            assert_eq!(cell_cmp(&leaf, &h, 2, &g, &t).unwrap(), want, "{t:?}");
        }
        let inner = page(&g, 1, false, &recs, 99);
        let h = NodeHeader::parse(&inner, 1, &g).unwrap();
        assert_eq!((h.leaf, h.cells, h.ptrs, h.right), (false, 5, HEADER_LEN + 12, 99));
        assert_eq!(child(&inner, &h, 3, &g).unwrap(), 13);
        assert_eq!(child(&inner, &h, 5, &g).unwrap(), 99);
        assert_eq!(cell_key(&inner, &h, 4, &g).unwrap(), [0, 4, -4, 4000]);
        // Cell pointers into the pointer array or the reserved area are corrupt.
        let mut bad = leaf.clone();
        bad[8..10].copy_from_slice(&9u16.to_be_bytes());
        assert!(matches!(cell_key(&bad, &h_leaf(&bad, &g), 0, &g), Err(Fallback::Corrupt(_))));
        bad[8..10].copy_from_slice(&((g.usable as u16) + 2).to_be_bytes());
        assert!(matches!(cell_key(&bad, &h_leaf(&bad, &g), 0, &g), Err(Fallback::Corrupt(_))));
        // Too many cells for the page, unknown page types, table pages.
        let mut bad = leaf.clone();
        bad[3..5].copy_from_slice(&600u16.to_be_bytes());
        assert!(NodeHeader::parse(&bad, 2, &g).is_err());
        for (kind, unsupported) in [(0x0Du8, true), (0x05, true), (0x00, false), (0x0B, false)] {
            let mut bad = leaf.clone();
            bad[0] = kind;
            let r = NodeHeader::parse(&bad, 2, &g);
            assert_eq!(matches!(r, Err(Fallback::Unsupported(_))), unsupported, "{kind:#x}");
            assert!(r.is_err());
        }
        assert!(NodeHeader::parse(&leaf[..500], 2, &g).is_err());
        // A payload larger than max_local would need an overflow page.
        let big = Geometry::new(512, 0);
        let mut p = vec![0u8; 512];
        p[0] = 0x0A;
        p[3..5].copy_from_slice(&1u16.to_be_bytes());
        p[8..10].copy_from_slice(&100u16.to_be_bytes());
        p[100] = 120; // payload length > max_local (103)
        let h = NodeHeader::parse(&p, 2, &big).unwrap();
        assert!(matches!(cell_key(&p, &h, 0, &big), Err(Fallback::Unsupported(_))));
    }

    fn h_leaf(p: &[u8], g: &Geometry) -> NodeHeader {
        NodeHeader::parse(p, 2, g).unwrap()
    }

    /// Arbitrary bytes never panic: every function returns Ok or a Fallback.
    #[test]
    fn random_pages_never_panic() {
        let g = Geometry::new(512, 8);
        let mut x = 0x1234_5678_9abc_def1u64;
        let mut next = || {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            x
        };
        for round in 0..20_000 {
            let mut p: Vec<u8> = (0..512).map(|_| next() as u8).collect();
            p[0] = [0x0A, 0x02, 0x0A, 0x02, 0x05][round % 5];
            p[3] = 0;
            p[4] = (next() % 40) as u8;
            let pgno = if round % 7 == 0 { 1 } else { 2 };
            if let Ok(h) = NodeHeader::parse(&p, pgno, &g) {
                for i in 0..=h.cells {
                    let _ = cell_key(&p, &h, i, &g);
                    let _ = cell_cmp(&p, &h, i, &g, &[0, 0, 0, 0]);
                    if !h.leaf {
                        let _ = child(&p, &h, i, &g);
                    }
                }
            }
            let _ = decode_record(&p[..(next() % 64) as usize]);
            let _ = DbHeader::parse(&p);
        }
    }
}
