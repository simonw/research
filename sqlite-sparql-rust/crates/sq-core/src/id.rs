//! 64-bit term identifiers.
//!
//! ```text
//!  63                                                 4 3    0
//! +----------------------------------------------------+------+
//! |                 payload (60 bits)                  | tag  |
//! +----------------------------------------------------+------+
//! ```
//! The tag sits in the LOW bits so dictionary ids stay small SQLite varints
//! (row 500K -> id 8,000,017 -> 3 bytes on disk).
//! Ids are stored and compared as SIGNED i64, which is SQLite's integer order.
//! Merge joins, seeks and range scans must compare `i64`, never `u64`.
//!
//! INVARIANT (sameTerm <=> id equality, within one execution):
//!   * inline-encodable terms (`codec::encode_inline` is `Some`) have exactly that id and never
//!     get a dictionary row;
//!   * every other IRI / literal has exactly one `rdf_term` row r and id `(r << 4) | tag`;
//!   * blank nodes have counter ids `(n << 4) | 2` and no dictionary row;
//!   * a computed / query-constant term that is neither inline nor in the dictionary gets ONE
//!     TEMP id from the execution's `TempDict` (which probes the dictionary first when the
//!     value must be canonical).

/// Low 4-bit tag.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Tag {
    /// Only `TermId(0)`: unbound slot; also `g = 0` = default graph in `rdf_quad`.
    Unbound = 0,
    /// Dictionary IRI.
    Iri = 1,
    /// Blank node: payload = counter (`rdf_meta.next_bnode`), no dictionary row.
    BNode = 2,
    /// Dictionary simple literal (xsd:string) that is not SHORTSTR-inlinable (> 7 UTF-8 bytes).
    Str = 3,
    /// Dictionary language-tagged literal (tag lower-cased by oxrdf).
    Lang = 4,
    /// Dictionary literal with any other datatype (incl. non-canonical numerics, doubles, floats,
    /// ill-typed lexical forms, unknown datatypes).
    Typed = 5,
    /// Inline canonical xsd:integer, payload = i60 value.
    Int = 6,
    /// Inline xsd:boolean "true"/"false".
    Bool = 7,
    /// Inline canonical xsd:decimal (see codec).
    Decimal = 8,
    /// Inline canonical xsd:dateTime (see codec).
    DateTime = 9,
    /// Inline canonical xsd:date (see codec).
    Date = 10,
    /// Inline simple literal of <= 7 UTF-8 bytes.
    ShortStr = 11,
    Reserved12 = 12,
    Reserved13 = 13,
    /// Execution-local term: payload = (index << 3) | would-be dictionary tag (1..=5).
    Temp = 14,
    /// Internal sentinel `TermId::ERROR` (error-valued GROUP BY key). Never stored or output.
    Error = 15,
}

const TAGS: [Tag; 16] = [
    Tag::Unbound,
    Tag::Iri,
    Tag::BNode,
    Tag::Str,
    Tag::Lang,
    Tag::Typed,
    Tag::Int,
    Tag::Bool,
    Tag::Decimal,
    Tag::DateTime,
    Tag::Date,
    Tag::ShortStr,
    Tag::Reserved12,
    Tag::Reserved13,
    Tag::Temp,
    Tag::Error,
];

/// Coarse RDF term kind, available without any lookup.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Kind {
    Unbound,
    Iri,
    BNode,
    Literal,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Debug)]
#[repr(transparent)]
pub struct TermId(pub i64);

impl TermId {
    pub const UNBOUND: TermId = TermId(0);
    pub const DEFAULT_GRAPH: TermId = TermId(0);
    /// Error-valued group key; mapped to UNBOUND on output.
    pub const ERROR: TermId = TermId(Tag::Error as i64);
    pub const INT_MIN: i64 = -(1 << 59);
    pub const INT_MAX: i64 = (1 << 59) - 1;

    #[inline]
    pub const fn raw(self) -> i64 {
        self.0
    }
    #[inline]
    pub const fn is_bound(self) -> bool {
        self.0 != 0
    }
    #[inline]
    pub const fn tag_bits(self) -> u8 {
        (self.0 & 0xF) as u8
    }
    #[inline]
    pub const fn tag(self) -> Tag {
        TAGS[(self.0 & 0xF) as usize]
    }
    /// Has an `rdf_term` row (tags 1, 3, 4, 5).
    #[inline]
    pub const fn is_dict(self) -> bool {
        matches!(self.0 & 0xF, 1 | 3 | 4 | 5) && self.0 > 0
    }
    /// Value is fully contained in the id (tags 6..=11).
    #[inline]
    pub const fn is_inline(self) -> bool {
        matches!(self.0 & 0xF, 6..=11)
    }
    #[inline]
    pub const fn is_temp(self) -> bool {
        self.0 & 0xF == Tag::Temp as i64
    }
    #[inline]
    pub const fn is_bnode(self) -> bool {
        self.0 & 0xF == Tag::BNode as i64
            || (self.is_temp() && (self.0 >> 4) & 0x7 == Tag::BNode as i64)
    }
    /// May this id be written to `rdf_quad`? (Not UNBOUND/TEMP/ERROR/reserved.)
    #[inline]
    pub const fn is_storable(self) -> bool {
        matches!(self.0 & 0xF, 1..=11) && self.0 != 0
    }
    /// `rdf_term.id` (row number) of a dictionary id.
    #[inline]
    pub const fn dict_row(self) -> i64 {
        self.0 >> 4
    }
    #[inline]
    pub const fn from_dict(row: i64, tag: Tag) -> TermId {
        TermId((row << 4) | tag as i64)
    }
    #[inline]
    pub const fn bnode(n: i64) -> TermId {
        TermId((n << 4) | Tag::BNode as i64)
    }
    #[inline]
    pub const fn bnode_counter(self) -> i64 {
        self.0 >> 4
    }
    #[inline]
    pub const fn from_int(v: i64) -> Option<TermId> {
        if v >= Self::INT_MIN && v <= Self::INT_MAX {
            Some(TermId((v << 4) | Tag::Int as i64))
        } else {
            None
        }
    }
    #[inline]
    pub const fn as_int(self) -> Option<i64> {
        if self.0 & 0xF == Tag::Int as i64 {
            Some(self.0 >> 4)
        } else {
            None
        }
    }
    #[inline]
    pub const fn from_bool(b: bool) -> TermId {
        TermId(((b as i64) << 4) | Tag::Bool as i64)
    }
    #[inline]
    pub const fn as_bool(self) -> Option<bool> {
        if self.0 & 0xF == Tag::Bool as i64 {
            Some(self.0 >> 4 != 0)
        } else {
            None
        }
    }
    /// TEMP id; `tag` is the dictionary tag the term would have (Iri/BNode/Str/Lang/Typed),
    /// so kind checks need no arena lookup.
    #[inline]
    pub const fn temp(index: u32, tag: Tag) -> TermId {
        TermId(((index as i64) << 7) | ((tag as i64) << 4) | Tag::Temp as i64)
    }
    #[inline]
    pub const fn temp_index(self) -> Option<u32> {
        if self.is_temp() {
            Some((self.0 >> 7) as u32)
        } else {
            None
        }
    }
    /// For TEMP ids: the dictionary tag the term would have.
    #[inline]
    pub const fn temp_tag(self) -> Tag {
        TAGS[((self.0 >> 4) & 0x7) as usize]
    }
    /// Kind without any lookup.
    #[inline]
    pub const fn kind(self) -> Kind {
        let t = if self.is_temp() { ((self.0 >> 4) & 0x7) as u8 } else { (self.0 & 0xF) as u8 };
        match t {
            0 => Kind::Unbound,
            1 => Kind::Iri,
            2 => Kind::BNode,
            _ => Kind::Literal,
        }
    }
    /// Raw id range covering inline ints in [lo, hi] (inclusive). Other tags interleave inside
    /// the range: callers re-check `tag() == Tag::Int`.
    #[inline]
    pub const fn int_raw_range(lo: i64, hi: i64) -> (i64, i64) {
        let lo = if lo < Self::INT_MIN { Self::INT_MIN } else { lo };
        let hi = if hi > Self::INT_MAX { Self::INT_MAX } else { hi };
        ((lo << 4) | Tag::Int as i64, (hi << 4) | Tag::Int as i64)
    }
}

impl From<i64> for TermId {
    fn from(v: i64) -> Self {
        TermId(v)
    }
}
