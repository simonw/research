// md4c.zig - Zig port of md4c.c (CommonMark Markdown parser)
// Auto-generated from translate-c output with manual fixes for goto-containing functions.
// Original: Copyright (c) 2016-2024 Martin Mitáš, MIT License

// Libc functions used by the parser
const __compar_fn_t = ?*const fn (?*const anyopaque, ?*const anyopaque) callconv(.C) c_int;
extern fn memcpy(dest: ?*anyopaque, src: ?*const anyopaque, n: usize) ?*anyopaque;
extern fn memset(dest: ?*anyopaque, c: c_int, n: usize) ?*anyopaque;
extern fn memcmp(s1: ?*const anyopaque, s2: ?*const anyopaque, n: usize) c_int;
extern fn malloc(size: usize) ?*anyopaque;
extern fn realloc(ptr: ?*anyopaque, size: usize) ?*anyopaque;
extern fn free(ptr: ?*anyopaque) void;
extern fn qsort(base: ?*anyopaque, nmemb: usize, size: usize, compar: __compar_fn_t) void;
extern fn bsearch(key: ?*const anyopaque, base: ?*const anyopaque, nmemb: usize, size: usize, compar: __compar_fn_t) ?*anyopaque;
extern fn strchr(s: [*c]const u8, c: c_int) ?[*:0]u8;
extern fn memmove(dest: ?*anyopaque, src: ?*const anyopaque, n: usize) ?*anyopaque;

pub const MD_CHAR = u8;
pub const MD_SIZE = c_uint;
pub const MD_OFFSET = c_uint;
pub const MD_BLOCK_DOC: c_int = 0;
pub const MD_BLOCK_QUOTE: c_int = 1;
pub const MD_BLOCK_UL: c_int = 2;
pub const MD_BLOCK_OL: c_int = 3;
pub const MD_BLOCK_LI: c_int = 4;
pub const MD_BLOCK_HR: c_int = 5;
pub const MD_BLOCK_H: c_int = 6;
pub const MD_BLOCK_CODE: c_int = 7;
pub const MD_BLOCK_HTML: c_int = 8;
pub const MD_BLOCK_P: c_int = 9;
pub const MD_BLOCK_TABLE: c_int = 10;
pub const MD_BLOCK_THEAD: c_int = 11;
pub const MD_BLOCK_TBODY: c_int = 12;
pub const MD_BLOCK_TR: c_int = 13;
pub const MD_BLOCK_TH: c_int = 14;
pub const MD_BLOCK_TD: c_int = 15;
pub const enum_MD_BLOCKTYPE = c_uint;
pub const MD_BLOCKTYPE = enum_MD_BLOCKTYPE;
pub const MD_SPAN_EM: c_int = 0;
pub const MD_SPAN_STRONG: c_int = 1;
pub const MD_SPAN_A: c_int = 2;
pub const MD_SPAN_IMG: c_int = 3;
pub const MD_SPAN_CODE: c_int = 4;
pub const MD_SPAN_DEL: c_int = 5;
pub const MD_SPAN_LATEXMATH: c_int = 6;
pub const MD_SPAN_LATEXMATH_DISPLAY: c_int = 7;
pub const MD_SPAN_WIKILINK: c_int = 8;
pub const MD_SPAN_U: c_int = 9;
pub const enum_MD_SPANTYPE = c_uint;
pub const MD_SPANTYPE = enum_MD_SPANTYPE;
pub const MD_TEXT_NORMAL: c_int = 0;
pub const MD_TEXT_NULLCHAR: c_int = 1;
pub const MD_TEXT_BR: c_int = 2;
pub const MD_TEXT_SOFTBR: c_int = 3;
pub const MD_TEXT_ENTITY: c_int = 4;
pub const MD_TEXT_CODE: c_int = 5;
pub const MD_TEXT_HTML: c_int = 6;
pub const MD_TEXT_LATEXMATH: c_int = 7;
pub const enum_MD_TEXTTYPE = c_uint;
pub const MD_TEXTTYPE = enum_MD_TEXTTYPE;
pub const MD_ALIGN_DEFAULT: c_int = 0;
pub const MD_ALIGN_LEFT: c_int = 1;
pub const MD_ALIGN_CENTER: c_int = 2;
pub const MD_ALIGN_RIGHT: c_int = 3;
pub const enum_MD_ALIGN = c_uint;
pub const MD_ALIGN = enum_MD_ALIGN;
pub const MD_RENDERER = MD_PARSER;
pub const struct_MD_ATTRIBUTE = extern struct {
    text: [*c]const MD_CHAR = @import("std").mem.zeroes([*c]const MD_CHAR),
    size: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    substr_types: [*c]const MD_TEXTTYPE = @import("std").mem.zeroes([*c]const MD_TEXTTYPE),
    substr_offsets: [*c]const MD_OFFSET = @import("std").mem.zeroes([*c]const MD_OFFSET),
};

pub const MD_ATTRIBUTE = struct_MD_ATTRIBUTE;
pub const struct_MD_BLOCK_UL_DETAIL = extern struct {
    is_tight: c_int = @import("std").mem.zeroes(c_int),
    mark: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
};

pub const MD_BLOCK_UL_DETAIL = struct_MD_BLOCK_UL_DETAIL;
pub const struct_MD_BLOCK_OL_DETAIL = extern struct {
    start: c_uint = @import("std").mem.zeroes(c_uint),
    is_tight: c_int = @import("std").mem.zeroes(c_int),
    mark_delimiter: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
};

pub const MD_BLOCK_OL_DETAIL = struct_MD_BLOCK_OL_DETAIL;
pub const struct_MD_BLOCK_LI_DETAIL = extern struct {
    is_task: c_int = @import("std").mem.zeroes(c_int),
    task_mark: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
    task_mark_offset: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
};

pub const MD_BLOCK_LI_DETAIL = struct_MD_BLOCK_LI_DETAIL;
pub const struct_MD_BLOCK_H_DETAIL = extern struct {
    level: c_uint = @import("std").mem.zeroes(c_uint),
};

pub const MD_BLOCK_H_DETAIL = struct_MD_BLOCK_H_DETAIL;
pub const struct_MD_BLOCK_CODE_DETAIL = extern struct {
    info: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
    lang: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
    fence_char: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
};

pub const MD_BLOCK_CODE_DETAIL = struct_MD_BLOCK_CODE_DETAIL;
pub const struct_MD_BLOCK_TABLE_DETAIL = extern struct {
    col_count: c_uint = @import("std").mem.zeroes(c_uint),
    head_row_count: c_uint = @import("std").mem.zeroes(c_uint),
    body_row_count: c_uint = @import("std").mem.zeroes(c_uint),
};

pub const MD_BLOCK_TABLE_DETAIL = struct_MD_BLOCK_TABLE_DETAIL;
pub const struct_MD_BLOCK_TD_DETAIL = extern struct {
    @"align": MD_ALIGN = @import("std").mem.zeroes(MD_ALIGN),
};

pub const MD_BLOCK_TD_DETAIL = struct_MD_BLOCK_TD_DETAIL;
pub const struct_MD_SPAN_A_DETAIL = extern struct {
    href: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
    title: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
    is_autolink: c_int = @import("std").mem.zeroes(c_int),
};

pub const MD_SPAN_A_DETAIL = struct_MD_SPAN_A_DETAIL;
pub const struct_MD_SPAN_IMG_DETAIL = extern struct {
    src: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
    title: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
};

pub const MD_SPAN_IMG_DETAIL = struct_MD_SPAN_IMG_DETAIL;
pub const struct_MD_SPAN_WIKILINK = extern struct {
    target: MD_ATTRIBUTE = @import("std").mem.zeroes(MD_ATTRIBUTE),
};

pub const MD_SPAN_WIKILINK_DETAIL = struct_MD_SPAN_WIKILINK;

pub const struct_MD_PARSER = extern struct {
    abi_version: c_uint = @import("std").mem.zeroes(c_uint),
    flags: c_uint = @import("std").mem.zeroes(c_uint),
    enter_block: ?*const fn (MD_BLOCKTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int = @import("std").mem.zeroes(?*const fn (MD_BLOCKTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int),
    leave_block: ?*const fn (MD_BLOCKTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int = @import("std").mem.zeroes(?*const fn (MD_BLOCKTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int),
    enter_span: ?*const fn (MD_SPANTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int = @import("std").mem.zeroes(?*const fn (MD_SPANTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int),
    leave_span: ?*const fn (MD_SPANTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int = @import("std").mem.zeroes(?*const fn (MD_SPANTYPE, ?*anyopaque, ?*anyopaque) callconv(.C) c_int),
    text: ?*const fn (MD_TEXTTYPE, [*c]const MD_CHAR, MD_SIZE, ?*anyopaque) callconv(.C) c_int = @import("std").mem.zeroes(?*const fn (MD_TEXTTYPE, [*c]const MD_CHAR, MD_SIZE, ?*anyopaque) callconv(.C) c_int),
    debug_log: ?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void = @import("std").mem.zeroes(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void),
    syntax: ?*const fn () callconv(.C) void = @import("std").mem.zeroes(?*const fn () callconv(.C) void),
};

pub const MD_PARSER = struct_MD_PARSER;


// === Internal Type Definitions ===

pub const struct_MD_MARK_tag = extern struct {
    beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    prev: c_int = @import("std").mem.zeroes(c_int),
    next: c_int = @import("std").mem.zeroes(c_int),
    ch: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
    flags: u8 = @import("std").mem.zeroes(u8),
};

pub const MD_MARK = struct_MD_MARK_tag;

pub const struct_MD_MARKSTACK_tag = extern struct {
    top: c_int = @import("std").mem.zeroes(c_int),
};

pub const MD_MARKSTACK = struct_MD_MARKSTACK_tag;

pub const struct_MD_UNICODE_FOLD_INFO_tag = extern struct {
    codepoints: [3]c_uint = @import("std").mem.zeroes([3]c_uint),
    n_codepoints: c_uint = @import("std").mem.zeroes(c_uint),
};

pub const MD_UNICODE_FOLD_INFO = struct_MD_UNICODE_FOLD_INFO_tag;

// MD_REF_DEF - translate-c made this opaque due to bitfields, defined manually
pub const struct_MD_REF_DEF_tag = extern struct {
    label: [*c]MD_CHAR = @import("std").mem.zeroes([*c]MD_CHAR),
    title: [*c]MD_CHAR = @import("std").mem.zeroes([*c]MD_CHAR),
    hash: c_uint = @import("std").mem.zeroes(c_uint),
    label_size: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    title_size: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    dest_beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    dest_end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    label_needs_free: u8 = 0,
    title_needs_free: u8 = 0,
};
pub const MD_REF_DEF = struct_MD_REF_DEF_tag;

// MD_BLOCK - translate-c made this opaque due to bitfields, defined manually  
pub const struct_MD_BLOCK_tag = extern struct {
    // Packed bitfields: type(8) | flags(8) | data(16)
    type_flags_data: u32 = 0,
    n_lines: MD_SIZE = 0,

    pub fn getType(self: *const struct_MD_BLOCK_tag) u8 {
        return @truncate(self.type_flags_data & 0xFF);
    }
    pub fn getFlags(self: *const struct_MD_BLOCK_tag) u8 {
        return @truncate((self.type_flags_data >> 8) & 0xFF);
    }
    pub fn getData(self: *const struct_MD_BLOCK_tag) u16 {
        return @truncate((self.type_flags_data >> 16) & 0xFFFF);
    }
    pub fn setType(self: *struct_MD_BLOCK_tag, t: u8) void {
        self.type_flags_data = (self.type_flags_data & 0xFFFFFF00) | @as(u32, t);
    }
    pub fn setFlags(self: *struct_MD_BLOCK_tag, f: u8) void {
        self.type_flags_data = (self.type_flags_data & 0xFFFF00FF) | (@as(u32, f) << 8);
    }
    pub fn setData(self: *struct_MD_BLOCK_tag, d: u16) void {
        self.type_flags_data = (self.type_flags_data & 0x0000FFFF) | (@as(u32, d) << 16);
    }
};
pub const MD_BLOCK = struct_MD_BLOCK_tag;

// MD_CONTAINER - translate-c made this opaque due to bitfields
pub const struct_MD_CONTAINER_tag = extern struct {
    ch: MD_CHAR = 0,
    is_loose: u8 = 0,
    is_task: u8 = 0,
    _pad: u8 = 0,
    start: c_uint = 0,
    mark_indent: c_uint = 0,
    contents_indent: c_uint = 0,
    block_byte_off: MD_OFFSET = 0,
    task_mark_off: MD_OFFSET = 0,
};
pub const MD_CONTAINER = struct_MD_CONTAINER_tag;

pub const struct_MD_LINE_ANALYSIS_tag = extern struct {
    type: c_uint = @import("std").mem.zeroes(c_uint),
    data: c_uint = @import("std").mem.zeroes(c_uint),
    enforce_new_block: c_int = @import("std").mem.zeroes(c_int),
    beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    indent: c_uint = @import("std").mem.zeroes(c_uint),
};

pub const MD_LINE_ANALYSIS = struct_MD_LINE_ANALYSIS_tag;

pub const struct_MD_LINE_tag = extern struct {
    beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
};

pub const MD_LINE = struct_MD_LINE_tag;

pub const struct_MD_VERBATIMLINE_tag = extern struct {
    beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    indent: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
};

pub const MD_VERBATIMLINE = struct_MD_VERBATIMLINE_tag;

pub const struct_MD_ATTRIBUTE_BUILD_tag = extern struct {
    text: [*c]MD_CHAR = @import("std").mem.zeroes([*c]MD_CHAR),
    substr_types: [*c]MD_TEXTTYPE = @import("std").mem.zeroes([*c]MD_TEXTTYPE),
    substr_offsets: [*c]MD_OFFSET = @import("std").mem.zeroes([*c]MD_OFFSET),
    substr_count: c_int = @import("std").mem.zeroes(c_int),
    substr_alloc: c_int = @import("std").mem.zeroes(c_int),
    trivial_types: [1]MD_TEXTTYPE = @import("std").mem.zeroes([1]MD_TEXTTYPE),
    trivial_offsets: [2]MD_OFFSET = @import("std").mem.zeroes([2]MD_OFFSET),
};

pub const MD_ATTRIBUTE_BUILD = struct_MD_ATTRIBUTE_BUILD_tag;

pub const struct_MD_REF_DEF_LIST_tag = extern struct {
    n_ref_defs: c_int align(8) = @import("std").mem.zeroes(c_int),
    alloc_ref_defs: c_int = @import("std").mem.zeroes(c_int),
    pub fn ref_defs(self: anytype) @import("std").zig.c_translation.FlexibleArrayType(@TypeOf(self), *MD_REF_DEF) {
        const Intermediate = @import("std").zig.c_translation.FlexibleArrayType(@TypeOf(self), u8);
        const ReturnType = @import("std").zig.c_translation.FlexibleArrayType(@TypeOf(self), *MD_REF_DEF);
        return @as(ReturnType, @ptrCast(@alignCast(@as(Intermediate, @ptrCast(self)) + 8)));
    }
};

pub const MD_REF_DEF_LIST = struct_MD_REF_DEF_LIST_tag;

pub const struct_MD_LINK_ATTR_tag = extern struct {
    dest_beg: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    dest_end: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    title: [*c]MD_CHAR = @import("std").mem.zeroes([*c]MD_CHAR),
    title_size: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    title_needs_free: c_int = @import("std").mem.zeroes(c_int),
};

pub const MD_LINK_ATTR = struct_MD_LINK_ATTR_tag;

pub const struct_MD_CTX_tag = extern struct {
    text: [*c]const MD_CHAR = @import("std").mem.zeroes([*c]const MD_CHAR),
    size: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    parser: MD_PARSER = @import("std").mem.zeroes(MD_PARSER),
    userdata: ?*anyopaque = @import("std").mem.zeroes(?*anyopaque),
    doc_ends_with_newline: c_int = @import("std").mem.zeroes(c_int),
    buffer: [*c]MD_CHAR = @import("std").mem.zeroes([*c]MD_CHAR),
    alloc_buffer: c_uint = @import("std").mem.zeroes(c_uint),
    ref_defs: ?*MD_REF_DEF = @import("std").mem.zeroes(?*MD_REF_DEF),
    n_ref_defs: c_int = @import("std").mem.zeroes(c_int),
    alloc_ref_defs: c_int = @import("std").mem.zeroes(c_int),
    ref_def_hashtable: [*c]?*anyopaque = @import("std").mem.zeroes([*c]?*anyopaque),
    ref_def_hashtable_size: c_int = @import("std").mem.zeroes(c_int),
    max_ref_def_output: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    marks: [*c]MD_MARK = @import("std").mem.zeroes([*c]MD_MARK),
    n_marks: c_int = @import("std").mem.zeroes(c_int),
    alloc_marks: c_int = @import("std").mem.zeroes(c_int),
    mark_char_map: [256]u8 = @import("std").mem.zeroes([256]u8),
    opener_stacks: [16]MD_MARKSTACK = @import("std").mem.zeroes([16]MD_MARKSTACK),
    ptr_stack: MD_MARKSTACK = @import("std").mem.zeroes(MD_MARKSTACK),
    n_table_cell_boundaries: c_int = @import("std").mem.zeroes(c_int),
    table_cell_boundaries_head: c_int = @import("std").mem.zeroes(c_int),
    table_cell_boundaries_tail: c_int = @import("std").mem.zeroes(c_int),
    unresolved_link_head: c_int = @import("std").mem.zeroes(c_int),
    unresolved_link_tail: c_int = @import("std").mem.zeroes(c_int),
    html_comment_horizon: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    html_proc_instr_horizon: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    html_decl_horizon: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    html_cdata_horizon: MD_OFFSET = @import("std").mem.zeroes(MD_OFFSET),
    block_bytes: ?*anyopaque = @import("std").mem.zeroes(?*anyopaque),
    current_block: ?*MD_BLOCK = @import("std").mem.zeroes(?*MD_BLOCK),
    n_block_bytes: c_int = @import("std").mem.zeroes(c_int),
    alloc_block_bytes: c_int = @import("std").mem.zeroes(c_int),
    containers: ?*MD_CONTAINER = @import("std").mem.zeroes(?*MD_CONTAINER),
    n_containers: c_int = @import("std").mem.zeroes(c_int),
    alloc_containers: c_int = @import("std").mem.zeroes(c_int),
    code_indent_offset: c_uint = @import("std").mem.zeroes(c_uint),
    code_fence_length: MD_SIZE = @import("std").mem.zeroes(MD_SIZE),
    html_block_type: c_int = @import("std").mem.zeroes(c_int),
    last_line_has_list_loosening_effect: c_int = @import("std").mem.zeroes(c_int),
    last_list_item_starts_with_two_blank_lines: c_int = @import("std").mem.zeroes(c_int),
};

pub const MD_CTX = struct_MD_CTX_tag;


// === Internal Constants ===
const TRUE: c_int = 1;
const FALSE: c_int = 0;
const SZ_MAX: c_uint = 0xFFFFFFFF;

fn SIZEOF_ARRAY(comptime T: type) usize {
    return @sizeOf(T) / @sizeOf(@import("std").meta.Elem(T));
}

fn MIN(a: anytype, b: @TypeOf(a)) @TypeOf(a) {
    return if (a < b) a else b;
}

// Block flags (internal constants from md4c.c)
const MD_BUILD_ATTR_NO_ESCAPES: c_int = 0x0001;

const MD_BLOCK_CONTAINER_OPENER: u8 = 0x01;
const MD_BLOCK_CONTAINER_CLOSER: u8 = 0x02;
const MD_BLOCK_CONTAINER: u8 = MD_BLOCK_CONTAINER_OPENER | MD_BLOCK_CONTAINER_CLOSER;
const MD_BLOCK_LOOSE_LIST: u8 = 0x04;
const MD_BLOCK_SETEXT_HEADER: u8 = 0x08;

// Line type constants
const MD_LINE_BLANK: c_uint = 0;
const MD_LINE_HR: c_uint = 1;
const MD_LINE_ATXHEADER: c_uint = 2;
const MD_LINE_SETEXTHEADER: c_uint = 3;
const MD_LINE_SETEXTUNDERLINE: c_uint = 4;
const MD_LINE_INDENTEDCODE: c_uint = 5;
const MD_LINE_FENCEDCODE: c_uint = 6;
const MD_LINE_HTML: c_uint = 7;
const MD_LINE_TEXT: c_uint = 8;
const MD_LINE_TABLE: c_uint = 9;
const MD_LINE_TABLEUNDERLINE: c_uint = 10;

const md_dummy_blank_line: MD_LINE_ANALYSIS = MD_LINE_ANALYSIS{
    .type = MD_LINE_BLANK,
    .data = 0,
    .enforce_new_block = 0,
    .beg = 0,
    .end = 0,
    .indent = 0,
};

// TAG struct for HTML block start/end condition checking
const TAG = struct {
    name: [*c]const MD_CHAR,
    len: c_uint,
};

fn makeTag(comptime s: []const u8) TAG {
    return TAG{ .name = s.ptr, .len = s.len };
}

const TAG_END = TAG{ .name = null, .len = 0 };

// Type 1 tags: pre, script, style, textarea
const t1 = [_]TAG{
    makeTag("pre"), makeTag("script"), makeTag("style"), makeTag("textarea"), TAG_END,
};

// Type 6 tags grouped by first letter
const a6 = [_]TAG{ makeTag("address"), makeTag("article"), makeTag("aside"), TAG_END };
const b6 = [_]TAG{ makeTag("base"), makeTag("basefont"), makeTag("blockquote"), makeTag("body"), TAG_END };
const c6 = [_]TAG{ makeTag("caption"), makeTag("center"), makeTag("col"), makeTag("colgroup"), TAG_END };
const d6 = [_]TAG{ makeTag("dd"), makeTag("details"), makeTag("dialog"), makeTag("dir"), makeTag("div"), makeTag("dl"), makeTag("dt"), TAG_END };
const f6 = [_]TAG{ makeTag("fieldset"), makeTag("figcaption"), makeTag("figure"), makeTag("footer"), makeTag("form"), makeTag("frame"), makeTag("frameset"), TAG_END };
const h6 = [_]TAG{ makeTag("h1"), makeTag("h2"), makeTag("h3"), makeTag("h4"), makeTag("h5"), makeTag("h6"), makeTag("head"), makeTag("header"), makeTag("hr"), makeTag("html"), TAG_END };
const @"i6" = [_]TAG{ makeTag("iframe"), TAG_END };
const l6 = [_]TAG{ makeTag("legend"), makeTag("li"), makeTag("link"), TAG_END };
const m6 = [_]TAG{ makeTag("main"), makeTag("menu"), makeTag("menuitem"), TAG_END };
const n6 = [_]TAG{ makeTag("nav"), makeTag("noframes"), TAG_END };
const o6 = [_]TAG{ makeTag("ol"), makeTag("optgroup"), makeTag("option"), TAG_END };
const p6 = [_]TAG{ makeTag("p"), makeTag("param"), TAG_END };
const s6 = [_]TAG{ makeTag("search"), makeTag("section"), makeTag("summary"), TAG_END };
const t6 = [_]TAG{ makeTag("table"), makeTag("tbody"), makeTag("td"), makeTag("tfoot"), makeTag("th"), makeTag("thead"), makeTag("title"), makeTag("tr"), makeTag("track"), TAG_END };
const @"u6" = [_]TAG{ makeTag("ul"), TAG_END };
const xx = [_]TAG{TAG_END};

const map6 = [26][]const TAG{
    &a6, &b6, &c6, &d6, &xx, &f6, &xx, &h6, &@"i6", &xx, &xx, &l6, &m6,
    &n6, &o6, &p6, &xx, &xx, &s6, &t6, &@"u6", &xx, &xx, &xx, &xx, &xx,
};


// === Function Implementations ===

pub export fn md_parse(arg_text: [*c]const MD_CHAR, arg_size: MD_SIZE, arg_parser: [*c]const MD_PARSER, arg_userdata: ?*anyopaque) c_int {
    var text = arg_text;
    _ = &text;
    var size = arg_size;
    _ = &size;
    var parser = arg_parser;
    _ = &parser;
    var userdata = arg_userdata;
    _ = &userdata;
    var ctx: MD_CTX = undefined;
    _ = &ctx;
    var i: c_int = undefined;
    _ = &i;
    var ret: c_int = undefined;
    _ = &ret;
    if (parser.*.abi_version != @as(c_uint, @bitCast(@as(c_int, 0)))) {
        if (parser.*.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            parser.*.debug_log.?("Unsupported abi_version.", userdata);
        }
        return -@as(c_int, 1);
    }
    _ = memset(@as(?*anyopaque, @ptrCast(&ctx)), @as(c_int, 0), @sizeOf(MD_CTX));
    ctx.text = text;
    ctx.size = size;
    _ = memcpy(@as(?*anyopaque, @ptrCast(&ctx.parser)), @as(?*const anyopaque, @ptrCast(parser)), @sizeOf(MD_PARSER));
    ctx.userdata = userdata;
    ctx.code_indent_offset = if ((ctx.parser.flags & @as(c_uint, @bitCast(@as(c_int, 16)))) != 0) @as(MD_OFFSET, @bitCast(-@as(c_int, 1))) else @as(MD_OFFSET, @bitCast(@as(c_int, 4)));
    md_build_mark_char_map(&ctx);
    ctx.doc_ends_with_newline = @intFromBool((size > @as(MD_SIZE, @bitCast(@as(c_int, 0)))) and ((@as(c_int, @bitCast(@as(c_uint, text[size -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, text[size -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\n'))));
    ctx.max_ref_def_output = @as(MD_SIZE, @bitCast(@as(c_uint, @truncate(if ((if ((@as(u64, @bitCast(@as(c_long, @as(c_int, 16)))) *% @as(u64, @bitCast(@as(c_ulong, size)))) < @as(u64, @bitCast(@as(c_long, @as(c_int, 1024) * @as(c_int, 1024))))) @as(u64, @bitCast(@as(c_long, @as(c_int, 16)))) *% @as(u64, @bitCast(@as(c_ulong, size))) else @as(u64, @bitCast(@as(c_long, @as(c_int, 1024) * @as(c_int, 1024))))) < @as(u64, @bitCast(if (@sizeOf(MD_SIZE) == @as(c_ulong, @bitCast(@as(c_long, @as(c_int, 8))))) @as(c_ulong, 18446744073709551615) else @as(c_ulong, @bitCast(@as(c_ulong, @as(c_uint, 4294967295))))))) if ((@as(u64, @bitCast(@as(c_long, @as(c_int, 16)))) *% @as(u64, @bitCast(@as(c_ulong, size)))) < @as(u64, @bitCast(@as(c_long, @as(c_int, 1024) * @as(c_int, 1024))))) @as(u64, @bitCast(@as(c_long, @as(c_int, 16)))) *% @as(u64, @bitCast(@as(c_ulong, size))) else @as(u64, @bitCast(@as(c_long, @as(c_int, 1024) * @as(c_int, 1024)))) else @as(u64, @bitCast(if (@sizeOf(MD_SIZE) == @as(c_ulong, @bitCast(@as(c_long, @as(c_int, 8))))) @as(c_ulong, 18446744073709551615) else @as(c_ulong, @bitCast(@as(c_ulong, @as(c_uint, 4294967295))))))))));
    {
        i = 0;
        while (i < @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf([16]MD_MARKSTACK) / @sizeOf(MD_MARKSTACK)))))) : (i += 1) {
            ctx.opener_stacks[@as(c_uint, @intCast(i))].top = -@as(c_int, 1);
        }
    }
    ctx.ptr_stack.top = -@as(c_int, 1);
    ctx.unresolved_link_head = -@as(c_int, 1);
    ctx.unresolved_link_tail = -@as(c_int, 1);
    ctx.table_cell_boundaries_head = -@as(c_int, 1);
    ctx.table_cell_boundaries_tail = -@as(c_int, 1);
    ret = md_process_doc(&ctx);
    md_free_ref_defs(&ctx);
    md_free_ref_def_hashtable(&ctx);
    free(@as(?*anyopaque, @ptrCast(ctx.buffer)));
    free(@as(?*anyopaque, @ptrCast(ctx.marks)));
    free(ctx.block_bytes);
    free(@as(?*anyopaque, @ptrCast(ctx.containers)));
    return ret;
}

pub fn md_ascii_case_eq(arg_s1: [*c]const MD_CHAR, arg_s2: [*c]const MD_CHAR, arg_n: MD_SIZE) callconv(.C) c_int {
    var s1 = arg_s1;
    _ = &s1;
    var s2 = arg_s2;
    _ = &s2;
    var n = arg_n;
    _ = &n;
    var i: MD_OFFSET = undefined;
    _ = &i;
    {
        i = 0;
        while (i < n) : (i +%= 1) {
            var ch1: MD_CHAR = s1[i];
            _ = &ch1;
            var ch2: MD_CHAR = s2[i];
            _ = &ch2;
            if ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ch1)))) and (@as(c_uint, @bitCast(@as(c_uint, ch1))) <= @as(c_uint, @bitCast(@as(c_int, 'z'))))) {
                ch1 +%= @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, 'A') - @as(c_int, 'a')))));
            }
            if ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ch2)))) and (@as(c_uint, @bitCast(@as(c_uint, ch2))) <= @as(c_uint, @bitCast(@as(c_int, 'z'))))) {
                ch2 +%= @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, 'A') - @as(c_int, 'a')))));
            }
            if (@as(c_int, @bitCast(@as(c_uint, ch1))) != @as(c_int, @bitCast(@as(c_uint, ch2)))) return 0;
        }
    }
    return 1;
}

pub fn md_ascii_eq(arg_s1: [*c]const MD_CHAR, arg_s2: [*c]const MD_CHAR, arg_n: MD_SIZE) callconv(.C) c_int {
    var s1 = arg_s1;
    _ = &s1;
    var s2 = arg_s2;
    _ = &s2;
    var n = arg_n;
    _ = &n;
    return @intFromBool(memcmp(@as(?*const anyopaque, @ptrCast(s1)), @as(?*const anyopaque, @ptrCast(s2)), @as(c_ulong, @bitCast(@as(c_ulong, n))) *% @sizeOf(MD_CHAR)) == @as(c_int, 0));
}

pub fn md_text_with_null_replacement(arg_ctx: [*c]MD_CTX, arg_type: MD_TEXTTYPE, arg_str: [*c]const MD_CHAR, arg_size: MD_SIZE) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var @"type" = arg_type;
    _ = &@"type";
    var str = arg_str;
    _ = &str;
    var size = arg_size;
    _ = &size;
    var off: MD_OFFSET = 0;
    _ = &off;
    var ret: c_int = 0;
    _ = &ret;
    while (true) {
        while ((off < size) and (@as(c_int, @bitCast(@as(c_uint, str[off]))) != @as(c_int, '\x00'))) {
            off +%= 1;
        }
        if (off > @as(MD_OFFSET, @bitCast(@as(c_int, 0)))) {
            ret = ctx.*.parser.text.?(@"type", str, off, ctx.*.userdata);
            if (ret != @as(c_int, 0)) return ret;
            str += @as([*c]const MD_CHAR, @ptrFromInt(off));
            size -%= @as(MD_SIZE, @bitCast(off));
            off = 0;
        }
        if (off >= size) return 0;
        ret = ctx.*.parser.text.?(@as(c_uint, @bitCast(MD_TEXT_NULLCHAR)), "", @as(MD_SIZE, @bitCast(@as(c_int, 1))), ctx.*.userdata);
        if (ret != @as(c_int, 0)) return ret;
        off +%= 1;
    }
    return 0;
}

pub fn md_lookup_line(arg_off: MD_OFFSET, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_p_line_index: [*c]MD_SIZE) callconv(.C) [*c]const MD_LINE {
    var off = arg_off;
    _ = &off;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var p_line_index = arg_p_line_index;
    _ = &p_line_index;
    var lo: MD_SIZE = undefined;
    _ = &lo;
    var hi: MD_SIZE = undefined;
    _ = &hi;
    var pivot: MD_SIZE = undefined;
    _ = &pivot;
    var line: [*c]const MD_LINE = undefined;
    _ = &line;
    lo = 0;
    hi = n_lines -% @as(MD_SIZE, @bitCast(@as(c_int, 1)));
    while (lo <= hi) {
        pivot = (lo +% hi) / @as(MD_SIZE, @bitCast(@as(c_int, 2)));
        line = &lines[pivot];
        if (off < line.*.beg) {
            if ((hi == @as(MD_SIZE, @bitCast(@as(c_int, 0)))) or (lines[hi -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))].end < off)) {
                if (p_line_index != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_line_index.* = pivot;
                }
                return line;
            }
            hi = pivot -% @as(MD_SIZE, @bitCast(@as(c_int, 1)));
        } else if (off > line.*.end) {
            lo = pivot +% @as(MD_SIZE, @bitCast(@as(c_int, 1)));
        } else {
            if (p_line_index != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                p_line_index.* = pivot;
            }
            return line;
        }
    }
    return null;
}

pub fn md_unicode_bsearch__(arg_codepoint: c_uint, arg_map: [*c]const c_uint, arg_map_size: usize) callconv(.C) c_int {
    var codepoint = arg_codepoint;
    _ = &codepoint;
    var map = arg_map;
    _ = &map;
    var map_size = arg_map_size;
    _ = &map_size;
    var beg: c_int = undefined;
    _ = &beg;
    var end: c_int = undefined;
    _ = &end;
    var pivot_beg: c_int = undefined;
    _ = &pivot_beg;
    var pivot_end: c_int = undefined;
    _ = &pivot_end;
    beg = 0;
    end = @as(c_int, @bitCast(@as(c_uint, @truncate(map_size)))) - @as(c_int, 1);
    while (beg <= end) {
        pivot_beg = blk: {
            const tmp = @divTrunc(beg + end, @as(c_int, 2));
            pivot_end = tmp;
            break :blk tmp;
        };
        if (((blk: {
            const tmp = pivot_end;
            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).* & @as(c_uint, @bitCast(@as(c_int, 1073741824)))) != 0) {
            pivot_end += 1;
        }
        if (((blk: {
            const tmp = pivot_beg;
            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).* & @as(c_uint, 2147483648)) != 0) {
            pivot_beg -= 1;
        }
        if (codepoint < ((blk: {
            const tmp = pivot_beg;
            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).* & @as(c_uint, @bitCast(@as(c_int, 16777215))))) {
            end = pivot_beg - @as(c_int, 1);
        } else if (codepoint > ((blk: {
            const tmp = pivot_end;
            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).* & @as(c_uint, @bitCast(@as(c_int, 16777215))))) {
            beg = pivot_end + @as(c_int, 1);
        } else return pivot_beg;
    }
    return -@as(c_int, 1);
}

pub fn md_is_unicode_whitespace__(arg_codepoint: c_uint) callconv(.C) c_int {
    var codepoint = arg_codepoint;
    _ = &codepoint;
    const WHITESPACE_MAP = struct {
        const static: [8]c_uint = [8]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 32))),
            @as(c_uint, @bitCast(@as(c_int, 160))),
            @as(c_uint, @bitCast(@as(c_int, 5760))),
            @as(c_uint, @bitCast(@as(c_int, 8192) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8202))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8239))),
            @as(c_uint, @bitCast(@as(c_int, 8287))),
            @as(c_uint, @bitCast(@as(c_int, 12288))),
        };
    };
    _ = &WHITESPACE_MAP;
    if (codepoint <= @as(c_uint, @bitCast(@as(c_int, 127)))) return @intFromBool(((codepoint == @as(c_uint, @bitCast(@as(c_int, ' ')))) or (codepoint == @as(c_uint, @bitCast(@as(c_int, '\t'))))) or ((codepoint == @as(c_uint, @bitCast(@as(c_int, '\x0b')))) or (codepoint == @as(c_uint, @bitCast(@as(c_int, '\x0c'))))));
    return @intFromBool(md_unicode_bsearch__(codepoint, @as([*c]const c_uint, @ptrCast(@alignCast(&WHITESPACE_MAP.static))), @sizeOf([8]c_uint) / @sizeOf(c_uint)) >= @as(c_int, 0));
}

pub fn md_is_unicode_punct__(arg_codepoint: c_uint) callconv(.C) c_int {
    var codepoint = arg_codepoint;
    _ = &codepoint;
    const PUNCT_MAP = struct {
        const static: [576]c_uint = [576]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 33) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 47))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 58) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 64))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 91) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 96))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 123) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 126))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 161) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 169))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 171) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 172))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 174) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 177))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 180))),
            @as(c_uint, @bitCast(@as(c_int, 182) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 184))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 187))),
            @as(c_uint, @bitCast(@as(c_int, 191))),
            @as(c_uint, @bitCast(@as(c_int, 215))),
            @as(c_uint, @bitCast(@as(c_int, 247))),
            @as(c_uint, @bitCast(@as(c_int, 706) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 709))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 722) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 735))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 741) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 747))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 749))),
            @as(c_uint, @bitCast(@as(c_int, 751) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 767))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 885))),
            @as(c_uint, @bitCast(@as(c_int, 894))),
            @as(c_uint, @bitCast(@as(c_int, 900) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 901))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 903))),
            @as(c_uint, @bitCast(@as(c_int, 1014))),
            @as(c_uint, @bitCast(@as(c_int, 1154))),
            @as(c_uint, @bitCast(@as(c_int, 1370) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1375))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1417) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1418))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1421) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1423))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1470))),
            @as(c_uint, @bitCast(@as(c_int, 1472))),
            @as(c_uint, @bitCast(@as(c_int, 1475))),
            @as(c_uint, @bitCast(@as(c_int, 1478))),
            @as(c_uint, @bitCast(@as(c_int, 1523) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1524))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1542) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1551))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1563))),
            @as(c_uint, @bitCast(@as(c_int, 1565) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1567))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1642) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1645))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1748))),
            @as(c_uint, @bitCast(@as(c_int, 1758))),
            @as(c_uint, @bitCast(@as(c_int, 1769))),
            @as(c_uint, @bitCast(@as(c_int, 1789) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1790))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1792) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1805))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2038) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2041))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2046) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2047))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2096) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2110))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2142))),
            @as(c_uint, @bitCast(@as(c_int, 2184))),
            @as(c_uint, @bitCast(@as(c_int, 2404) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2405))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2416))),
            @as(c_uint, @bitCast(@as(c_int, 2546) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2547))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2554) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2555))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2557))),
            @as(c_uint, @bitCast(@as(c_int, 2678))),
            @as(c_uint, @bitCast(@as(c_int, 2800) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 2801))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 2928))),
            @as(c_uint, @bitCast(@as(c_int, 3059) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 3066))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 3191))),
            @as(c_uint, @bitCast(@as(c_int, 3199))),
            @as(c_uint, @bitCast(@as(c_int, 3204))),
            @as(c_uint, @bitCast(@as(c_int, 3407))),
            @as(c_uint, @bitCast(@as(c_int, 3449))),
            @as(c_uint, @bitCast(@as(c_int, 3572))),
            @as(c_uint, @bitCast(@as(c_int, 3647))),
            @as(c_uint, @bitCast(@as(c_int, 3663))),
            @as(c_uint, @bitCast(@as(c_int, 3674) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 3675))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 3841) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 3863))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 3866) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 3871))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 3892))),
            @as(c_uint, @bitCast(@as(c_int, 3894))),
            @as(c_uint, @bitCast(@as(c_int, 3896))),
            @as(c_uint, @bitCast(@as(c_int, 3898) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 3901))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 3973))),
            @as(c_uint, @bitCast(@as(c_int, 4030) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4037))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4039) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4044))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4046) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4058))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4170) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4175))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4254) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4255))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4347))),
            @as(c_uint, @bitCast(@as(c_int, 4960) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4968))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 5008) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5017))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 5120))),
            @as(c_uint, @bitCast(@as(c_int, 5741) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5742))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 5787) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5788))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 5867) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5869))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 5941) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5942))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6100) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6102))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6104) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6107))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6144) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6154))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6464))),
            @as(c_uint, @bitCast(@as(c_int, 6468) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6469))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6622) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6655))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6686) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6687))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6816) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6822))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 6824) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 6829))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7002) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7018))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7028) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7038))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7164) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7167))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7227) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7231))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7294) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7295))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7360) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7367))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7379))),
            @as(c_uint, @bitCast(@as(c_int, 8125))),
            @as(c_uint, @bitCast(@as(c_int, 8127) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8129))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8141) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8143))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8157) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8159))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8173) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8175))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8189) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8190))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8208) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8231))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8240) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8286))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8314) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8318))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8330) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8334))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8352) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8384))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8448) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8449))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8451) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8454))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8456) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8457))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8468))),
            @as(c_uint, @bitCast(@as(c_int, 8470) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8472))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8478) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8483))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8485))),
            @as(c_uint, @bitCast(@as(c_int, 8487))),
            @as(c_uint, @bitCast(@as(c_int, 8489))),
            @as(c_uint, @bitCast(@as(c_int, 8494))),
            @as(c_uint, @bitCast(@as(c_int, 8506) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8507))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8512) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8516))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8522) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8525))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8527))),
            @as(c_uint, @bitCast(@as(c_int, 8586) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8587))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8592) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 9254))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 9280) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 9290))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 9372) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 9449))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 9472) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 10101))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 10132) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11123))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11126) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11157))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11159) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11263))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11493) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11498))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11513) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11516))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11518) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11519))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11632))),
            @as(c_uint, @bitCast(@as(c_int, 11776) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11822))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11824) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11869))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11904) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11929))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11931) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12019))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12032) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12245))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12272) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12287))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12289) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12292))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12296) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12320))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12336))),
            @as(c_uint, @bitCast(@as(c_int, 12342) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12343))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12349) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12351))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12443) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12444))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12448))),
            @as(c_uint, @bitCast(@as(c_int, 12539))),
            @as(c_uint, @bitCast(@as(c_int, 12688) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12689))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12694) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12703))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12736) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12771))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12783))),
            @as(c_uint, @bitCast(@as(c_int, 12800) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12830))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12842) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12871))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12880))),
            @as(c_uint, @bitCast(@as(c_int, 12896) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12927))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12938) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 12976))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 12992) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 13311))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 19904) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 19967))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42128) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42182))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42238) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42239))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42509) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42511))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42611))),
            @as(c_uint, @bitCast(@as(c_int, 42622))),
            @as(c_uint, @bitCast(@as(c_int, 42738) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42743))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42752) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42774))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42784) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42785))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42889) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42890))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43048) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43051))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43062) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43065))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43124) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43127))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43214) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43215))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43256) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43258))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43260))),
            @as(c_uint, @bitCast(@as(c_int, 43310) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43311))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43359))),
            @as(c_uint, @bitCast(@as(c_int, 43457) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43469))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43486) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43487))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43612) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43615))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43639) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43641))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43742) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43743))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43760) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43761))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 43867))),
            @as(c_uint, @bitCast(@as(c_int, 43882) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43883))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 44011))),
            @as(c_uint, @bitCast(@as(c_int, 64297))),
            @as(c_uint, @bitCast(@as(c_int, 64434) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 64450))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 64830) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 64847))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 64975))),
            @as(c_uint, @bitCast(@as(c_int, 65020) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65023))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65040) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65049))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65072) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65106))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65108) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65126))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65128) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65131))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65281) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65295))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65306) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65312))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65339) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65344))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65371) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65381))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65504) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65510))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65512) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65518))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65532) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65533))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65792) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65794))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65847) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65855))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65913) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65929))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65932) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65934))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65936) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65948))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65952))),
            @as(c_uint, @bitCast(@as(c_int, 66000) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66044))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66463))),
            @as(c_uint, @bitCast(@as(c_int, 66512))),
            @as(c_uint, @bitCast(@as(c_int, 66927))),
            @as(c_uint, @bitCast(@as(c_int, 67671))),
            @as(c_uint, @bitCast(@as(c_int, 67703) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 67704))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 67871))),
            @as(c_uint, @bitCast(@as(c_int, 67903))),
            @as(c_uint, @bitCast(@as(c_int, 68176) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 68184))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 68223))),
            @as(c_uint, @bitCast(@as(c_int, 68296))),
            @as(c_uint, @bitCast(@as(c_int, 68336) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 68342))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 68409) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 68415))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 68505) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 68508))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69293))),
            @as(c_uint, @bitCast(@as(c_int, 69461) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69465))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69510) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69513))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69703) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69709))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69819) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69820))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69822) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69825))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 69952) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 69955))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70004) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70005))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70085) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70088))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70093))),
            @as(c_uint, @bitCast(@as(c_int, 70107))),
            @as(c_uint, @bitCast(@as(c_int, 70109) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70111))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70200) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70205))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70313))),
            @as(c_uint, @bitCast(@as(c_int, 70731) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70735))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70746) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 70747))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 70749))),
            @as(c_uint, @bitCast(@as(c_int, 70854))),
            @as(c_uint, @bitCast(@as(c_int, 71105) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 71127))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 71233) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 71235))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 71264) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 71276))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 71353))),
            @as(c_uint, @bitCast(@as(c_int, 71484) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 71487))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 71739))),
            @as(c_uint, @bitCast(@as(c_int, 72004) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72006))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72162))),
            @as(c_uint, @bitCast(@as(c_int, 72255) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72262))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72346) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72348))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72350) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72354))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72448) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72457))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72769) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72773))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 72816) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 72817))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 73463) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 73464))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 73539) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 73551))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 73685) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 73713))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 73727))),
            @as(c_uint, @bitCast(@as(c_int, 74864) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 74868))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 77809) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 77810))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 92782) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 92783))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 92917))),
            @as(c_uint, @bitCast(@as(c_int, 92983) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 92991))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 92996) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 92997))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 93847) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 93850))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 94178))),
            @as(c_uint, @bitCast(@as(c_int, 113820))),
            @as(c_uint, @bitCast(@as(c_int, 113823))),
            @as(c_uint, @bitCast(@as(c_int, 118608) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 118723))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 118784) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119029))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119040) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119078))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119081) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119140))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119146) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119148))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119171) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119172))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119180) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119209))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119214) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119274))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119296) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119361))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 119365))),
            @as(c_uint, @bitCast(@as(c_int, 119552) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 119638))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 120513))),
            @as(c_uint, @bitCast(@as(c_int, 120539))),
            @as(c_uint, @bitCast(@as(c_int, 120571))),
            @as(c_uint, @bitCast(@as(c_int, 120597))),
            @as(c_uint, @bitCast(@as(c_int, 120629))),
            @as(c_uint, @bitCast(@as(c_int, 120655))),
            @as(c_uint, @bitCast(@as(c_int, 120687))),
            @as(c_uint, @bitCast(@as(c_int, 120713))),
            @as(c_uint, @bitCast(@as(c_int, 120745))),
            @as(c_uint, @bitCast(@as(c_int, 120771))),
            @as(c_uint, @bitCast(@as(c_int, 120832) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 121343))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 121399) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 121402))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 121453) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 121460))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 121462) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 121475))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 121477) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 121483))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 123215))),
            @as(c_uint, @bitCast(@as(c_int, 123647))),
            @as(c_uint, @bitCast(@as(c_int, 125278) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 125279))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 126124))),
            @as(c_uint, @bitCast(@as(c_int, 126128))),
            @as(c_uint, @bitCast(@as(c_int, 126254))),
            @as(c_uint, @bitCast(@as(c_int, 126704) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 126705))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 126976) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127019))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127024) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127123))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127136) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127150))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127153) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127167))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127169) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127183))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127185) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127221))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127245) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127405))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127462) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127490))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127504) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127547))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127552) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127560))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127568) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127569))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127584) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 127589))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 127744) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 128727))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 128732) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 128748))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 128752) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 128764))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 128768) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 128886))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 128891) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 128985))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 128992) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129003))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129008))),
            @as(c_uint, @bitCast(@as(c_int, 129024) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129035))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129040) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129095))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129104) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129113))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129120) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129159))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129168) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129197))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129200) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129201))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129280) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129619))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129632) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129645))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129648) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129660))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129664) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129672))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129680) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129725))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129727) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129733))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129742) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129755))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129760) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129768))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129776) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129784))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129792) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129938))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 129940) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 129994))) | @as(c_uint, 2147483648),
        };
    };
    _ = &PUNCT_MAP;
    if (codepoint <= @as(c_uint, @bitCast(@as(c_int, 127)))) return @intFromBool(((((@as(c_uint, @bitCast(@as(c_int, 33))) <= codepoint) and (codepoint <= @as(c_uint, @bitCast(@as(c_int, 47))))) or ((@as(c_uint, @bitCast(@as(c_int, 58))) <= codepoint) and (codepoint <= @as(c_uint, @bitCast(@as(c_int, 64)))))) or ((@as(c_uint, @bitCast(@as(c_int, 91))) <= codepoint) and (codepoint <= @as(c_uint, @bitCast(@as(c_int, 96)))))) or ((@as(c_uint, @bitCast(@as(c_int, 123))) <= codepoint) and (codepoint <= @as(c_uint, @bitCast(@as(c_int, 126))))));
    return @intFromBool(md_unicode_bsearch__(codepoint, @as([*c]const c_uint, @ptrCast(@alignCast(&PUNCT_MAP.static))), @sizeOf([576]c_uint) / @sizeOf(c_uint)) >= @as(c_int, 0));
}

pub fn md_get_unicode_fold_info(arg_codepoint: c_uint, arg_info: [*c]MD_UNICODE_FOLD_INFO) callconv(.C) void {
    var codepoint = arg_codepoint;
    _ = &codepoint;
    var info = arg_info;
    _ = &info;
    const FOLD_MAP_1 = struct {
        const static: [283]c_uint = [283]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 65) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 90))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 181))),
            @as(c_uint, @bitCast(@as(c_int, 192) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 214))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 216) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 222))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 256) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 302))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 306) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 310))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 313) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 327))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 330) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 374))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 376))),
            @as(c_uint, @bitCast(@as(c_int, 377) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 381))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 383))),
            @as(c_uint, @bitCast(@as(c_int, 385))),
            @as(c_uint, @bitCast(@as(c_int, 386))),
            @as(c_uint, @bitCast(@as(c_int, 388))),
            @as(c_uint, @bitCast(@as(c_int, 390))),
            @as(c_uint, @bitCast(@as(c_int, 391))),
            @as(c_uint, @bitCast(@as(c_int, 393))),
            @as(c_uint, @bitCast(@as(c_int, 394))),
            @as(c_uint, @bitCast(@as(c_int, 395))),
            @as(c_uint, @bitCast(@as(c_int, 398))),
            @as(c_uint, @bitCast(@as(c_int, 399))),
            @as(c_uint, @bitCast(@as(c_int, 400))),
            @as(c_uint, @bitCast(@as(c_int, 401))),
            @as(c_uint, @bitCast(@as(c_int, 403))),
            @as(c_uint, @bitCast(@as(c_int, 404))),
            @as(c_uint, @bitCast(@as(c_int, 406))),
            @as(c_uint, @bitCast(@as(c_int, 407))),
            @as(c_uint, @bitCast(@as(c_int, 408))),
            @as(c_uint, @bitCast(@as(c_int, 412))),
            @as(c_uint, @bitCast(@as(c_int, 413))),
            @as(c_uint, @bitCast(@as(c_int, 415))),
            @as(c_uint, @bitCast(@as(c_int, 416) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 420))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 422))),
            @as(c_uint, @bitCast(@as(c_int, 423))),
            @as(c_uint, @bitCast(@as(c_int, 425))),
            @as(c_uint, @bitCast(@as(c_int, 428))),
            @as(c_uint, @bitCast(@as(c_int, 430))),
            @as(c_uint, @bitCast(@as(c_int, 431))),
            @as(c_uint, @bitCast(@as(c_int, 433))),
            @as(c_uint, @bitCast(@as(c_int, 434))),
            @as(c_uint, @bitCast(@as(c_int, 435))),
            @as(c_uint, @bitCast(@as(c_int, 437))),
            @as(c_uint, @bitCast(@as(c_int, 439))),
            @as(c_uint, @bitCast(@as(c_int, 440))),
            @as(c_uint, @bitCast(@as(c_int, 444))),
            @as(c_uint, @bitCast(@as(c_int, 452))),
            @as(c_uint, @bitCast(@as(c_int, 453))),
            @as(c_uint, @bitCast(@as(c_int, 455))),
            @as(c_uint, @bitCast(@as(c_int, 456))),
            @as(c_uint, @bitCast(@as(c_int, 458))),
            @as(c_uint, @bitCast(@as(c_int, 459) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 475))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 478) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 494))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 497))),
            @as(c_uint, @bitCast(@as(c_int, 498))),
            @as(c_uint, @bitCast(@as(c_int, 500))),
            @as(c_uint, @bitCast(@as(c_int, 502))),
            @as(c_uint, @bitCast(@as(c_int, 503))),
            @as(c_uint, @bitCast(@as(c_int, 504) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 542))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 544))),
            @as(c_uint, @bitCast(@as(c_int, 546) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 562))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 570))),
            @as(c_uint, @bitCast(@as(c_int, 571))),
            @as(c_uint, @bitCast(@as(c_int, 573))),
            @as(c_uint, @bitCast(@as(c_int, 574))),
            @as(c_uint, @bitCast(@as(c_int, 577))),
            @as(c_uint, @bitCast(@as(c_int, 579))),
            @as(c_uint, @bitCast(@as(c_int, 580))),
            @as(c_uint, @bitCast(@as(c_int, 581))),
            @as(c_uint, @bitCast(@as(c_int, 582) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 590))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 837))),
            @as(c_uint, @bitCast(@as(c_int, 880))),
            @as(c_uint, @bitCast(@as(c_int, 882))),
            @as(c_uint, @bitCast(@as(c_int, 886))),
            @as(c_uint, @bitCast(@as(c_int, 895))),
            @as(c_uint, @bitCast(@as(c_int, 902))),
            @as(c_uint, @bitCast(@as(c_int, 904) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 906))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 908))),
            @as(c_uint, @bitCast(@as(c_int, 910))),
            @as(c_uint, @bitCast(@as(c_int, 911))),
            @as(c_uint, @bitCast(@as(c_int, 913) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 929))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 931) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 939))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 962))),
            @as(c_uint, @bitCast(@as(c_int, 975))),
            @as(c_uint, @bitCast(@as(c_int, 976))),
            @as(c_uint, @bitCast(@as(c_int, 977))),
            @as(c_uint, @bitCast(@as(c_int, 981))),
            @as(c_uint, @bitCast(@as(c_int, 982))),
            @as(c_uint, @bitCast(@as(c_int, 984) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1006))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1008))),
            @as(c_uint, @bitCast(@as(c_int, 1009))),
            @as(c_uint, @bitCast(@as(c_int, 1012))),
            @as(c_uint, @bitCast(@as(c_int, 1013))),
            @as(c_uint, @bitCast(@as(c_int, 1015))),
            @as(c_uint, @bitCast(@as(c_int, 1017))),
            @as(c_uint, @bitCast(@as(c_int, 1018))),
            @as(c_uint, @bitCast(@as(c_int, 1021) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1023))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1024) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1039))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1040) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1071))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1120) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1152))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1162) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1214))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1216))),
            @as(c_uint, @bitCast(@as(c_int, 1217) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1229))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1232) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1326))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 1329) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 1366))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4256) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 4293))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 4295))),
            @as(c_uint, @bitCast(@as(c_int, 4301))),
            @as(c_uint, @bitCast(@as(c_int, 5112) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 5117))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7296))),
            @as(c_uint, @bitCast(@as(c_int, 7297))),
            @as(c_uint, @bitCast(@as(c_int, 7298))),
            @as(c_uint, @bitCast(@as(c_int, 7299))),
            @as(c_uint, @bitCast(@as(c_int, 7300))),
            @as(c_uint, @bitCast(@as(c_int, 7301))),
            @as(c_uint, @bitCast(@as(c_int, 7302))),
            @as(c_uint, @bitCast(@as(c_int, 7303))),
            @as(c_uint, @bitCast(@as(c_int, 7304))),
            @as(c_uint, @bitCast(@as(c_int, 7312) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7354))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7357) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7359))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7680) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7828))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7835))),
            @as(c_uint, @bitCast(@as(c_int, 7840) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7934))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7944) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7951))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7960) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7965))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7976) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7983))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 7992) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 7999))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8008) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8013))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8025))),
            @as(c_uint, @bitCast(@as(c_int, 8027))),
            @as(c_uint, @bitCast(@as(c_int, 8029))),
            @as(c_uint, @bitCast(@as(c_int, 8031))),
            @as(c_uint, @bitCast(@as(c_int, 8040) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8047))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8120))),
            @as(c_uint, @bitCast(@as(c_int, 8121))),
            @as(c_uint, @bitCast(@as(c_int, 8122))),
            @as(c_uint, @bitCast(@as(c_int, 8123))),
            @as(c_uint, @bitCast(@as(c_int, 8126))),
            @as(c_uint, @bitCast(@as(c_int, 8136) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8139))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8152))),
            @as(c_uint, @bitCast(@as(c_int, 8153))),
            @as(c_uint, @bitCast(@as(c_int, 8154))),
            @as(c_uint, @bitCast(@as(c_int, 8155))),
            @as(c_uint, @bitCast(@as(c_int, 8168))),
            @as(c_uint, @bitCast(@as(c_int, 8169))),
            @as(c_uint, @bitCast(@as(c_int, 8170))),
            @as(c_uint, @bitCast(@as(c_int, 8171))),
            @as(c_uint, @bitCast(@as(c_int, 8172))),
            @as(c_uint, @bitCast(@as(c_int, 8184))),
            @as(c_uint, @bitCast(@as(c_int, 8185))),
            @as(c_uint, @bitCast(@as(c_int, 8186))),
            @as(c_uint, @bitCast(@as(c_int, 8187))),
            @as(c_uint, @bitCast(@as(c_int, 8486))),
            @as(c_uint, @bitCast(@as(c_int, 8490))),
            @as(c_uint, @bitCast(@as(c_int, 8491))),
            @as(c_uint, @bitCast(@as(c_int, 8498))),
            @as(c_uint, @bitCast(@as(c_int, 8544) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8559))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8579))),
            @as(c_uint, @bitCast(@as(c_int, 9398) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 9423))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11264) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11311))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11360))),
            @as(c_uint, @bitCast(@as(c_int, 11362))),
            @as(c_uint, @bitCast(@as(c_int, 11363))),
            @as(c_uint, @bitCast(@as(c_int, 11364))),
            @as(c_uint, @bitCast(@as(c_int, 11367) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11371))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11373))),
            @as(c_uint, @bitCast(@as(c_int, 11374))),
            @as(c_uint, @bitCast(@as(c_int, 11375))),
            @as(c_uint, @bitCast(@as(c_int, 11376))),
            @as(c_uint, @bitCast(@as(c_int, 11378))),
            @as(c_uint, @bitCast(@as(c_int, 11381))),
            @as(c_uint, @bitCast(@as(c_int, 11390))),
            @as(c_uint, @bitCast(@as(c_int, 11391))),
            @as(c_uint, @bitCast(@as(c_int, 11392) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 11490))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 11499))),
            @as(c_uint, @bitCast(@as(c_int, 11501))),
            @as(c_uint, @bitCast(@as(c_int, 11506))),
            @as(c_uint, @bitCast(@as(c_int, 42560) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42604))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42624) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42650))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42786) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42798))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42802) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42862))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42873))),
            @as(c_uint, @bitCast(@as(c_int, 42875))),
            @as(c_uint, @bitCast(@as(c_int, 42877))),
            @as(c_uint, @bitCast(@as(c_int, 42878) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42886))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42891))),
            @as(c_uint, @bitCast(@as(c_int, 42893))),
            @as(c_uint, @bitCast(@as(c_int, 42896))),
            @as(c_uint, @bitCast(@as(c_int, 42898))),
            @as(c_uint, @bitCast(@as(c_int, 42902) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42920))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42922))),
            @as(c_uint, @bitCast(@as(c_int, 42923))),
            @as(c_uint, @bitCast(@as(c_int, 42924))),
            @as(c_uint, @bitCast(@as(c_int, 42925))),
            @as(c_uint, @bitCast(@as(c_int, 42926))),
            @as(c_uint, @bitCast(@as(c_int, 42928))),
            @as(c_uint, @bitCast(@as(c_int, 42929))),
            @as(c_uint, @bitCast(@as(c_int, 42930))),
            @as(c_uint, @bitCast(@as(c_int, 42931))),
            @as(c_uint, @bitCast(@as(c_int, 42932) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 42946))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 42948))),
            @as(c_uint, @bitCast(@as(c_int, 42949))),
            @as(c_uint, @bitCast(@as(c_int, 42950))),
            @as(c_uint, @bitCast(@as(c_int, 42951))),
            @as(c_uint, @bitCast(@as(c_int, 42953))),
            @as(c_uint, @bitCast(@as(c_int, 42960))),
            @as(c_uint, @bitCast(@as(c_int, 42966))),
            @as(c_uint, @bitCast(@as(c_int, 42968))),
            @as(c_uint, @bitCast(@as(c_int, 42997))),
            @as(c_uint, @bitCast(@as(c_int, 43888) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 43967))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 65313) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 65338))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66560) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66599))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66736) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66771))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66928) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66938))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66940) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66954))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66956) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 66962))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 66964))),
            @as(c_uint, @bitCast(@as(c_int, 66965))),
            @as(c_uint, @bitCast(@as(c_int, 68736) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 68786))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 71840) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 71871))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 93760) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 93791))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 125184) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 125217))) | @as(c_uint, 2147483648),
        };
    };
    _ = &FOLD_MAP_1;
    const FOLD_MAP_1_DATA = struct {
        const static: [283]c_uint = [283]c_uint{
            97,
            122,
            @as(c_uint, @bitCast(@as(c_int, 956))),
            224,
            246,
            248,
            254,
            @as(c_uint, @bitCast(@as(c_int, 257))),
            @as(c_uint, @bitCast(@as(c_int, 303))),
            @as(c_uint, @bitCast(@as(c_int, 307))),
            @as(c_uint, @bitCast(@as(c_int, 311))),
            @as(c_uint, @bitCast(@as(c_int, 314))),
            @as(c_uint, @bitCast(@as(c_int, 328))),
            @as(c_uint, @bitCast(@as(c_int, 331))),
            @as(c_uint, @bitCast(@as(c_int, 375))),
            255,
            @as(c_uint, @bitCast(@as(c_int, 378))),
            @as(c_uint, @bitCast(@as(c_int, 382))),
            115,
            @as(c_uint, @bitCast(@as(c_int, 595))),
            @as(c_uint, @bitCast(@as(c_int, 387))),
            @as(c_uint, @bitCast(@as(c_int, 389))),
            @as(c_uint, @bitCast(@as(c_int, 596))),
            @as(c_uint, @bitCast(@as(c_int, 392))),
            @as(c_uint, @bitCast(@as(c_int, 598))),
            @as(c_uint, @bitCast(@as(c_int, 599))),
            @as(c_uint, @bitCast(@as(c_int, 396))),
            @as(c_uint, @bitCast(@as(c_int, 477))),
            @as(c_uint, @bitCast(@as(c_int, 601))),
            @as(c_uint, @bitCast(@as(c_int, 603))),
            @as(c_uint, @bitCast(@as(c_int, 402))),
            @as(c_uint, @bitCast(@as(c_int, 608))),
            @as(c_uint, @bitCast(@as(c_int, 611))),
            @as(c_uint, @bitCast(@as(c_int, 617))),
            @as(c_uint, @bitCast(@as(c_int, 616))),
            @as(c_uint, @bitCast(@as(c_int, 409))),
            @as(c_uint, @bitCast(@as(c_int, 623))),
            @as(c_uint, @bitCast(@as(c_int, 626))),
            @as(c_uint, @bitCast(@as(c_int, 629))),
            @as(c_uint, @bitCast(@as(c_int, 417))),
            @as(c_uint, @bitCast(@as(c_int, 421))),
            @as(c_uint, @bitCast(@as(c_int, 640))),
            @as(c_uint, @bitCast(@as(c_int, 424))),
            @as(c_uint, @bitCast(@as(c_int, 643))),
            @as(c_uint, @bitCast(@as(c_int, 429))),
            @as(c_uint, @bitCast(@as(c_int, 648))),
            @as(c_uint, @bitCast(@as(c_int, 432))),
            @as(c_uint, @bitCast(@as(c_int, 650))),
            @as(c_uint, @bitCast(@as(c_int, 651))),
            @as(c_uint, @bitCast(@as(c_int, 436))),
            @as(c_uint, @bitCast(@as(c_int, 438))),
            @as(c_uint, @bitCast(@as(c_int, 658))),
            @as(c_uint, @bitCast(@as(c_int, 441))),
            @as(c_uint, @bitCast(@as(c_int, 445))),
            @as(c_uint, @bitCast(@as(c_int, 454))),
            @as(c_uint, @bitCast(@as(c_int, 454))),
            @as(c_uint, @bitCast(@as(c_int, 457))),
            @as(c_uint, @bitCast(@as(c_int, 457))),
            @as(c_uint, @bitCast(@as(c_int, 460))),
            @as(c_uint, @bitCast(@as(c_int, 460))),
            @as(c_uint, @bitCast(@as(c_int, 476))),
            @as(c_uint, @bitCast(@as(c_int, 479))),
            @as(c_uint, @bitCast(@as(c_int, 495))),
            @as(c_uint, @bitCast(@as(c_int, 499))),
            @as(c_uint, @bitCast(@as(c_int, 499))),
            @as(c_uint, @bitCast(@as(c_int, 501))),
            @as(c_uint, @bitCast(@as(c_int, 405))),
            @as(c_uint, @bitCast(@as(c_int, 447))),
            @as(c_uint, @bitCast(@as(c_int, 505))),
            @as(c_uint, @bitCast(@as(c_int, 543))),
            @as(c_uint, @bitCast(@as(c_int, 414))),
            @as(c_uint, @bitCast(@as(c_int, 547))),
            @as(c_uint, @bitCast(@as(c_int, 563))),
            @as(c_uint, @bitCast(@as(c_int, 11365))),
            @as(c_uint, @bitCast(@as(c_int, 572))),
            @as(c_uint, @bitCast(@as(c_int, 410))),
            @as(c_uint, @bitCast(@as(c_int, 11366))),
            @as(c_uint, @bitCast(@as(c_int, 578))),
            @as(c_uint, @bitCast(@as(c_int, 384))),
            @as(c_uint, @bitCast(@as(c_int, 649))),
            @as(c_uint, @bitCast(@as(c_int, 652))),
            @as(c_uint, @bitCast(@as(c_int, 583))),
            @as(c_uint, @bitCast(@as(c_int, 591))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 881))),
            @as(c_uint, @bitCast(@as(c_int, 883))),
            @as(c_uint, @bitCast(@as(c_int, 887))),
            @as(c_uint, @bitCast(@as(c_int, 1011))),
            @as(c_uint, @bitCast(@as(c_int, 940))),
            @as(c_uint, @bitCast(@as(c_int, 941))),
            @as(c_uint, @bitCast(@as(c_int, 943))),
            @as(c_uint, @bitCast(@as(c_int, 972))),
            @as(c_uint, @bitCast(@as(c_int, 973))),
            @as(c_uint, @bitCast(@as(c_int, 974))),
            @as(c_uint, @bitCast(@as(c_int, 945))),
            @as(c_uint, @bitCast(@as(c_int, 961))),
            @as(c_uint, @bitCast(@as(c_int, 963))),
            @as(c_uint, @bitCast(@as(c_int, 971))),
            @as(c_uint, @bitCast(@as(c_int, 963))),
            @as(c_uint, @bitCast(@as(c_int, 983))),
            @as(c_uint, @bitCast(@as(c_int, 946))),
            @as(c_uint, @bitCast(@as(c_int, 952))),
            @as(c_uint, @bitCast(@as(c_int, 966))),
            @as(c_uint, @bitCast(@as(c_int, 960))),
            @as(c_uint, @bitCast(@as(c_int, 985))),
            @as(c_uint, @bitCast(@as(c_int, 1007))),
            @as(c_uint, @bitCast(@as(c_int, 954))),
            @as(c_uint, @bitCast(@as(c_int, 961))),
            @as(c_uint, @bitCast(@as(c_int, 952))),
            @as(c_uint, @bitCast(@as(c_int, 949))),
            @as(c_uint, @bitCast(@as(c_int, 1016))),
            @as(c_uint, @bitCast(@as(c_int, 1010))),
            @as(c_uint, @bitCast(@as(c_int, 1019))),
            @as(c_uint, @bitCast(@as(c_int, 891))),
            @as(c_uint, @bitCast(@as(c_int, 893))),
            @as(c_uint, @bitCast(@as(c_int, 1104))),
            @as(c_uint, @bitCast(@as(c_int, 1119))),
            @as(c_uint, @bitCast(@as(c_int, 1072))),
            @as(c_uint, @bitCast(@as(c_int, 1103))),
            @as(c_uint, @bitCast(@as(c_int, 1121))),
            @as(c_uint, @bitCast(@as(c_int, 1153))),
            @as(c_uint, @bitCast(@as(c_int, 1163))),
            @as(c_uint, @bitCast(@as(c_int, 1215))),
            @as(c_uint, @bitCast(@as(c_int, 1231))),
            @as(c_uint, @bitCast(@as(c_int, 1218))),
            @as(c_uint, @bitCast(@as(c_int, 1230))),
            @as(c_uint, @bitCast(@as(c_int, 1233))),
            @as(c_uint, @bitCast(@as(c_int, 1327))),
            @as(c_uint, @bitCast(@as(c_int, 1377))),
            @as(c_uint, @bitCast(@as(c_int, 1414))),
            @as(c_uint, @bitCast(@as(c_int, 11520))),
            @as(c_uint, @bitCast(@as(c_int, 11557))),
            @as(c_uint, @bitCast(@as(c_int, 11559))),
            @as(c_uint, @bitCast(@as(c_int, 11565))),
            @as(c_uint, @bitCast(@as(c_int, 5104))),
            @as(c_uint, @bitCast(@as(c_int, 5109))),
            @as(c_uint, @bitCast(@as(c_int, 1074))),
            @as(c_uint, @bitCast(@as(c_int, 1076))),
            @as(c_uint, @bitCast(@as(c_int, 1086))),
            @as(c_uint, @bitCast(@as(c_int, 1089))),
            @as(c_uint, @bitCast(@as(c_int, 1090))),
            @as(c_uint, @bitCast(@as(c_int, 1090))),
            @as(c_uint, @bitCast(@as(c_int, 1098))),
            @as(c_uint, @bitCast(@as(c_int, 1123))),
            @as(c_uint, @bitCast(@as(c_int, 42571))),
            @as(c_uint, @bitCast(@as(c_int, 4304))),
            @as(c_uint, @bitCast(@as(c_int, 4346))),
            @as(c_uint, @bitCast(@as(c_int, 4349))),
            @as(c_uint, @bitCast(@as(c_int, 4351))),
            @as(c_uint, @bitCast(@as(c_int, 7681))),
            @as(c_uint, @bitCast(@as(c_int, 7829))),
            @as(c_uint, @bitCast(@as(c_int, 7777))),
            @as(c_uint, @bitCast(@as(c_int, 7841))),
            @as(c_uint, @bitCast(@as(c_int, 7935))),
            @as(c_uint, @bitCast(@as(c_int, 7936))),
            @as(c_uint, @bitCast(@as(c_int, 7943))),
            @as(c_uint, @bitCast(@as(c_int, 7952))),
            @as(c_uint, @bitCast(@as(c_int, 7957))),
            @as(c_uint, @bitCast(@as(c_int, 7968))),
            @as(c_uint, @bitCast(@as(c_int, 7975))),
            @as(c_uint, @bitCast(@as(c_int, 7984))),
            @as(c_uint, @bitCast(@as(c_int, 7991))),
            @as(c_uint, @bitCast(@as(c_int, 8000))),
            @as(c_uint, @bitCast(@as(c_int, 8005))),
            @as(c_uint, @bitCast(@as(c_int, 8017))),
            @as(c_uint, @bitCast(@as(c_int, 8019))),
            @as(c_uint, @bitCast(@as(c_int, 8021))),
            @as(c_uint, @bitCast(@as(c_int, 8023))),
            @as(c_uint, @bitCast(@as(c_int, 8032))),
            @as(c_uint, @bitCast(@as(c_int, 8039))),
            @as(c_uint, @bitCast(@as(c_int, 8112))),
            @as(c_uint, @bitCast(@as(c_int, 8113))),
            @as(c_uint, @bitCast(@as(c_int, 8048))),
            @as(c_uint, @bitCast(@as(c_int, 8049))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8050))),
            @as(c_uint, @bitCast(@as(c_int, 8053))),
            @as(c_uint, @bitCast(@as(c_int, 8144))),
            @as(c_uint, @bitCast(@as(c_int, 8145))),
            @as(c_uint, @bitCast(@as(c_int, 8054))),
            @as(c_uint, @bitCast(@as(c_int, 8055))),
            @as(c_uint, @bitCast(@as(c_int, 8160))),
            @as(c_uint, @bitCast(@as(c_int, 8161))),
            @as(c_uint, @bitCast(@as(c_int, 8058))),
            @as(c_uint, @bitCast(@as(c_int, 8059))),
            @as(c_uint, @bitCast(@as(c_int, 8165))),
            @as(c_uint, @bitCast(@as(c_int, 8056))),
            @as(c_uint, @bitCast(@as(c_int, 8057))),
            @as(c_uint, @bitCast(@as(c_int, 8060))),
            @as(c_uint, @bitCast(@as(c_int, 8061))),
            @as(c_uint, @bitCast(@as(c_int, 969))),
            107,
            229,
            @as(c_uint, @bitCast(@as(c_int, 8526))),
            @as(c_uint, @bitCast(@as(c_int, 8560))),
            @as(c_uint, @bitCast(@as(c_int, 8575))),
            @as(c_uint, @bitCast(@as(c_int, 8580))),
            @as(c_uint, @bitCast(@as(c_int, 9424))),
            @as(c_uint, @bitCast(@as(c_int, 9449))),
            @as(c_uint, @bitCast(@as(c_int, 11312))),
            @as(c_uint, @bitCast(@as(c_int, 11359))),
            @as(c_uint, @bitCast(@as(c_int, 11361))),
            @as(c_uint, @bitCast(@as(c_int, 619))),
            @as(c_uint, @bitCast(@as(c_int, 7549))),
            @as(c_uint, @bitCast(@as(c_int, 637))),
            @as(c_uint, @bitCast(@as(c_int, 11368))),
            @as(c_uint, @bitCast(@as(c_int, 11372))),
            @as(c_uint, @bitCast(@as(c_int, 593))),
            @as(c_uint, @bitCast(@as(c_int, 625))),
            @as(c_uint, @bitCast(@as(c_int, 592))),
            @as(c_uint, @bitCast(@as(c_int, 594))),
            @as(c_uint, @bitCast(@as(c_int, 11379))),
            @as(c_uint, @bitCast(@as(c_int, 11382))),
            @as(c_uint, @bitCast(@as(c_int, 575))),
            @as(c_uint, @bitCast(@as(c_int, 576))),
            @as(c_uint, @bitCast(@as(c_int, 11393))),
            @as(c_uint, @bitCast(@as(c_int, 11491))),
            @as(c_uint, @bitCast(@as(c_int, 11500))),
            @as(c_uint, @bitCast(@as(c_int, 11502))),
            @as(c_uint, @bitCast(@as(c_int, 11507))),
            @as(c_uint, @bitCast(@as(c_int, 42561))),
            @as(c_uint, @bitCast(@as(c_int, 42605))),
            @as(c_uint, @bitCast(@as(c_int, 42625))),
            @as(c_uint, @bitCast(@as(c_int, 42651))),
            @as(c_uint, @bitCast(@as(c_int, 42787))),
            @as(c_uint, @bitCast(@as(c_int, 42799))),
            @as(c_uint, @bitCast(@as(c_int, 42803))),
            @as(c_uint, @bitCast(@as(c_int, 42863))),
            @as(c_uint, @bitCast(@as(c_int, 42874))),
            @as(c_uint, @bitCast(@as(c_int, 42876))),
            @as(c_uint, @bitCast(@as(c_int, 7545))),
            @as(c_uint, @bitCast(@as(c_int, 42879))),
            @as(c_uint, @bitCast(@as(c_int, 42887))),
            @as(c_uint, @bitCast(@as(c_int, 42892))),
            @as(c_uint, @bitCast(@as(c_int, 613))),
            @as(c_uint, @bitCast(@as(c_int, 42897))),
            @as(c_uint, @bitCast(@as(c_int, 42899))),
            @as(c_uint, @bitCast(@as(c_int, 42903))),
            @as(c_uint, @bitCast(@as(c_int, 42921))),
            @as(c_uint, @bitCast(@as(c_int, 614))),
            @as(c_uint, @bitCast(@as(c_int, 604))),
            @as(c_uint, @bitCast(@as(c_int, 609))),
            @as(c_uint, @bitCast(@as(c_int, 620))),
            @as(c_uint, @bitCast(@as(c_int, 618))),
            @as(c_uint, @bitCast(@as(c_int, 670))),
            @as(c_uint, @bitCast(@as(c_int, 647))),
            @as(c_uint, @bitCast(@as(c_int, 669))),
            @as(c_uint, @bitCast(@as(c_int, 43859))),
            @as(c_uint, @bitCast(@as(c_int, 42933))),
            @as(c_uint, @bitCast(@as(c_int, 42947))),
            @as(c_uint, @bitCast(@as(c_int, 42900))),
            @as(c_uint, @bitCast(@as(c_int, 642))),
            @as(c_uint, @bitCast(@as(c_int, 7566))),
            @as(c_uint, @bitCast(@as(c_int, 42952))),
            @as(c_uint, @bitCast(@as(c_int, 42954))),
            @as(c_uint, @bitCast(@as(c_int, 42961))),
            @as(c_uint, @bitCast(@as(c_int, 42967))),
            @as(c_uint, @bitCast(@as(c_int, 42969))),
            @as(c_uint, @bitCast(@as(c_int, 42998))),
            @as(c_uint, @bitCast(@as(c_int, 5024))),
            @as(c_uint, @bitCast(@as(c_int, 5103))),
            @as(c_uint, @bitCast(@as(c_int, 65345))),
            @as(c_uint, @bitCast(@as(c_int, 65370))),
            @as(c_uint, @bitCast(@as(c_int, 66600))),
            @as(c_uint, @bitCast(@as(c_int, 66639))),
            @as(c_uint, @bitCast(@as(c_int, 66776))),
            @as(c_uint, @bitCast(@as(c_int, 66811))),
            @as(c_uint, @bitCast(@as(c_int, 66967))),
            @as(c_uint, @bitCast(@as(c_int, 66977))),
            @as(c_uint, @bitCast(@as(c_int, 66979))),
            @as(c_uint, @bitCast(@as(c_int, 66993))),
            @as(c_uint, @bitCast(@as(c_int, 66995))),
            @as(c_uint, @bitCast(@as(c_int, 67001))),
            @as(c_uint, @bitCast(@as(c_int, 67003))),
            @as(c_uint, @bitCast(@as(c_int, 67004))),
            @as(c_uint, @bitCast(@as(c_int, 68800))),
            @as(c_uint, @bitCast(@as(c_int, 68850))),
            @as(c_uint, @bitCast(@as(c_int, 71872))),
            @as(c_uint, @bitCast(@as(c_int, 71903))),
            @as(c_uint, @bitCast(@as(c_int, 93792))),
            @as(c_uint, @bitCast(@as(c_int, 93823))),
            @as(c_uint, @bitCast(@as(c_int, 125218))),
            @as(c_uint, @bitCast(@as(c_int, 125251))),
        };
    };
    _ = &FOLD_MAP_1_DATA;
    const FOLD_MAP_2 = struct {
        const static: [52]c_uint = [52]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 223))),
            @as(c_uint, @bitCast(@as(c_int, 304))),
            @as(c_uint, @bitCast(@as(c_int, 329))),
            @as(c_uint, @bitCast(@as(c_int, 496))),
            @as(c_uint, @bitCast(@as(c_int, 1415))),
            @as(c_uint, @bitCast(@as(c_int, 7830))),
            @as(c_uint, @bitCast(@as(c_int, 7831))),
            @as(c_uint, @bitCast(@as(c_int, 7832))),
            @as(c_uint, @bitCast(@as(c_int, 7833))),
            @as(c_uint, @bitCast(@as(c_int, 7834))),
            @as(c_uint, @bitCast(@as(c_int, 7838))),
            @as(c_uint, @bitCast(@as(c_int, 8016))),
            @as(c_uint, @bitCast(@as(c_int, 8064) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8071))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8072) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8079))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8080) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8087))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8088) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8095))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8096) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8103))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8104) | @as(c_int, 1073741824))),
            @as(c_uint, @bitCast(@as(c_int, 8111))) | @as(c_uint, 2147483648),
            @as(c_uint, @bitCast(@as(c_int, 8114))),
            @as(c_uint, @bitCast(@as(c_int, 8115))),
            @as(c_uint, @bitCast(@as(c_int, 8116))),
            @as(c_uint, @bitCast(@as(c_int, 8118))),
            @as(c_uint, @bitCast(@as(c_int, 8124))),
            @as(c_uint, @bitCast(@as(c_int, 8130))),
            @as(c_uint, @bitCast(@as(c_int, 8131))),
            @as(c_uint, @bitCast(@as(c_int, 8132))),
            @as(c_uint, @bitCast(@as(c_int, 8134))),
            @as(c_uint, @bitCast(@as(c_int, 8140))),
            @as(c_uint, @bitCast(@as(c_int, 8150))),
            @as(c_uint, @bitCast(@as(c_int, 8164))),
            @as(c_uint, @bitCast(@as(c_int, 8166))),
            @as(c_uint, @bitCast(@as(c_int, 8178))),
            @as(c_uint, @bitCast(@as(c_int, 8179))),
            @as(c_uint, @bitCast(@as(c_int, 8180))),
            @as(c_uint, @bitCast(@as(c_int, 8182))),
            @as(c_uint, @bitCast(@as(c_int, 8188))),
            @as(c_uint, @bitCast(@as(c_int, 64256))),
            @as(c_uint, @bitCast(@as(c_int, 64257))),
            @as(c_uint, @bitCast(@as(c_int, 64258))),
            @as(c_uint, @bitCast(@as(c_int, 64261))),
            @as(c_uint, @bitCast(@as(c_int, 64262))),
            @as(c_uint, @bitCast(@as(c_int, 64275))),
            @as(c_uint, @bitCast(@as(c_int, 64276))),
            @as(c_uint, @bitCast(@as(c_int, 64277))),
            @as(c_uint, @bitCast(@as(c_int, 64278))),
            @as(c_uint, @bitCast(@as(c_int, 64279))),
        };
    };
    _ = &FOLD_MAP_2;
    const FOLD_MAP_2_DATA = struct {
        const static: [104]c_uint = [104]c_uint{
            115,
            115,
            105,
            @as(c_uint, @bitCast(@as(c_int, 775))),
            @as(c_uint, @bitCast(@as(c_int, 700))),
            110,
            106,
            @as(c_uint, @bitCast(@as(c_int, 780))),
            @as(c_uint, @bitCast(@as(c_int, 1381))),
            @as(c_uint, @bitCast(@as(c_int, 1410))),
            104,
            @as(c_uint, @bitCast(@as(c_int, 817))),
            116,
            @as(c_uint, @bitCast(@as(c_int, 776))),
            119,
            @as(c_uint, @bitCast(@as(c_int, 778))),
            121,
            @as(c_uint, @bitCast(@as(c_int, 778))),
            97,
            @as(c_uint, @bitCast(@as(c_int, 702))),
            115,
            115,
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 787))),
            @as(c_uint, @bitCast(@as(c_int, 7936))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7943))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7936))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7943))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7968))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7975))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7968))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 7975))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8032))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8039))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8032))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8039))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8048))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 945))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 940))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 945))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 945))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 8052))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 951))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 942))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 951))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 951))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 961))),
            @as(c_uint, @bitCast(@as(c_int, 787))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 8060))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 969))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 974))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 969))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 969))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            102,
            102,
            102,
            105,
            102,
            108,
            115,
            116,
            115,
            116,
            @as(c_uint, @bitCast(@as(c_int, 1396))),
            @as(c_uint, @bitCast(@as(c_int, 1398))),
            @as(c_uint, @bitCast(@as(c_int, 1396))),
            @as(c_uint, @bitCast(@as(c_int, 1381))),
            @as(c_uint, @bitCast(@as(c_int, 1396))),
            @as(c_uint, @bitCast(@as(c_int, 1387))),
            @as(c_uint, @bitCast(@as(c_int, 1406))),
            @as(c_uint, @bitCast(@as(c_int, 1398))),
            @as(c_uint, @bitCast(@as(c_int, 1396))),
            @as(c_uint, @bitCast(@as(c_int, 1389))),
        };
    };
    _ = &FOLD_MAP_2_DATA;
    const FOLD_MAP_3 = struct {
        const static: [16]c_uint = [16]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 912))),
            @as(c_uint, @bitCast(@as(c_int, 944))),
            @as(c_uint, @bitCast(@as(c_int, 8018))),
            @as(c_uint, @bitCast(@as(c_int, 8020))),
            @as(c_uint, @bitCast(@as(c_int, 8022))),
            @as(c_uint, @bitCast(@as(c_int, 8119))),
            @as(c_uint, @bitCast(@as(c_int, 8135))),
            @as(c_uint, @bitCast(@as(c_int, 8146))),
            @as(c_uint, @bitCast(@as(c_int, 8147))),
            @as(c_uint, @bitCast(@as(c_int, 8151))),
            @as(c_uint, @bitCast(@as(c_int, 8162))),
            @as(c_uint, @bitCast(@as(c_int, 8163))),
            @as(c_uint, @bitCast(@as(c_int, 8167))),
            @as(c_uint, @bitCast(@as(c_int, 8183))),
            @as(c_uint, @bitCast(@as(c_int, 64259))),
            @as(c_uint, @bitCast(@as(c_int, 64260))),
        };
    };
    _ = &FOLD_MAP_3;
    const FOLD_MAP_3_DATA = struct {
        const static: [48]c_uint = [48]c_uint{
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 769))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 769))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 787))),
            @as(c_uint, @bitCast(@as(c_int, 768))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 787))),
            @as(c_uint, @bitCast(@as(c_int, 769))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 787))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 945))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 951))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 768))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 769))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 768))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 769))),
            @as(c_uint, @bitCast(@as(c_int, 965))),
            @as(c_uint, @bitCast(@as(c_int, 776))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 969))),
            @as(c_uint, @bitCast(@as(c_int, 834))),
            @as(c_uint, @bitCast(@as(c_int, 953))),
            102,
            102,
            105,
            102,
            102,
            108,
        };
    };
    _ = &FOLD_MAP_3_DATA;
    const struct_unnamed_5 = extern struct {
        map: [*c]const c_uint = @import("std").mem.zeroes([*c]const c_uint),
        data: [*c]const c_uint = @import("std").mem.zeroes([*c]const c_uint),
        map_size: usize = @import("std").mem.zeroes(usize),
        n_codepoints: c_uint = @import("std").mem.zeroes(c_uint),
    };
    _ = &struct_unnamed_5;
    const FOLD_MAP_LIST = struct {
        const static: [3]struct_unnamed_5 = [3]struct_unnamed_5{
            struct_unnamed_5{
                .map = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_1.static))),
                .data = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_1_DATA.static))),
                .map_size = @sizeOf([283]c_uint) / @sizeOf(c_uint),
                .n_codepoints = @as(c_uint, @bitCast(@as(c_int, 1))),
            },
            struct_unnamed_5{
                .map = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_2.static))),
                .data = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_2_DATA.static))),
                .map_size = @sizeOf([52]c_uint) / @sizeOf(c_uint),
                .n_codepoints = @as(c_uint, @bitCast(@as(c_int, 2))),
            },
            struct_unnamed_5{
                .map = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_3.static))),
                .data = @as([*c]const c_uint, @ptrCast(@alignCast(&FOLD_MAP_3_DATA.static))),
                .map_size = @sizeOf([16]c_uint) / @sizeOf(c_uint),
                .n_codepoints = @as(c_uint, @bitCast(@as(c_int, 3))),
            },
        };
    };
    _ = &FOLD_MAP_LIST;
    var i: c_int = undefined;
    _ = &i;
    if (codepoint <= @as(c_uint, @bitCast(@as(c_int, 127)))) {
        info.*.codepoints[@as(c_uint, @intCast(@as(c_int, 0)))] = codepoint;
        if ((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= codepoint) and (codepoint <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) {
            info.*.codepoints[@as(c_uint, @intCast(@as(c_int, 0)))] +%= @as(c_uint, @bitCast(@as(c_int, 'a') - @as(c_int, 'A')));
        }
        info.*.n_codepoints = 1;
        return;
    }
    {
        i = 0;
        while (i < @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf([3]struct_unnamed_5) / @sizeOf(struct_unnamed_5)))))) : (i += 1) {
            var index_1: c_int = undefined;
            _ = &index_1;
            index_1 = md_unicode_bsearch__(codepoint, FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].map, FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].map_size);
            if (index_1 >= @as(c_int, 0)) {
                var n_codepoints: c_uint = FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].n_codepoints;
                _ = &n_codepoints;
                var map: [*c]const c_uint = FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].map;
                _ = &map;
                var codepoints: [*c]const c_uint = FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].data + (@as(c_uint, @bitCast(index_1)) *% n_codepoints);
                _ = &codepoints;
                _ = memcpy(@as(?*anyopaque, @ptrCast(@as([*c]c_uint, @ptrCast(@alignCast(&info.*.codepoints))))), @as(?*const anyopaque, @ptrCast(codepoints)), @sizeOf(c_uint) *% @as(c_ulong, @bitCast(@as(c_ulong, n_codepoints))));
                info.*.n_codepoints = n_codepoints;
                if ((blk: {
                    const tmp = index_1;
                    if (tmp >= 0) break :blk FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].map + @as(usize, @intCast(tmp)) else break :blk FOLD_MAP_LIST.static[@as(c_uint, @intCast(i))].map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).* != codepoint) {
                    if ((((blk: {
                        const tmp = index_1;
                        if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                    }).* & @as(c_uint, @bitCast(@as(c_int, 16777215)))) +% @as(c_uint, @bitCast(@as(c_int, 1)))) == codepoints[@as(c_uint, @intCast(@as(c_int, 0)))]) {
                        info.*.codepoints[@as(c_uint, @intCast(@as(c_int, 0)))] = codepoint +% @as(c_uint, @bitCast(if ((codepoint & @as(c_uint, @bitCast(@as(c_int, 1)))) == ((blk: {
                            const tmp = index_1;
                            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                        }).* & @as(c_uint, @bitCast(@as(c_int, 1))))) @as(c_int, 1) else @as(c_int, 0)));
                    } else {
                        info.*.codepoints[@as(c_uint, @intCast(@as(c_int, 0)))] +%= codepoint -% ((blk: {
                            const tmp = index_1;
                            if (tmp >= 0) break :blk map + @as(usize, @intCast(tmp)) else break :blk map - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                        }).* & @as(c_uint, @bitCast(@as(c_int, 16777215))));
                    }
                }
                return;
            }
        }
    }
    info.*.codepoints[@as(c_uint, @intCast(@as(c_int, 0)))] = codepoint;
    info.*.n_codepoints = 1;
}

pub fn md_decode_utf8__(arg_str: [*c]const MD_CHAR, arg_str_size: MD_SIZE, arg_p_size: [*c]MD_SIZE) callconv(.C) c_uint {
    var str = arg_str;
    _ = &str;
    var str_size = arg_str_size;
    _ = &str_size;
    var p_size = arg_p_size;
    _ = &p_size;
    if (!(@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 0)))]))))) <= @as(c_int, 127))) {
        if ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 0)))]))))) & @as(c_int, 224)) == @as(c_int, 192)) {
            if ((@as(MD_SIZE, @bitCast(@as(c_int, 1))) < str_size) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) {
                if (p_size != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_size.* = 2;
                }
                return ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 0)))]))) & @as(c_uint, @bitCast(@as(c_int, 31)))) << @intCast(6)) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
            }
        } else if ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 0)))]))))) & @as(c_int, 240)) == @as(c_int, 224)) {
            if (((@as(MD_SIZE, @bitCast(@as(c_int, 2))) < str_size) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 2)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) {
                if (p_size != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_size.* = 3;
                }
                return (((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 0)))]))) & @as(c_uint, @bitCast(@as(c_int, 15)))) << @intCast(12)) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(6))) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 2)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
            }
        } else if ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 0)))]))))) & @as(c_int, 248)) == @as(c_int, 240)) {
            if ((((@as(MD_SIZE, @bitCast(@as(c_int, 3))) < str_size) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 2)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(str[@as(c_uint, @intCast(@as(c_int, 3)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) {
                if (p_size != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_size.* = 4;
                }
                return ((((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 0)))]))) & @as(c_uint, @bitCast(@as(c_int, 7)))) << @intCast(18)) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(12))) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 2)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(6))) | ((@as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 3)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
            }
        }
    }
    if (p_size != @as([*c]MD_SIZE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
        p_size.* = 1;
    }
    return @as(c_uint, @bitCast(@as(c_uint, str[@as(c_uint, @intCast(@as(c_int, 0)))])));
}

pub fn md_decode_utf8_before__(arg_ctx: [*c]MD_CTX, arg_off: MD_OFFSET) callconv(.C) c_uint {
    var ctx = arg_ctx;
    _ = &ctx;
    var off = arg_off;
    _ = &off;
    if (!(@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))))) <= @as(c_int, 127))) {
        if (((off > @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))))) & @as(c_int, 224)) == @as(c_int, 192))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) return ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) & @as(c_uint, @bitCast(@as(c_int, 31)))) << @intCast(6)) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
        if ((((off > @as(MD_OFFSET, @bitCast(@as(c_int, 2)))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 3)))]))))) & @as(c_int, 240)) == @as(c_int, 224))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) return (((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 3)))]))) & @as(c_uint, @bitCast(@as(c_int, 15)))) << @intCast(12)) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(6))) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
        if (((((off > @as(MD_OFFSET, @bitCast(@as(c_int, 3)))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 4)))]))))) & @as(c_int, 248)) == @as(c_int, 240))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 3)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) and ((@as(c_int, @bitCast(@as(c_uint, @as(u8, @bitCast(ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))))) & @as(c_int, 192)) == @as(c_int, 128))) return ((((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 4)))]))) & @as(c_uint, @bitCast(@as(c_int, 7)))) << @intCast(18)) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 3)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(12))) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(6))) | ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) & @as(c_uint, @bitCast(@as(c_int, 63)))) << @intCast(0));
    }
    return @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])));
}

pub fn md_decode_unicode(arg_str: [*c]const MD_CHAR, arg_off: MD_OFFSET, arg_str_size: MD_SIZE, arg_p_char_size: [*c]MD_SIZE) callconv(.C) c_uint {
    var str = arg_str;
    _ = &str;
    var off = arg_off;
    _ = &off;
    var str_size = arg_str_size;
    _ = &str_size;
    var p_char_size = arg_p_char_size;
    _ = &p_char_size;
    return md_decode_utf8__(str + off, str_size -% off, p_char_size);
}

pub fn md_merge_lines(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_end: MD_OFFSET, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_line_break_replacement_char: MD_CHAR, arg_buffer: [*c]MD_CHAR, arg_p_size: [*c]MD_SIZE) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var end = arg_end;
    _ = &end;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var line_break_replacement_char = arg_line_break_replacement_char;
    _ = &line_break_replacement_char;
    var buffer = arg_buffer;
    _ = &buffer;
    var p_size = arg_p_size;
    _ = &p_size;
    var ptr: [*c]MD_CHAR = buffer;
    _ = &ptr;
    var line_index: c_int = 0;
    _ = &line_index;
    var off: MD_OFFSET = beg;
    _ = &off;
    _ = &n_lines;
    while (true) {
        var line: [*c]const MD_LINE = &(blk: {
            const tmp = line_index;
            if (tmp >= 0) break :blk lines + @as(usize, @intCast(tmp)) else break :blk lines - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &line;
        var line_end: MD_OFFSET = line.*.end;
        _ = &line_end;
        if (end < line_end) {
            line_end = end;
        }
        while (off < line_end) {
            ptr.* = ctx.*.text[off];
            ptr += 1;
            off +%= 1;
        }
        if (off >= end) {
            p_size.* = @as(MD_SIZE, @bitCast(@as(c_int, @truncate(@divExact(@as(c_long, @bitCast(@intFromPtr(ptr) -% @intFromPtr(buffer))), @sizeOf(MD_CHAR))))));
            return;
        }
        ptr.* = line_break_replacement_char;
        ptr += 1;
        line_index += 1;
        off = (blk: {
            const tmp = line_index;
            if (tmp >= 0) break :blk lines + @as(usize, @intCast(tmp)) else break :blk lines - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*.beg;
    }
}

pub fn md_merge_lines_alloc(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_end: MD_OFFSET, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_line_break_replacement_char: MD_CHAR, arg_p_str: [*c][*c]MD_CHAR, arg_p_size: [*c]MD_SIZE) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var end = arg_end;
    _ = &end;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var line_break_replacement_char = arg_line_break_replacement_char;
    _ = &line_break_replacement_char;
    var p_str = arg_p_str;
    _ = &p_str;
    var p_size = arg_p_size;
    _ = &p_size;
    var buffer: [*c]MD_CHAR = undefined;
    _ = &buffer;
    buffer = @as([*c]MD_CHAR, @ptrCast(@alignCast(malloc(@sizeOf(MD_CHAR) *% @as(c_ulong, @bitCast(@as(c_ulong, end -% beg)))))));
    if (buffer == @as([*c]MD_CHAR, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
        while (true) {
            if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                ctx.*.parser.debug_log.?("malloc() failed.", ctx.*.userdata);
            }
            if (!false) break;
        }
        return -@as(c_int, 1);
    }
    md_merge_lines(ctx, beg, end, lines, n_lines, line_break_replacement_char, buffer, p_size);
    p_str.* = buffer;
    return 0;
}

pub fn md_skip_unicode_whitespace(arg_label: [*c]const MD_CHAR, arg_off: MD_OFFSET, arg_size: MD_SIZE) callconv(.C) MD_OFFSET {
    var label = arg_label;
    _ = &label;
    var off = arg_off;
    _ = &off;
    var size = arg_size;
    _ = &size;
    var char_size: MD_SIZE = undefined;
    _ = &char_size;
    var codepoint: c_uint = undefined;
    _ = &codepoint;
    while (off < size) {
        codepoint = md_decode_unicode(label, off, size, &char_size);
        if (!(md_is_unicode_whitespace__(codepoint) != 0) and !((@as(c_int, @bitCast(@as(c_uint, label[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, label[off]))) == @as(c_int, '\n')))) break;
        off +%= @as(MD_OFFSET, @bitCast(char_size));
    }
    return off;
}

pub fn md_is_html_tag(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    const max_end = arg_max_end;
    const p_end = arg_p_end;

    var attr_state: c_int = 0;
    var off: MD_OFFSET = arg_beg;
    var line_end: MD_OFFSET = if (n_lines > 0) lines[0].end else ctx.*.size;
    var line_index: MD_SIZE = 0;

    if (off +% 1 >= line_end)
        return FALSE;
    off += 1;

    if (ctx.*.text[off] == '/') {
        attr_state = -1;
        off += 1;
    }

    // Tag name
    if (off >= line_end or !((@as(c_uint, ctx.*.text[off]) >= 'A' and @as(c_uint, ctx.*.text[off]) <= 'Z') or (@as(c_uint, ctx.*.text[off]) >= 'a' and @as(c_uint, ctx.*.text[off]) <= 'z')))
        return FALSE;
    off += 1;
    while (off < line_end and (((@as(c_uint, ctx.*.text[off]) >= 'A' and @as(c_uint, ctx.*.text[off]) <= 'Z') or (@as(c_uint, ctx.*.text[off]) >= 'a' and @as(c_uint, ctx.*.text[off]) <= 'z') or (@as(c_uint, ctx.*.text[off]) >= '0' and @as(c_uint, ctx.*.text[off]) <= '9')) or ctx.*.text[off] == '-'))
        off += 1;

    // Attributes and closing
    while (true) {
        while (off < line_end and !(ctx.*.text[off] == '\r' or ctx.*.text[off] == '\n')) {
            if (attr_state > 40) {
                if (attr_state == 41 and ((ctx.*.text[off] == ' ' or ctx.*.text[off] == '\t') or (ctx.*.text[off] != 0 and isInPalette(ctx.*.text[off], "\"'=<>`")))) {
                    attr_state = 0;
                    off -%= 1;
                } else if (attr_state == 42 and ctx.*.text[off] == '\'') {
                    attr_state = 0;
                } else if (attr_state == 43 and ctx.*.text[off] == '"') {
                    attr_state = 0;
                }
                off += 1;
            } else if (ctx.*.text[off] == ' ' or ctx.*.text[off] == '\t' or ctx.*.text[off] == 0x0B or ctx.*.text[off] == 0x0C) {
                if (attr_state == 0)
                    attr_state = 1;
                off += 1;
            } else if (attr_state <= 2 and ctx.*.text[off] == '>') {
                // End
                if (off >= max_end) return FALSE;
                p_end.* = off + 1;
                return TRUE;
            } else if (attr_state <= 2 and ctx.*.text[off] == '/' and off + 1 < line_end and ctx.*.text[off + 1] == '>') {
                off += 1;
                if (off >= max_end) return FALSE;
                p_end.* = off + 1;
                return TRUE;
            } else if ((attr_state == 1 or attr_state == 2) and ((@as(c_uint, ctx.*.text[off]) >= 'A' and @as(c_uint, ctx.*.text[off]) <= 'Z') or (@as(c_uint, ctx.*.text[off]) >= 'a' and @as(c_uint, ctx.*.text[off]) <= 'z') or ctx.*.text[off] == '_' or ctx.*.text[off] == ':')) {
                off += 1;
                while (off < line_end and (((@as(c_uint, ctx.*.text[off]) >= 'A' and @as(c_uint, ctx.*.text[off]) <= 'Z') or (@as(c_uint, ctx.*.text[off]) >= 'a' and @as(c_uint, ctx.*.text[off]) <= 'z') or (@as(c_uint, ctx.*.text[off]) >= '0' and @as(c_uint, ctx.*.text[off]) <= '9')) or ctx.*.text[off] != 0 and isInPalette(ctx.*.text[off], "_.:-")))
                    off += 1;
                attr_state = 2;
            } else if (attr_state == 2 and ctx.*.text[off] == '=') {
                off += 1;
                attr_state = 3;
            } else if (attr_state == 3) {
                if (ctx.*.text[off] == '"')
                    attr_state = 43
                else if (ctx.*.text[off] == '\'')
                    attr_state = 42
                else if (!(ctx.*.text[off] != 0 and isInPalette(ctx.*.text[off], "\"'=<>`")) and !(ctx.*.text[off] == '\r' or ctx.*.text[off] == '\n'))
                    attr_state = 41
                else
                    return FALSE;
                off += 1;
            } else {
                return FALSE;
            }
        }

        if (n_lines == 0)
            return FALSE;

        line_index += 1;
        if (line_index >= n_lines)
            return FALSE;

        off = lines[line_index].beg;
        line_end = lines[line_index].end;

        if (attr_state == 0 or attr_state == 41)
            attr_state = 1;

        if (off >= max_end)
            return FALSE;
    }
}

fn isInPalette(ch: u8, palette: []const u8) bool {
    for (palette) |p| {
        if (p == ch) return true;
    }
    return false;
}

pub fn md_scan_for_html_closer(arg_ctx: [*c]MD_CTX, arg_str: [*c]const MD_CHAR, arg_len: MD_SIZE, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_scan_horizon: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var str = arg_str;
    _ = &str;
    var len = arg_len;
    _ = &len;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_scan_horizon = arg_p_scan_horizon;
    _ = &p_scan_horizon;
    var off: MD_OFFSET = beg;
    _ = &off;
    var line_index: MD_SIZE = 0;
    _ = &line_index;
    if ((off < p_scan_horizon.*) and (p_scan_horizon.* >= (max_end -% len))) {
        return 0;
    }
    while (true) {
        while (((off +% len) <= lines[line_index].end) and ((off +% len) <= max_end)) {
            if (md_ascii_eq(ctx.*.text + off, str, len) != 0) {
                p_end.* = off +% len;
                return 1;
            }
            off +%= 1;
        }
        line_index +%= 1;
        if ((off >= max_end) or (line_index >= n_lines)) {
            p_scan_horizon.* = off;
            return 0;
        }
        off = lines[line_index].beg;
    }
    return 0;
}

pub fn md_is_html_comment(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '<'))) {
            unreachable;
        }
        if (!false) break;
    }
    if ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 4)))) >= lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) return 0;
    if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '!')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) != @as(c_int, '-'))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 3)))]))) != @as(c_int, '-'))) return 0;
    off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
    return md_scan_for_html_closer(ctx, "-->", @as(MD_SIZE, @bitCast(@as(c_int, 3))), lines, n_lines, off, max_end, p_end, &ctx.*.html_comment_horizon);
}

pub fn md_is_html_processing_instruction(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    if ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))) >= lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) return 0;
    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '?')) return 0;
    off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
    return md_scan_for_html_closer(ctx, "?>", @as(MD_SIZE, @bitCast(@as(c_int, 2))), lines, n_lines, off, max_end, p_end, &ctx.*.html_proc_instr_horizon);
}

pub fn md_is_html_declaration(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    if ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))) >= lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) return 0;
    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '!')) return 0;
    off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
    if ((off >= lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) or !(((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z'))))))) return 0;
    off +%= 1;
    while ((off < lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) and (((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z'))))))) {
        off +%= 1;
    }
    return md_scan_for_html_closer(ctx, ">", @as(MD_SIZE, @bitCast(@as(c_int, 1))), lines, n_lines, off, max_end, p_end, &ctx.*.html_decl_horizon);
}

pub fn md_is_html_cdata(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    const open_str = struct {
        const static: [9:0]MD_CHAR = "<![CDATA[".*;
    };
    _ = &open_str;
    const open_size = struct {
        const static: MD_SIZE = @as(MD_SIZE, @bitCast(@as(c_uint, @truncate((@sizeOf([10]MD_CHAR) / @sizeOf(MD_CHAR)) -% @as(c_ulong, @bitCast(@as(c_long, @as(c_int, 1))))))));
    };
    _ = &open_size;
    var off: MD_OFFSET = beg;
    _ = &off;
    if ((off +% open_size.static) >= lines[@as(c_uint, @intCast(@as(c_int, 0)))].end) return 0;
    if (memcmp(@as(?*const anyopaque, @ptrCast(ctx.*.text + off)), @as(?*const anyopaque, @ptrCast(@as([*c]const MD_CHAR, @ptrCast(@alignCast(&open_str.static))))), @as(c_ulong, @bitCast(@as(c_ulong, open_size.static)))) != @as(c_int, 0)) return 0;
    off +%= @as(MD_OFFSET, @bitCast(open_size.static));
    return md_scan_for_html_closer(ctx, "]]>", @as(MD_SIZE, @bitCast(@as(c_int, 3))), lines, n_lines, off, max_end, p_end, &ctx.*.html_cdata_horizon);
}

pub fn md_is_html_any(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '<'))) {
            unreachable;
        }
        if (!false) break;
    }
    return @intFromBool(((((md_is_html_tag(ctx, lines, n_lines, beg, max_end, p_end) != 0) or (md_is_html_comment(ctx, lines, n_lines, beg, max_end, p_end) != 0)) or (md_is_html_processing_instruction(ctx, lines, n_lines, beg, max_end, p_end) != 0)) or (md_is_html_declaration(ctx, lines, n_lines, beg, max_end, p_end) != 0)) or (md_is_html_cdata(ctx, lines, n_lines, beg, max_end, p_end) != 0));
}

pub fn md_is_hex_entity_contents(arg_ctx: [*c]MD_CTX, arg_text: [*c]const MD_CHAR, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var text = arg_text;
    _ = &text;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    _ = &ctx;
    while (((off < max_end) and ((((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'F')))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'f'))))))) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 8))))) {
        off +%= 1;
    }
    if ((@as(MD_OFFSET, @bitCast(@as(c_int, 1))) <= (off -% beg)) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 6))))) {
        p_end.* = off;
        return 1;
    } else {
        return 0;
    }
    return 0;
}

pub fn md_is_dec_entity_contents(arg_ctx: [*c]MD_CTX, arg_text: [*c]const MD_CHAR, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var text = arg_text;
    _ = &text;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    _ = &ctx;
    while (((off < max_end) and ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 8))))) {
        off +%= 1;
    }
    if ((@as(MD_OFFSET, @bitCast(@as(c_int, 1))) <= (off -% beg)) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 7))))) {
        p_end.* = off;
        return 1;
    } else {
        return 0;
    }
    return 0;
}

pub fn md_is_named_entity_contents(arg_ctx: [*c]MD_CTX, arg_text: [*c]const MD_CHAR, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var text = arg_text;
    _ = &text;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    _ = &ctx;
    if ((off < max_end) and (((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z'))))))) {
        off +%= 1;
    } else return 0;
    while (((off < max_end) and ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9'))))))) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 48))))) {
        off +%= 1;
    }
    if ((@as(MD_OFFSET, @bitCast(@as(c_int, 2))) <= (off -% beg)) and ((off -% beg) <= @as(MD_OFFSET, @bitCast(@as(c_int, 48))))) {
        p_end.* = off;
        return 1;
    } else {
        return 0;
    }
    return 0;
}

pub fn md_is_entity_str(arg_ctx: [*c]MD_CTX, arg_text: [*c]const MD_CHAR, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var text = arg_text;
    _ = &text;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var is_contents: c_int = undefined;
    _ = &is_contents;
    var off: MD_OFFSET = beg;
    _ = &off;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, text[off]))) == @as(c_int, '&'))) {
            unreachable;
        }
        if (!false) break;
    }
    off +%= 1;
    if ((((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))) < max_end) and (@as(c_int, @bitCast(@as(c_uint, text[off]))) == @as(c_int, '#'))) and ((@as(c_int, @bitCast(@as(c_uint, text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, 'x')) or (@as(c_int, @bitCast(@as(c_uint, text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, 'X')))) {
        is_contents = md_is_hex_entity_contents(ctx, text, off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2))), max_end, &off);
    } else if (((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < max_end) and (@as(c_int, @bitCast(@as(c_uint, text[off]))) == @as(c_int, '#'))) {
        is_contents = md_is_dec_entity_contents(ctx, text, off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1))), max_end, &off);
    } else {
        is_contents = md_is_named_entity_contents(ctx, text, off, max_end, &off);
    }
    if (((is_contents != 0) and (off < max_end)) and (@as(c_int, @bitCast(@as(c_uint, text[off]))) == @as(c_int, ';'))) {
        p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
        return 1;
    } else {
        return 0;
    }
    return 0;
}

pub fn md_is_entity(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    return md_is_entity_str(ctx, ctx.*.text, beg, max_end, p_end);
}

pub fn md_build_attr_append_substr(arg_ctx: [*c]MD_CTX, arg_build: [*c]MD_ATTRIBUTE_BUILD, arg_type: MD_TEXTTYPE, arg_off: MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var build = arg_build;
    _ = &build;
    var @"type" = arg_type;
    _ = &@"type";
    var off = arg_off;
    _ = &off;
    if (build.*.substr_count >= build.*.substr_alloc) {
        var new_substr_types: [*c]MD_TEXTTYPE = undefined;
        _ = &new_substr_types;
        var new_substr_offsets: [*c]MD_OFFSET = undefined;
        _ = &new_substr_offsets;
        build.*.substr_alloc = if (build.*.substr_alloc > @as(c_int, 0)) build.*.substr_alloc + @divTrunc(build.*.substr_alloc, @as(c_int, 2)) else @as(c_int, 8);
        new_substr_types = @as([*c]MD_TEXTTYPE, @ptrCast(@alignCast(realloc(@as(?*anyopaque, @ptrCast(build.*.substr_types)), @as(c_ulong, @bitCast(@as(c_long, build.*.substr_alloc))) *% @sizeOf(MD_TEXTTYPE)))));
        if (new_substr_types == @as([*c]MD_TEXTTYPE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("realloc() failed.", ctx.*.userdata);
                }
                if (!false) break;
            }
            return -@as(c_int, 1);
        }
        new_substr_offsets = @as([*c]MD_OFFSET, @ptrCast(@alignCast(realloc(@as(?*anyopaque, @ptrCast(build.*.substr_offsets)), @as(c_ulong, @bitCast(@as(c_long, build.*.substr_alloc + @as(c_int, 1)))) *% @sizeOf(MD_OFFSET)))));
        if (new_substr_offsets == @as([*c]MD_OFFSET, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("realloc() failed.", ctx.*.userdata);
                }
                if (!false) break;
            }
            free(@as(?*anyopaque, @ptrCast(new_substr_types)));
            return -@as(c_int, 1);
        }
        build.*.substr_types = new_substr_types;
        build.*.substr_offsets = new_substr_offsets;
    }
    (blk: {
        const tmp = build.*.substr_count;
        if (tmp >= 0) break :blk build.*.substr_types + @as(usize, @intCast(tmp)) else break :blk build.*.substr_types - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).* = @"type";
    (blk: {
        const tmp = build.*.substr_count;
        if (tmp >= 0) break :blk build.*.substr_offsets + @as(usize, @intCast(tmp)) else break :blk build.*.substr_offsets - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).* = off;
    build.*.substr_count += 1;
    return 0;
}

pub fn md_free_attribute(arg_ctx: [*c]MD_CTX, arg_build: [*c]MD_ATTRIBUTE_BUILD) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var build = arg_build;
    _ = &build;
    _ = &ctx;
    if (build.*.substr_alloc > @as(c_int, 0)) {
        free(@as(?*anyopaque, @ptrCast(build.*.text)));
        free(@as(?*anyopaque, @ptrCast(build.*.substr_types)));
        free(@as(?*anyopaque, @ptrCast(build.*.substr_offsets)));
    }
}

pub fn md_build_attribute(arg_ctx: [*c]MD_CTX, arg_raw_text: [*c]const MD_CHAR, arg_raw_size: MD_SIZE, arg_flags: c_uint, arg_attr: [*c]MD_ATTRIBUTE, arg_build: [*c]MD_ATTRIBUTE_BUILD) callconv(.C) c_int {
    const ctx = arg_ctx;
    const raw_text = arg_raw_text;
    const raw_size = arg_raw_size;
    const flags = arg_flags;
    const attr = arg_attr;
    const build = arg_build;
    var raw_off: MD_OFFSET = undefined;
    var off: MD_OFFSET = undefined;
    var ret: c_int = 0;

    _ = memset(@ptrCast(build), 0, @sizeOf(MD_ATTRIBUTE_BUILD));

    // Check if trivial (no backslash, no ampersand, no null)
    var is_trivial: c_int = TRUE;
    raw_off = 0;
    while (raw_off < raw_size) : (raw_off += 1) {
        if (raw_text[raw_off] == '\\' or raw_text[raw_off] == '&' or raw_text[raw_off] == 0) {
            is_trivial = FALSE;
            break;
        }
    }

    if (is_trivial != 0) {
        build.*.text = if (raw_size != 0) @constCast(raw_text) else @as([*c]MD_CHAR, @ptrFromInt(0));
        build.*.substr_types = &build.*.trivial_types;
        build.*.substr_offsets = &build.*.trivial_offsets;
        build.*.substr_count = 1;
        build.*.substr_alloc = 0;
        build.*.trivial_types[0] = MD_TEXT_NORMAL;
        build.*.trivial_offsets[0] = 0;
        build.*.trivial_offsets[1] = raw_size;
        off = raw_size;
    } else {
        build.*.text = @ptrCast(@alignCast(malloc(raw_size)));
        if (build.*.text == @as([*c]MD_CHAR, @ptrFromInt(0))) {
            md_log(ctx, "malloc() failed.");
            md_free_attribute(ctx, build);
            return -1;
        }

        raw_off = 0;
        off = 0;

        abort: {
            while (raw_off < raw_size) {
                if (raw_text[raw_off] == 0) {
                    ret = md_build_attr_append_substr(ctx, build, MD_TEXT_NULLCHAR, off);
                    if (ret < 0) break :abort;
                    _ = memcpy(@ptrCast(build.*.text + off), @ptrCast(raw_text + raw_off), 1);
                    off += 1;
                    raw_off += 1;
                    continue;
                }

                if (raw_text[raw_off] == '&') {
                    var ent_end: MD_OFFSET = undefined;
                    if (md_is_entity_str(ctx, raw_text, raw_off, raw_size, &ent_end) != 0) {
                        ret = md_build_attr_append_substr(ctx, build, MD_TEXT_ENTITY, off);
                        if (ret < 0) break :abort;
                        _ = memcpy(@ptrCast(build.*.text + off), @ptrCast(raw_text + raw_off), ent_end -% raw_off);
                        off += ent_end -% raw_off;
                        raw_off = ent_end;
                        continue;
                    }
                }

                if (build.*.substr_count == 0 or build.*.substr_types[@intCast(build.*.substr_count - 1)] != MD_TEXT_NORMAL) {
                    ret = md_build_attr_append_substr(ctx, build, MD_TEXT_NORMAL, off);
                    if (ret < 0) break :abort;
                }

                if ((flags & MD_BUILD_ATTR_NO_ESCAPES) == 0 and
                    raw_text[raw_off] == '\\' and raw_off + 1 < raw_size and
                    (ISPUNCT_c(raw_text[raw_off + 1]) or raw_text[raw_off + 1] == '\r' or raw_text[raw_off + 1] == '\n'))
                    raw_off += 1;

                build.*.text[off] = raw_text[raw_off];
                off += 1;
                raw_off += 1;
            }
            build.*.substr_offsets[@intCast(build.*.substr_count)] = off;

            attr.*.text = build.*.text;
            attr.*.size = off;
            attr.*.substr_offsets = build.*.substr_offsets;
            attr.*.substr_types = build.*.substr_types;
            return 0;
        }

        md_free_attribute(ctx, build);
        return -1;
    }

    attr.*.text = build.*.text;
    attr.*.size = off;
    attr.*.substr_offsets = build.*.substr_offsets;
    attr.*.substr_types = build.*.substr_types;
    return 0;
}

fn ISPUNCT_c(ch: u8) bool {
    return (ch >= 33 and ch <= 47) or (ch >= 58 and ch <= 64) or (ch >= 91 and ch <= 96) or (ch >= 123 and ch <= 126);
}

fn md_log(ctx: [*c]MD_CTX, msg: [*:0]const u8) void {
    if (ctx.*.parser.debug_log) |log_fn| {
        log_fn(msg, ctx.*.userdata);
    }
}

pub fn md_fnv1a(arg_base: c_uint, arg_data: ?*const anyopaque, arg_n: usize) callconv(.C) c_uint {
    var base = arg_base;
    _ = &base;
    var data = arg_data;
    _ = &data;
    var n = arg_n;
    _ = &n;
    var buf: [*c]const u8 = @as([*c]const u8, @ptrCast(@alignCast(data)));
    _ = &buf;
    var hash: c_uint = base;
    _ = &hash;
    var i: usize = undefined;
    _ = &i;
    {
        i = 0;
        while (i < n) : (i +%= 1) {
            hash ^= @as(c_uint, @bitCast(@as(c_uint, buf[i])));
            hash *%= @as(c_uint, 16777619);
        }
    }
    return hash;
}

pub fn md_link_label_hash(arg_label: [*c]const MD_CHAR, arg_size: MD_SIZE) callconv(.C) c_uint {
    var label = arg_label;
    _ = &label;
    var size = arg_size;
    _ = &size;
    var hash: c_uint = 2166136261;
    _ = &hash;
    var off: MD_OFFSET = undefined;
    _ = &off;
    var codepoint: c_uint = undefined;
    _ = &codepoint;
    var is_whitespace: c_int = 0;
    _ = &is_whitespace;
    off = md_skip_unicode_whitespace(label, @as(MD_OFFSET, @bitCast(@as(c_int, 0))), size);
    while (off < size) {
        var char_size: MD_SIZE = undefined;
        _ = &char_size;
        codepoint = md_decode_unicode(label, off, size, &char_size);
        is_whitespace = @intFromBool((md_is_unicode_whitespace__(codepoint) != 0) or ((@as(c_int, @bitCast(@as(c_uint, label[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, label[off]))) == @as(c_int, '\n'))));
        if (is_whitespace != 0) {
            codepoint = ' ';
            hash = md_fnv1a(hash, @as(?*const anyopaque, @ptrCast(&codepoint)), @sizeOf(c_uint));
            off = md_skip_unicode_whitespace(label, off, size);
        } else {
            var fold_info: MD_UNICODE_FOLD_INFO = undefined;
            _ = &fold_info;
            md_get_unicode_fold_info(codepoint, &fold_info);
            hash = md_fnv1a(hash, @as(?*const anyopaque, @ptrCast(@as([*c]c_uint, @ptrCast(@alignCast(&fold_info.codepoints))))), @as(c_ulong, @bitCast(@as(c_ulong, fold_info.n_codepoints))) *% @sizeOf(c_uint));
            off +%= @as(MD_OFFSET, @bitCast(char_size));
        }
    }
    return hash;
}

pub fn md_link_label_cmp_load_fold_info(arg_label: [*c]const MD_CHAR, arg_off: MD_OFFSET, arg_size: MD_SIZE, arg_fold_info: [*c]MD_UNICODE_FOLD_INFO) callconv(.C) MD_OFFSET {
    const label = arg_label;
    var off = arg_off;
    const size = arg_size;
    const fold_info = arg_fold_info;

    if (off >= size) {
        // Treat end of a link label as whitespace
        fold_info.*.codepoints[0] = ' ';
        fold_info.*.n_codepoints = 1;
        return md_skip_unicode_whitespace(label, off, size);
    }

    var char_size: MD_SIZE = undefined;
    const codepoint = md_decode_unicode(label, off, size, &char_size);
    off +%= char_size;
    if (md_is_unicode_whitespace__(codepoint) != 0) {
        // Treat all whitespace as equivalent
        fold_info.*.codepoints[0] = ' ';
        fold_info.*.n_codepoints = 1;
        return md_skip_unicode_whitespace(label, off, size);
    }

    // Get real folding info
    md_get_unicode_fold_info(codepoint, fold_info);
    return off;
}

pub fn md_link_label_cmp(arg_a_label: [*c]const MD_CHAR, arg_a_size: MD_SIZE, arg_b_label: [*c]const MD_CHAR, arg_b_size: MD_SIZE) callconv(.C) c_int {
    var a_label = arg_a_label;
    _ = &a_label;
    var a_size = arg_a_size;
    _ = &a_size;
    var b_label = arg_b_label;
    _ = &b_label;
    var b_size = arg_b_size;
    _ = &b_size;
    var a_off: MD_OFFSET = undefined;
    _ = &a_off;
    var b_off: MD_OFFSET = undefined;
    _ = &b_off;
    var a_fi: MD_UNICODE_FOLD_INFO = MD_UNICODE_FOLD_INFO{
        .codepoints = [1]c_uint{
            0,
        } ++ [1]c_uint{0} ** 2,
        .n_codepoints = @as(c_uint, @bitCast(@as(c_int, 0))),
    };
    _ = &a_fi;
    var b_fi: MD_UNICODE_FOLD_INFO = MD_UNICODE_FOLD_INFO{
        .codepoints = [1]c_uint{
            0,
        } ++ [1]c_uint{0} ** 2,
        .n_codepoints = @as(c_uint, @bitCast(@as(c_int, 0))),
    };
    _ = &b_fi;
    var a_fi_off: MD_OFFSET = 0;
    _ = &a_fi_off;
    var b_fi_off: MD_OFFSET = 0;
    _ = &b_fi_off;
    var cmp: c_int = undefined;
    _ = &cmp;
    a_off = md_skip_unicode_whitespace(a_label, @as(MD_OFFSET, @bitCast(@as(c_int, 0))), a_size);
    b_off = md_skip_unicode_whitespace(b_label, @as(MD_OFFSET, @bitCast(@as(c_int, 0))), b_size);
    while ((((a_off < a_size) or (a_fi_off < a_fi.n_codepoints)) or (b_off < b_size)) or (b_fi_off < b_fi.n_codepoints)) {
        if (a_fi_off >= a_fi.n_codepoints) {
            a_fi_off = 0;
            a_off = md_link_label_cmp_load_fold_info(a_label, a_off, a_size, &a_fi);
        }
        if (b_fi_off >= b_fi.n_codepoints) {
            b_fi_off = 0;
            b_off = md_link_label_cmp_load_fold_info(b_label, b_off, b_size, &b_fi);
        }
        cmp = @as(c_int, @bitCast(b_fi.codepoints[b_fi_off] -% a_fi.codepoints[a_fi_off]));
        if (cmp != @as(c_int, 0)) return cmp;
        a_fi_off +%= 1;
        b_fi_off +%= 1;
    }
    return 0;
}

pub fn md_ref_def_cmp(arg_a: ?*const anyopaque, arg_b: ?*const anyopaque) callconv(.C) c_int {
    var a = arg_a;
    _ = &a;
    var b = arg_b;
    _ = &b;
    var a_ref: ?*const MD_REF_DEF = @as([*c]?*const MD_REF_DEF, @ptrCast(@alignCast(@volatileCast(@constCast(a))))).*;
    _ = &a_ref;
    var b_ref: ?*const MD_REF_DEF = @as([*c]?*const MD_REF_DEF, @ptrCast(@alignCast(@volatileCast(@constCast(b))))).*;
    _ = &b_ref;
    if (a_ref.?.hash < b_ref.?.hash) return -@as(c_int, 1) else if (a_ref.?.hash > b_ref.?.hash) return @as(c_int, 1) else return md_link_label_cmp(a_ref.?.label, a_ref.?.label_size, b_ref.?.label, b_ref.?.label_size);
    return 0;
}

pub fn md_ref_def_cmp_for_sort(arg_a: ?*const anyopaque, arg_b: ?*const anyopaque) callconv(.C) c_int {
    var a = arg_a;
    _ = &a;
    var b = arg_b;
    _ = &b;
    var cmp: c_int = undefined;
    _ = &cmp;
    cmp = md_ref_def_cmp(a, b);
    if (cmp == @as(c_int, 0)) {
        var a_ref: ?*const MD_REF_DEF = @as([*c]?*const MD_REF_DEF, @ptrCast(@alignCast(@volatileCast(@constCast(a))))).*;
        _ = &a_ref;
        var b_ref: ?*const MD_REF_DEF = @as([*c]?*const MD_REF_DEF, @ptrCast(@alignCast(@volatileCast(@constCast(b))))).*;
        _ = &b_ref;
        if (@intFromPtr(a_ref) < @intFromPtr(b_ref)) {
            cmp = -@as(c_int, 1);
        } else if (@intFromPtr(a_ref) > @intFromPtr(b_ref)) {
            cmp = @as(c_int, 1);
        } else {
            cmp = 0;
        }
    }
    return cmp;
}

pub fn md_build_ref_def_hashtable(arg_ctx: [*c]MD_CTX) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = 0;

    if (ctx.*.n_ref_defs == 0)
        return 0;

    ret = abort: {
        ctx.*.ref_def_hashtable_size = @divTrunc(ctx.*.n_ref_defs * @as(c_int, 5), @as(c_int, 4));
        ctx.*.ref_def_hashtable = @ptrCast(@alignCast(malloc(@as(usize, @intCast(ctx.*.ref_def_hashtable_size)) * @sizeOf(?*anyopaque))));
        if (ctx.*.ref_def_hashtable == @as([*c]?*anyopaque, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            md_log(ctx, "malloc() failed.");
            break :abort -1;
        }
        _ = memset(@ptrCast(ctx.*.ref_def_hashtable), 0, @as(usize, @intCast(ctx.*.ref_def_hashtable_size)) * @sizeOf(?*anyopaque));

        {
            var i: c_int = 0;
            while (i < ctx.*.n_ref_defs) : (i += 1) {
                const ref_defs_ptr: [*c]MD_REF_DEF = @ptrCast(ctx.*.ref_defs);
                const def: *MD_REF_DEF = @ptrCast(&ref_defs_ptr[@intCast(i)]);
                var bucket: ?*anyopaque = undefined;
                var list: *MD_REF_DEF_LIST = undefined;

                def.hash = md_link_label_hash(def.label, def.label_size);
                const bucket_idx: usize = @intCast(@mod(def.hash, @as(c_uint, @intCast(ctx.*.ref_def_hashtable_size))));
                bucket = ctx.*.ref_def_hashtable[bucket_idx];

                if (bucket == null) {
                    ctx.*.ref_def_hashtable[bucket_idx] = @ptrCast(def);
                    continue;
                }

                const ref_defs_start: usize = @intFromPtr(ctx.*.ref_defs.?);
                const ref_defs_end: usize = ref_defs_start + @as(usize, @intCast(ctx.*.n_ref_defs)) * @sizeOf(MD_REF_DEF);
                const bucket_addr: usize = @intFromPtr(bucket.?);

                if (bucket_addr >= ref_defs_start and bucket_addr < ref_defs_end) {
                    const old_def: *MD_REF_DEF = @ptrCast(@alignCast(bucket.?));

                    if (md_link_label_cmp(def.label, def.label_size, old_def.label, old_def.label_size) == 0) {
                        continue;
                    }

                    const list_ptr = malloc(@sizeOf(MD_REF_DEF_LIST) + 2 * @sizeOf(*MD_REF_DEF));
                    if (list_ptr == null) {
                        md_log(ctx, "malloc() failed.");
                        break :abort -1;
                    }
                    list = @ptrCast(@alignCast(list_ptr));
                    list.ref_defs()[0] = old_def;
                    list.ref_defs()[1] = def;
                    list.n_ref_defs = 2;
                    list.alloc_ref_defs = 2;
                    ctx.*.ref_def_hashtable[bucket_idx] = @ptrCast(list);
                    continue;
                }

                list = @ptrCast(@alignCast(bucket.?));
                if (list.n_ref_defs >= list.alloc_ref_defs) {
                    const alloc_ref_defs: c_int = list.alloc_ref_defs + @divTrunc(list.alloc_ref_defs, @as(c_int, 2));
                    const list_tmp = realloc(@ptrCast(list), @sizeOf(MD_REF_DEF_LIST) + @as(usize, @intCast(alloc_ref_defs)) * @sizeOf(*MD_REF_DEF));
                    if (list_tmp == null) {
                        md_log(ctx, "realloc() failed.");
                        break :abort -1;
                    }
                    list = @ptrCast(@alignCast(list_tmp));
                    list.alloc_ref_defs = alloc_ref_defs;
                    ctx.*.ref_def_hashtable[bucket_idx] = @ptrCast(list);
                }

                list.ref_defs()[@intCast(list.n_ref_defs)] = def;
                list.n_ref_defs += 1;
            }
        }

        // Sort the complex buckets so we can use bsearch() with them.
        {
            var i: c_int = 0;
            while (i < ctx.*.ref_def_hashtable_size) : (i += 1) {
                const bucket: ?*anyopaque = ctx.*.ref_def_hashtable[@intCast(i)];
                var list: *MD_REF_DEF_LIST = undefined;

                if (bucket == null)
                    continue;

                const ref_defs_start: usize = @intFromPtr(ctx.*.ref_defs.?);
                const ref_defs_end: usize = ref_defs_start + @as(usize, @intCast(ctx.*.n_ref_defs)) * @sizeOf(MD_REF_DEF);
                const bucket_addr: usize = @intFromPtr(bucket.?);

                if (bucket_addr >= ref_defs_start and bucket_addr < ref_defs_end)
                    continue;

                list = @ptrCast(@alignCast(bucket.?));
                qsort(@ptrCast(list.ref_defs()), @intCast(list.n_ref_defs), @sizeOf(*MD_REF_DEF), md_ref_def_cmp_for_sort);

                {
                    var j: c_int = 1;
                    while (j < list.n_ref_defs) : (j += 1) {
                        if (md_ref_def_cmp(@ptrCast(&list.ref_defs()[@intCast(j - 1)]), @ptrCast(&list.ref_defs()[@intCast(j)])) == 0)
                            list.ref_defs()[@intCast(j)] = list.ref_defs()[@intCast(j - 1)];
                    }
                }
            }
        }

        break :abort 0;
    };

    return ret;
}

pub fn md_free_ref_def_hashtable(arg_ctx: [*c]MD_CTX) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    if (ctx.*.ref_def_hashtable != @as([*c]?*anyopaque, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
        var i: c_int = undefined;
        _ = &i;
        {
            i = 0;
            while (i < ctx.*.ref_def_hashtable_size) : (i += 1) {
                var bucket: ?*anyopaque = (blk: {
                    const tmp = i;
                    if (tmp >= 0) break :blk ctx.*.ref_def_hashtable + @as(usize, @intCast(tmp)) else break :blk ctx.*.ref_def_hashtable - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*;
                _ = &bucket;
                if (bucket == @as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))) continue;
                {
                    const bucket_addr = @intFromPtr(bucket);
                    const ref_defs_start = @intFromPtr(ctx.*.ref_defs);
                    const ref_defs_end = @intFromPtr(@as([*c]MD_REF_DEF, @ptrCast(ctx.*.ref_defs)) + @as(usize, @intCast(ctx.*.n_ref_defs)));
                    if (ref_defs_start <= bucket_addr and bucket_addr < ref_defs_end) continue;
                }
                free(bucket);
            }
        }
        free(@as(?*anyopaque, @ptrCast(ctx.*.ref_def_hashtable)));
    }
}

pub fn md_lookup_ref_def(arg_ctx: [*c]MD_CTX, arg_label: [*c]const MD_CHAR, arg_label_size: MD_SIZE) callconv(.C) ?*const MD_REF_DEF {
    const ctx = arg_ctx;
    const label = arg_label;
    const label_size = arg_label_size;

    if (ctx.*.ref_def_hashtable_size == 0)
        return null;

    const hash: c_uint = md_link_label_hash(label, label_size);
    const bucket_idx: usize = @intCast(@mod(hash, @as(c_uint, @intCast(ctx.*.ref_def_hashtable_size))));
    const bucket: ?*anyopaque = ctx.*.ref_def_hashtable[bucket_idx];

    if (bucket == null) {
        return null;
    }

    const ref_defs_start: usize = @intFromPtr(ctx.*.ref_defs.?);
    const ref_defs_end: usize = ref_defs_start + @as(usize, @intCast(ctx.*.n_ref_defs)) * @sizeOf(MD_REF_DEF);
    const bucket_addr: usize = @intFromPtr(bucket.?);

    if (bucket_addr >= ref_defs_start and bucket_addr < ref_defs_end) {
        const def: *const MD_REF_DEF = @ptrCast(@alignCast(bucket.?));
        if (md_link_label_cmp(def.label, def.label_size, label, label_size) == 0)
            return def
        else
            return null;
    } else {
        const list: *MD_REF_DEF_LIST = @ptrCast(@alignCast(bucket.?));
        var key_buf: MD_REF_DEF = @import("std").mem.zeroes(MD_REF_DEF);
        key_buf.label = @constCast(label);
        key_buf.label_size = label_size;
        key_buf.hash = md_link_label_hash(key_buf.label, key_buf.label_size);
        const key: *const MD_REF_DEF = &key_buf;

        const ret_ptr = bsearch(@ptrCast(&key), @ptrCast(list.ref_defs()), @intCast(list.n_ref_defs), @sizeOf(*MD_REF_DEF), md_ref_def_cmp);
        if (ret_ptr != null) {
            const ret: *const *const MD_REF_DEF = @ptrCast(@alignCast(ret_ptr));
            return ret.*;
        } else {
            return null;
        }
    }
}

pub fn md_is_link_label(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_beg_line_index: [*c]MD_SIZE, arg_p_end_line_index: [*c]MD_SIZE, arg_p_contents_beg: [*c]MD_OFFSET, arg_p_contents_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_beg_line_index = arg_p_beg_line_index;
    _ = &p_beg_line_index;
    var p_end_line_index = arg_p_end_line_index;
    _ = &p_end_line_index;
    var p_contents_beg = arg_p_contents_beg;
    _ = &p_contents_beg;
    var p_contents_end = arg_p_contents_end;
    _ = &p_contents_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    var contents_beg: MD_OFFSET = 0;
    _ = &contents_beg;
    var contents_end: MD_OFFSET = 0;
    _ = &contents_end;
    var line_index: MD_SIZE = 0;
    _ = &line_index;
    var len: c_int = 0;
    _ = &len;
    p_beg_line_index.* = 0;
    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '[')) return 0;
    off +%= 1;
    while (true) {
        var line_end: MD_OFFSET = lines[line_index].end;
        _ = &line_end;
        while (off < line_end) {
            if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\\')) and ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < ctx.*.size)) and ((((((@as(c_uint, @bitCast(@as(c_int, 33))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 47))))) or ((@as(c_uint, @bitCast(@as(c_int, 58))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 64)))))) or ((@as(c_uint, @bitCast(@as(c_int, 91))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 96)))))) or ((@as(c_uint, @bitCast(@as(c_int, 123))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 126)))))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\n'))))) {
                if (contents_end == @as(MD_OFFSET, @bitCast(@as(c_int, 0)))) {
                    contents_beg = off;
                    p_beg_line_index.* = line_index;
                }
                contents_end = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
                off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
            } else if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '[')) {
                return 0;
            } else if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ']')) {
                if (contents_beg < contents_end) {
                    p_contents_beg.* = contents_beg;
                    p_contents_end.* = contents_end;
                    p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
                    p_end_line_index.* = line_index;
                    return 1;
                } else {
                    return 0;
                }
            } else {
                var codepoint: c_uint = undefined;
                _ = &codepoint;
                var char_size: MD_SIZE = undefined;
                _ = &char_size;
                codepoint = md_decode_unicode(ctx.*.text, off, ctx.*.size, &char_size);
                if (!(md_is_unicode_whitespace__(codepoint) != 0)) {
                    if (contents_end == @as(MD_OFFSET, @bitCast(@as(c_int, 0)))) {
                        contents_beg = off;
                        p_beg_line_index.* = line_index;
                    }
                    contents_end = off +% char_size;
                }
                off +%= @as(MD_OFFSET, @bitCast(char_size));
            }
            len += 1;
            if (len > @as(c_int, 999)) return 0;
        }
        line_index +%= 1;
        len += 1;
        if (line_index < n_lines) {
            off = lines[line_index].beg;
        } else break;
    }
    return 0;
}

pub fn md_is_link_destination_A(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_contents_beg: [*c]MD_OFFSET, arg_p_contents_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_contents_beg = arg_p_contents_beg;
    _ = &p_contents_beg;
    var p_contents_end = arg_p_contents_end;
    _ = &p_contents_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    if ((off >= max_end) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '<'))) return 0;
    off +%= 1;
    while (off < max_end) {
        if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\\')) and ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < max_end)) and (((((@as(c_uint, @bitCast(@as(c_int, 33))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 47))))) or ((@as(c_uint, @bitCast(@as(c_int, 58))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 64)))))) or ((@as(c_uint, @bitCast(@as(c_int, 91))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 96)))))) or ((@as(c_uint, @bitCast(@as(c_int, 123))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 126))))))) {
            off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
            continue;
        }
        if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n'))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '<'))) return 0;
        if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '>')) {
            p_contents_beg.* = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
            p_contents_end.* = off;
            p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
            return 1;
        }
        off +%= 1;
    }
    return 0;
}

pub fn md_is_link_destination_B(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_contents_beg: [*c]MD_OFFSET, arg_p_contents_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_contents_beg = arg_p_contents_beg;
    _ = &p_contents_beg;
    var p_contents_end = arg_p_contents_end;
    _ = &p_contents_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    var parenthesis_level: c_int = 0;
    _ = &parenthesis_level;
    while (off < max_end) {
        if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\\')) and ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < max_end)) and (((((@as(c_uint, @bitCast(@as(c_int, 33))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 47))))) or ((@as(c_uint, @bitCast(@as(c_int, 58))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 64)))))) or ((@as(c_uint, @bitCast(@as(c_int, 91))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 96)))))) or ((@as(c_uint, @bitCast(@as(c_int, 123))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 126))))))) {
            off +%= @as(MD_OFFSET, @bitCast(@as(c_int, 2)));
            continue;
        }
        if ((((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c')))) or ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 31)))) or (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_uint, @bitCast(@as(c_int, 127)))))) break;
        if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '(')) {
            parenthesis_level += 1;
            if (parenthesis_level > @as(c_int, 32)) return 0;
        } else if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ')')) {
            if (parenthesis_level == @as(c_int, 0)) break;
            parenthesis_level -= 1;
        }
        off +%= 1;
    }
    if ((parenthesis_level != @as(c_int, 0)) or (off == beg)) return 0;
    p_contents_beg.* = beg;
    p_contents_end.* = off;
    p_end.* = off;
    return 1;
}

pub fn md_is_link_destination(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_contents_beg: [*c]MD_OFFSET, arg_p_contents_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_contents_beg = arg_p_contents_beg;
    _ = &p_contents_beg;
    var p_contents_end = arg_p_contents_end;
    _ = &p_contents_end;
    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '<')) return md_is_link_destination_A(ctx, beg, max_end, p_end, p_contents_beg, p_contents_end) else return md_is_link_destination_B(ctx, beg, max_end, p_end, p_contents_beg, p_contents_end);
    return 0;
}

pub fn md_is_link_title(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_beg_line_index: [*c]MD_SIZE, arg_p_end_line_index: [*c]MD_SIZE, arg_p_contents_beg: [*c]MD_OFFSET, arg_p_contents_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_beg_line_index = arg_p_beg_line_index;
    _ = &p_beg_line_index;
    var p_end_line_index = arg_p_end_line_index;
    _ = &p_end_line_index;
    var p_contents_beg = arg_p_contents_beg;
    _ = &p_contents_beg;
    var p_contents_end = arg_p_contents_end;
    _ = &p_contents_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    var closer_char: MD_CHAR = undefined;
    _ = &closer_char;
    var line_index: MD_SIZE = 0;
    _ = &line_index;
    while ((off < lines[line_index].end) and (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c'))))) {
        off +%= 1;
    }
    if (off >= lines[line_index].end) {
        line_index +%= 1;
        if (line_index >= n_lines) return 0;
        off = lines[line_index].beg;
    }
    if (off == beg) return 0;
    p_beg_line_index.* = line_index;
    while (true) {
        switch (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off])))) {
            @as(c_int, 34) => {
                closer_char = '"';
                break;
            },
            @as(c_int, 39) => {
                closer_char = '\'';
                break;
            },
            @as(c_int, 40) => {
                closer_char = ')';
                break;
            },
            else => return 0,
        }
        break;
    }
    off +%= 1;
    p_contents_beg.* = off;
    while (line_index < n_lines) {
        var line_end: MD_OFFSET = lines[line_index].end;
        _ = &line_end;
        while (off < line_end) {
            if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\\')) and ((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < ctx.*.size)) and ((((((@as(c_uint, @bitCast(@as(c_int, 33))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 47))))) or ((@as(c_uint, @bitCast(@as(c_int, 58))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 64)))))) or ((@as(c_uint, @bitCast(@as(c_int, 91))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 96)))))) or ((@as(c_uint, @bitCast(@as(c_int, 123))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 126)))))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\n'))))) {
                off +%= 1;
            } else if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, @bitCast(@as(c_uint, closer_char)))) {
                p_contents_end.* = off;
                p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
                p_end_line_index.* = line_index;
                return 1;
            } else if ((@as(c_int, @bitCast(@as(c_uint, closer_char))) == @as(c_int, ')')) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '('))) {
                return 0;
            }
            off +%= 1;
        }
        line_index +%= 1;
    }
    return 0;
}

pub fn md_is_link_reference_definition(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    var label_contents_beg: MD_OFFSET = undefined;
    var label_contents_end: MD_OFFSET = undefined;
    var label_contents_line_index: MD_SIZE = undefined;
    var label_is_multiline: c_int = FALSE;
    var dest_contents_beg: MD_OFFSET = undefined;
    var dest_contents_end: MD_OFFSET = undefined;
    var title_contents_beg: MD_OFFSET = undefined;
    var title_contents_end: MD_OFFSET = undefined;
    var title_contents_line_index: MD_SIZE = undefined;
    var title_is_multiline: c_int = FALSE;
    var off: MD_OFFSET = undefined;
    var line_index: MD_SIZE = 0;
    var tmp_line_index: MD_SIZE = undefined;
    var def: ?*MD_REF_DEF = null;
    var ret: c_int = 0;

    if (md_is_link_label(ctx, lines, n_lines, lines[0].beg, &off, &label_contents_line_index, &line_index, &label_contents_beg, &label_contents_end) == 0)
        return FALSE;
    label_is_multiline = @intFromBool(label_contents_line_index != line_index);

    if (off >= lines[@intCast(line_index)].end or ctx.*.text[@intCast(off)] != ':')
        return FALSE;
    off += 1;

    while (off < lines[@intCast(line_index)].end and (ctx.*.text[@intCast(off)] == ' ' or ctx.*.text[@intCast(off)] == '\t'))
        off += 1;
    if (off >= lines[@intCast(line_index)].end) {
        line_index += 1;
        if (line_index >= n_lines)
            return FALSE;
        off = lines[@intCast(line_index)].beg;
    }

    if (md_is_link_destination(ctx, off, lines[@intCast(line_index)].end, &off, &dest_contents_beg, &dest_contents_end) == 0)
        return FALSE;

    if (md_is_link_title(ctx, lines + @as(usize, @intCast(line_index)), n_lines - line_index, off, &off, &title_contents_line_index, &tmp_line_index, &title_contents_beg, &title_contents_end) != 0 and off >= lines[@intCast(line_index + tmp_line_index)].end) {
        title_is_multiline = @intFromBool(tmp_line_index != title_contents_line_index);
        title_contents_line_index += line_index;
        line_index += tmp_line_index;
    } else {
        title_is_multiline = FALSE;
        title_contents_beg = off;
        title_contents_end = off;
        title_contents_line_index = 0;
    }

    if (off < lines[@intCast(line_index)].end)
        return FALSE;

    ret = abort: {
        if (ctx.*.n_ref_defs >= ctx.*.alloc_ref_defs) {
            ctx.*.alloc_ref_defs = if (ctx.*.alloc_ref_defs > 0)
                ctx.*.alloc_ref_defs + @divTrunc(ctx.*.alloc_ref_defs, @as(c_int, 2))
            else
                16;
            const new_defs: ?*MD_REF_DEF = @ptrCast(@alignCast(realloc(@ptrCast(ctx.*.ref_defs), @as(usize, @intCast(ctx.*.alloc_ref_defs)) * @sizeOf(MD_REF_DEF))));
            if (new_defs == null) {
                md_log(ctx, "realloc() failed.");
                break :abort -1;
            }
            ctx.*.ref_defs = new_defs;
        }
        const ref_defs_ptr: [*c]MD_REF_DEF = @ptrCast(ctx.*.ref_defs);
        def = &ref_defs_ptr[@intCast(ctx.*.n_ref_defs)];
        _ = memset(@ptrCast(def.?), 0, @sizeOf(MD_REF_DEF));

        if (label_is_multiline != 0) {
            const r = md_merge_lines_alloc(ctx, label_contents_beg, label_contents_end, lines + @as(usize, @intCast(label_contents_line_index)), n_lines - label_contents_line_index, ' ', &def.?.label, &def.?.label_size);
            if (r < 0) break :abort -1;
            def.?.label_needs_free = 1;
        } else {
            def.?.label = @constCast(ctx.*.text + @as(usize, @intCast(label_contents_beg)));
            def.?.label_size = label_contents_end - label_contents_beg;
        }

        if (title_is_multiline != 0) {
            const r = md_merge_lines_alloc(ctx, title_contents_beg, title_contents_end, lines + @as(usize, @intCast(title_contents_line_index)), n_lines - title_contents_line_index, '\n', &def.?.title, &def.?.title_size);
            if (r < 0) break :abort -1;
            def.?.title_needs_free = 1;
        } else {
            def.?.title = @constCast(ctx.*.text + @as(usize, @intCast(title_contents_beg)));
            def.?.title_size = title_contents_end - title_contents_beg;
        }

        def.?.dest_beg = dest_contents_beg;
        def.?.dest_end = dest_contents_end;

        ctx.*.n_ref_defs += 1;
        return @as(c_int, @intCast(line_index)) + 1;
    };

    if (def != null and def.?.label_needs_free != 0)
        free(@ptrCast(def.?.label));
    if (def != null and def.?.title_needs_free != 0)
        free(@ptrCast(def.?.title));
    return ret;
}

pub fn md_is_link_reference(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_end: MD_OFFSET, arg_attr: [*c]MD_LINK_ATTR) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    var beg: MD_OFFSET = arg_beg;
    var end: MD_OFFSET = arg_end;
    const attr = arg_attr;
    var is_multiline: c_int = undefined;
    var label: [*c]MD_CHAR = undefined;
    var label_size: MD_SIZE = undefined;
    var ret: c_int = FALSE;

    if (ctx.*.max_ref_def_output == 0)
        return FALSE;

    beg += @as(MD_OFFSET, if (ctx.*.text[@intCast(beg)] == '!') 2 else 1);
    end -= 1;

    const beg_line: [*c]const MD_LINE = md_lookup_line(beg, lines, n_lines, null);
    is_multiline = @intFromBool(end > beg_line.*.end);

    ret = abort: {
        if (is_multiline != 0) {
            const r = md_merge_lines_alloc(ctx, beg, end, beg_line, @as(c_uint, @intCast(n_lines -% (@as(usize, @intFromPtr(beg_line)) -% @as(usize, @intFromPtr(lines))) / @sizeOf(MD_LINE))), ' ', &label, &label_size);
            if (r < 0) break :abort -1;
        } else {
            label = @constCast(ctx.*.text + @as(usize, @intCast(beg)));
            label_size = end - beg;
        }

        const def = md_lookup_ref_def(ctx, label, label_size);
        if (def != null) {
            attr.*.dest_beg = def.?.dest_beg;
            attr.*.dest_end = def.?.dest_end;
            attr.*.title = def.?.title;
            attr.*.title_size = def.?.title_size;
            attr.*.title_needs_free = FALSE;
        }

        if (is_multiline != 0)
            free(@ptrCast(label));

        if (def != null) {
            const output_size_estimation: MD_SIZE = def.?.label_size + def.?.title_size + def.?.dest_end - def.?.dest_beg;
            if (output_size_estimation < ctx.*.max_ref_def_output) {
                ctx.*.max_ref_def_output -= output_size_estimation;
                break :abort TRUE;
            } else {
                md_log(ctx, "Too many link reference definition instantiations.");
                ctx.*.max_ref_def_output = 0;
            }
        }

        break :abort FALSE;
    };

    return ret;
}

pub fn md_is_inline_link_spec(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_attr: [*c]MD_LINK_ATTR) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    const attr = arg_attr;
    const p_end = arg_p_end;
    var line_index: MD_SIZE = 0;
    var tmp_line_index: MD_SIZE = undefined;
    var title_contents_beg: MD_OFFSET = undefined;
    var title_contents_end: MD_OFFSET = undefined;
    var title_contents_line_index: MD_SIZE = undefined;
    var title_is_multiline: c_int = undefined;
    var off: MD_OFFSET = arg_beg;
    var ret: c_int = FALSE;

    _ = md_lookup_line(off, lines, n_lines, &line_index);

    off += 1;

    while (off < lines[@intCast(line_index)].end and (ctx.*.text[@intCast(off)] == ' ' or ctx.*.text[@intCast(off)] == '\t'))
        off += 1;
    if (off >= lines[@intCast(line_index)].end and (off >= ctx.*.size or (ctx.*.text[@intCast(off)] == '\r' or ctx.*.text[@intCast(off)] == '\n'))) {
        line_index += 1;
        if (line_index >= n_lines)
            return FALSE;
        off = lines[@intCast(line_index)].beg;
    }

    if (off < ctx.*.size and ctx.*.text[@intCast(off)] == ')') {
        attr.*.dest_beg = off;
        attr.*.dest_end = off;
        attr.*.title = null;
        attr.*.title_size = 0;
        attr.*.title_needs_free = FALSE;
        off += 1;
        p_end.* = off;
        return TRUE;
    }

    if (md_is_link_destination(ctx, off, lines[@intCast(line_index)].end, &off, &attr.*.dest_beg, &attr.*.dest_end) == 0)
        return FALSE;

    if (md_is_link_title(ctx, lines + @as(usize, @intCast(line_index)), n_lines - line_index, off, &off, &title_contents_line_index, &tmp_line_index, &title_contents_beg, &title_contents_end) != 0) {
        title_is_multiline = @intFromBool(tmp_line_index != title_contents_line_index);
        title_contents_line_index += line_index;
        line_index += tmp_line_index;
    } else {
        title_is_multiline = FALSE;
        title_contents_beg = off;
        title_contents_end = off;
        title_contents_line_index = 0;
    }

    while (off < lines[@intCast(line_index)].end and (ctx.*.text[@intCast(off)] == ' ' or ctx.*.text[@intCast(off)] == '\t'))
        off += 1;
    if (off >= lines[@intCast(line_index)].end) {
        line_index += 1;
        if (line_index >= n_lines)
            return FALSE;
        off = lines[@intCast(line_index)].beg;
    }
    if (ctx.*.text[@intCast(off)] != ')')
        return ret;

    off += 1;

    ret = abort: {
        if (title_contents_beg >= title_contents_end) {
            attr.*.title = null;
            attr.*.title_size = 0;
            attr.*.title_needs_free = FALSE;
        } else if (title_is_multiline == 0) {
            attr.*.title = @constCast(ctx.*.text + @as(usize, @intCast(title_contents_beg)));
            attr.*.title_size = title_contents_end - title_contents_beg;
            attr.*.title_needs_free = FALSE;
        } else {
            const r = md_merge_lines_alloc(ctx, title_contents_beg, title_contents_end, lines + @as(usize, @intCast(title_contents_line_index)), n_lines - title_contents_line_index, '\n', &attr.*.title, &attr.*.title_size);
            if (r < 0) break :abort ret;
            attr.*.title_needs_free = TRUE;
        }

        p_end.* = off;
        break :abort TRUE;
    };

    return ret;
}

pub fn md_free_ref_defs(arg_ctx: [*c]MD_CTX) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var i: c_int = undefined;
    _ = &i;
    {
        i = 0;
        while (i < ctx.*.n_ref_defs) : (i += 1) {
            var def: [*c]MD_REF_DEF = @as([*c]MD_REF_DEF, @ptrCast(ctx.*.ref_defs)) + @as(usize, @intCast(i));
            _ = &def;
            if (def.*.label_needs_free != 0) {
                free(@as(?*anyopaque, @ptrCast(def.*.label)));
            }
            if (def.*.title_needs_free != 0) {
                free(@as(?*anyopaque, @ptrCast(def.*.title)));
            }
        }
    }
    free(@as(?*anyopaque, @ptrCast(ctx.*.ref_defs)));
}

pub fn md_emph_stack(arg_ctx: [*c]MD_CTX, arg_ch: MD_CHAR, arg_flags: c_uint) callconv(.C) [*c]MD_MARKSTACK {
    var ctx = arg_ctx;
    _ = &ctx;
    var ch = arg_ch;
    _ = &ch;
    var flags = arg_flags;
    _ = &flags;
    var stack: [*c]MD_MARKSTACK = undefined;
    _ = &stack;
    while (true) {
        switch (@as(c_int, @bitCast(@as(c_uint, ch)))) {
            @as(c_int, 42) => {
                stack = &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 0)))];
                break;
            },
            @as(c_int, 95) => {
                stack = &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 6)))];
                break;
            },
            else => {
                while (true) {
                    unreachable;
// removed unreachable: if (!false) break;
                }
            },
        }
        break;
    }
    if ((flags & @as(c_uint, @bitCast(@as(c_int, 32)))) != 0) {
        stack += @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 3)))));
    }
    while (true) {
        switch (flags & @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) {
            @as(c_uint, @bitCast(@as(c_int, 64))) => {
                stack += @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 0)))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 128))) => {
                stack += @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 1)))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 192))) => {
                stack += @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 2)))));
                break;
            },
            else => {
                while (true) {
                    unreachable;
// removed unreachable: if (!false) break;
                }
            },
        }
        break;
    }
    return stack;
}

pub fn md_opener_stack(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) [*c]MD_MARKSTACK {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    while (true) {
        switch (@as(c_int, @bitCast(@as(c_uint, mark.*.ch)))) {
            @as(c_int, 42), @as(c_int, 95) => return md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_uint, mark.*.flags)))),
            @as(c_int, 126) => return if ((mark.*.end -% mark.*.beg) == @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 12)))] else &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 13)))],
            @as(c_int, 33), @as(c_int, 91) => return &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))],
            else => {
                while (true) {
                    unreachable;
// removed unreachable: if (!false) break;
                }
            },
        }
        break;
    }
    return null;
}

pub fn md_add_mark(arg_ctx: [*c]MD_CTX) callconv(.C) [*c]MD_MARK {
    var ctx = arg_ctx;
    _ = &ctx;
    if (ctx.*.n_marks >= ctx.*.alloc_marks) {
        var new_marks: [*c]MD_MARK = undefined;
        _ = &new_marks;
        ctx.*.alloc_marks = if (ctx.*.alloc_marks > @as(c_int, 0)) ctx.*.alloc_marks + @divTrunc(ctx.*.alloc_marks, @as(c_int, 2)) else @as(c_int, 64);
        new_marks = @as([*c]MD_MARK, @ptrCast(@alignCast(realloc(@as(?*anyopaque, @ptrCast(ctx.*.marks)), @as(c_ulong, @bitCast(@as(c_long, ctx.*.alloc_marks))) *% @sizeOf(MD_MARK)))));
        if (new_marks == @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("realloc() failed.", ctx.*.userdata);
                }
                if (!false) break;
            }
            return null;
        }
        ctx.*.marks = new_marks;
    }
    return &(blk: {
        const tmp = blk_1: {
            const ref = &ctx.*.n_marks;
            const tmp_2 = ref.*;
            ref.* += 1;
            break :blk_1 tmp_2;
        };
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
}

pub fn md_mark_stack_push(arg_ctx: [*c]MD_CTX, arg_stack: [*c]MD_MARKSTACK, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var stack = arg_stack;
    _ = &stack;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    (blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*.next = stack.*.top;
    stack.*.top = mark_index;
}

pub fn md_mark_stack_pop(arg_ctx: [*c]MD_CTX, arg_stack: [*c]MD_MARKSTACK) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var stack = arg_stack;
    _ = &stack;
    var top: c_int = stack.*.top;
    _ = &top;
    if (top >= @as(c_int, 0)) {
        stack.*.top = (blk: {
            const tmp = top;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*.next;
    }
    return top;
}

pub fn md_mark_store_ptr(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int, arg_ptr: ?*anyopaque) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var ptr = arg_ptr;
    _ = &ptr;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) == @as(c_int, 'D'))) {
            unreachable;
        }
        if (!false) break;
    }
    while (true) {
        if (!(@sizeOf(?*anyopaque) <= (@as(c_ulong, @bitCast(@as(c_long, @as(c_int, 2)))) *% @sizeOf(MD_OFFSET)))) {
            unreachable;
        }
        if (!false) break;
    }
    _ = memcpy(@as(?*anyopaque, @ptrCast(mark)), @as(?*const anyopaque, @ptrCast(&ptr)), @sizeOf(?*anyopaque));
}

pub fn md_mark_get_ptr(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) ?*anyopaque {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var ptr: ?*anyopaque = undefined;
    _ = &ptr;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) == @as(c_int, 'D'))) {
            unreachable;
        }
        if (!false) break;
    }
    _ = memcpy(@as(?*anyopaque, @ptrCast(&ptr)), @as(?*const anyopaque, @ptrCast(mark)), @sizeOf(?*anyopaque));
    return ptr;
}

pub fn md_resolve_range(arg_ctx: [*c]MD_CTX, arg_opener_index: c_int, arg_closer_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var opener_index = arg_opener_index;
    _ = &opener_index;
    var closer_index = arg_closer_index;
    _ = &closer_index;
    var opener: [*c]MD_MARK = &(blk: {
        const tmp = opener_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &opener;
    var closer: [*c]MD_MARK = &(blk: {
        const tmp = closer_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &closer;
    opener.*.next = closer_index;
    closer.*.prev = opener_index;
    opener.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 4) | @as(c_int, 16)))));
    closer.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 8) | @as(c_int, 16)))));
}

pub fn md_rollback(arg_ctx: [*c]MD_CTX, arg_opener_index: c_int, arg_closer_index: c_int, arg_how: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var opener_index = arg_opener_index;
    _ = &opener_index;
    var closer_index = arg_closer_index;
    _ = &closer_index;
    var how = arg_how;
    _ = &how;
    var i: c_int = undefined;
    _ = &i;
    {
        i = 0;
        while (i < @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf([16]MD_MARKSTACK) / @sizeOf(MD_MARKSTACK)))))) : (i += 1) {
            var stack: [*c]MD_MARKSTACK = &ctx.*.opener_stacks[@as(c_uint, @intCast(i))];
            _ = &stack;
            while (stack.*.top >= opener_index) {
                _ = md_mark_stack_pop(ctx, stack);
            }
        }
    }
    if (how == @as(c_int, 1)) {
        {
            i = opener_index + @as(c_int, 1);
            while (i < closer_index) : (i += 1) {
                (blk: {
                    const tmp = i;
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.ch = 'D';
                (blk: {
                    const tmp = i;
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.flags = 0;
            }
        }
    }
}

pub fn md_build_mark_char_map(arg_ctx: [*c]MD_CTX) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    _ = memset(@as(?*anyopaque, @ptrCast(@as([*c]u8, @ptrCast(@alignCast(&ctx.*.mark_char_map))))), @as(c_int, 0), @sizeOf([256]u8));
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '\\')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '*')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '_')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '`')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '&')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, ';')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '<')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '>')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '[')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '!')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, ']')))] = 1;
    ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '\x00')))] = 1;
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 512)))) != 0) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '~')))] = 1;
    }
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 4096)))) != 0) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '$')))] = 1;
    }
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 8)))) != 0) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '@')))] = 1;
    }
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 4)))) != 0) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, ':')))] = 1;
    }
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 1024)))) != 0) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '.')))] = 1;
    }
    if (((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 256)))) != 0) or ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 8192)))) != 0)) {
        ctx.*.mark_char_map[@as(c_uint, @intCast(@as(c_int, '|')))] = 1;
    }
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 1)))) != 0) {
        var i: c_int = undefined;
        _ = &i;
        {
            i = 0;
            while (i < @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf([256]u8)))))) : (i += 1) {
                if (((i == @as(c_int, ' ')) or (i == @as(c_int, '\t'))) or ((i == @as(c_int, '\x0b')) or (i == @as(c_int, '\x0c')))) {
                    ctx.*.mark_char_map[@as(c_uint, @intCast(i))] = 1;
                }
            }
        }
    }
}

pub fn md_is_code_span(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_beg: MD_OFFSET, arg_opener: [*c]MD_MARK, arg_closer: [*c]MD_MARK, arg_last_potential_closers: [*c]MD_OFFSET, arg_p_reached_paragraph_end: [*c]c_int) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var beg = arg_beg;
    _ = &beg;
    var opener = arg_opener;
    _ = &opener;
    var closer = arg_closer;
    _ = &closer;
    var last_potential_closers = arg_last_potential_closers;
    _ = &last_potential_closers;
    var p_reached_paragraph_end = arg_p_reached_paragraph_end;
    _ = &p_reached_paragraph_end;
    var opener_beg: MD_OFFSET = beg;
    _ = &opener_beg;
    var opener_end: MD_OFFSET = undefined;
    _ = &opener_end;
    var closer_beg: MD_OFFSET = undefined;
    _ = &closer_beg;
    var closer_end: MD_OFFSET = undefined;
    _ = &closer_end;
    var mark_len: MD_SIZE = undefined;
    _ = &mark_len;
    var line_end: MD_OFFSET = undefined;
    _ = &line_end;
    var has_space_after_opener: c_int = 0;
    _ = &has_space_after_opener;
    var has_eol_after_opener: c_int = 0;
    _ = &has_eol_after_opener;
    var has_space_before_closer: c_int = 0;
    _ = &has_space_before_closer;
    var has_eol_before_closer: c_int = 0;
    _ = &has_eol_before_closer;
    var has_only_space: c_int = 1;
    _ = &has_only_space;
    var line_index: MD_SIZE = 0;
    _ = &line_index;
    line_end = lines[@as(c_uint, @intCast(@as(c_int, 0)))].end;
    opener_end = opener_beg;
    while ((opener_end < line_end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[opener_end]))) == @as(c_int, '`'))) {
        opener_end +%= 1;
    }
    has_space_after_opener = @intFromBool((opener_end < line_end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[opener_end]))) == @as(c_int, ' ')));
    has_eol_after_opener = @intFromBool(opener_end == line_end);
    opener.*.end = opener_end;
    mark_len = opener_end -% opener_beg;
    if (mark_len > @as(MD_SIZE, @bitCast(@as(c_int, 32)))) return 0;
    if ((last_potential_closers[mark_len -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))] >= lines[n_lines -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))].end) or ((p_reached_paragraph_end.* != 0) and (last_potential_closers[mark_len -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))] < opener_end))) return 0;
    closer_beg = opener_end;
    closer_end = opener_end;
    while (true) {
        while ((closer_beg < line_end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_beg]))) != @as(c_int, '`'))) {
            if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_beg]))) != @as(c_int, ' ')) {
                has_only_space = 0;
            }
            closer_beg +%= 1;
        }
        closer_end = closer_beg;
        while ((closer_end < line_end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_end]))) == @as(c_int, '`'))) {
            closer_end +%= 1;
        }
        if ((closer_end -% closer_beg) == mark_len) {
            has_space_before_closer = @intFromBool((closer_beg > lines[line_index].beg) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, ' ')));
            has_eol_before_closer = @intFromBool(closer_beg == lines[line_index].beg);
            break;
        }
        if ((closer_end -% closer_beg) > @as(MD_OFFSET, @bitCast(@as(c_int, 0)))) {
            has_only_space = 0;
            if ((closer_end -% closer_beg) < @as(MD_OFFSET, @bitCast(@as(c_int, 32)))) {
                if (closer_beg > last_potential_closers[(closer_end -% closer_beg) -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]) {
                    last_potential_closers[(closer_end -% closer_beg) -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))] = closer_beg;
                }
            }
        }
        if (closer_end >= line_end) {
            line_index +%= 1;
            if (line_index >= n_lines) {
                p_reached_paragraph_end.* = 1;
                return 0;
            }
            line_end = lines[line_index].end;
            closer_beg = lines[line_index].beg;
        } else {
            closer_beg = closer_end;
        }
    }
    if ((!(has_only_space != 0) and ((has_space_after_opener != 0) or (has_eol_after_opener != 0))) and ((has_space_before_closer != 0) or (has_eol_before_closer != 0))) {
        if (has_space_after_opener != 0) {
            opener_end +%= 1;
        } else {
            opener_end = lines[@as(c_uint, @intCast(@as(c_int, 1)))].beg;
        }
        if (has_space_before_closer != 0) {
            closer_beg -%= 1;
        } else {
            closer_beg = lines[line_index -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))].end;
            while ((closer_beg < ctx.*.size) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_beg]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer_beg]))) == @as(c_int, '\t')))) {
                closer_beg +%= 1;
            }
        }
    }
    opener.*.ch = '`';
    opener.*.beg = opener_beg;
    opener.*.end = opener_end;
    opener.*.flags = 1;
    closer.*.ch = '`';
    closer.*.beg = closer_beg;
    closer.*.end = closer_end;
    closer.*.flags = 2;
    return 1;
}

pub fn md_is_autolink_uri(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    _ = &off;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '<'))) {
            unreachable;
        }
        if (!false) break;
    }
    if ((off >= max_end) or !(@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 127))))) return 0;
    off +%= 1;
    while (true) {
        if (off >= max_end) return 0;
        if ((off -% beg) > @as(MD_OFFSET, @bitCast(@as(c_int, 32)))) return 0;
        if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ':')) and ((off -% beg) >= @as(MD_OFFSET, @bitCast(@as(c_int, 3))))) break;
        if (((!((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '+'))) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '-'))) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '.'))) return 0;
        off +%= 1;
    }
    while ((off < max_end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '>'))) {
        if (((((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c')))) or ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 31)))) or (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_uint, @bitCast(@as(c_int, 127)))))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '<'))) return 0;
        off +%= 1;
    }
    if (off >= max_end) return 0;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '>'))) {
            unreachable;
        }
        if (!false) break;
    }
    p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    return 1;
}

pub fn md_is_autolink_email(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    _ = &off;
    var label_len: c_int = undefined;
    _ = &label_len;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '<'))) {
            unreachable;
        }
        if (!false) break;
    }
    while ((off < max_end) and (((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '\x00')) and (strchr(".!#$%&'*+/=?^_`{|}~-", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[off])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))))) {
        off +%= 1;
    }
    if (off <= (beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1))))) return 0;
    if ((off >= max_end) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '@'))) return 0;
    off +%= 1;
    label_len = 0;
    while (off < max_end) {
        if ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) {
            label_len += 1;
        } else if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '-')) and (label_len > @as(c_int, 0))) {
            label_len += 1;
        } else if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '.')) and (label_len > @as(c_int, 0))) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '-'))) {
            label_len = 0;
        } else break;
        if (label_len > @as(c_int, 63)) return 0;
        off +%= 1;
    }
    if ((((label_len <= @as(c_int, 0)) or (off >= max_end)) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '>'))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '-'))) return 0;
    p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    return 1;
}

pub fn md_is_autolink(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_max_end: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_missing_mailto: [*c]c_int) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var max_end = arg_max_end;
    _ = &max_end;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_missing_mailto = arg_p_missing_mailto;
    _ = &p_missing_mailto;
    if (md_is_autolink_uri(ctx, beg, max_end, p_end) != 0) {
        p_missing_mailto.* = 0;
        return 1;
    }
    if (md_is_autolink_email(ctx, beg, max_end, p_end) != 0) {
        p_missing_mailto.* = 1;
        return 1;
    }
    return 0;
}

pub fn md_collect_marks(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_table_mode: c_int) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    const table_mode = arg_table_mode;
    var ret: c_int = 0;
    var mark: [*c]MD_MARK = undefined;
    var codespan_last_potential_closers: [32]MD_OFFSET = [_]MD_OFFSET{0} ** 32;
    var codespan_scanned_till_paragraph_end: c_int = FALSE;

    // Helper: ADD_MARK inline
    const ADD_MARK_ = struct {
        fn f(c: [*c]MD_CTX, m: *[*c]MD_MARK) c_int {
            m.* = md_add_mark(c);
            if (m.* == @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                return -1;
            }
            return 0;
        }
    }.f;

    ret = abort: {
        var line_index: MD_SIZE = 0;
        while (line_index < n_lines) : (line_index += 1) {
            const line: *const MD_LINE = @ptrCast(&lines[@intCast(line_index)]);
            var off: MD_OFFSET = line.beg;

            while (true) {
                // Optimization: skip non-mark chars (loop unrolling)
                while (off + 3 < line.end and
                    ctx.*.mark_char_map[@as(u8, @bitCast(@as(i8, @truncate(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 0)])))))))] == 0 and
                    ctx.*.mark_char_map[@as(u8, @bitCast(@as(i8, @truncate(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)])))))))] == 0 and
                    ctx.*.mark_char_map[@as(u8, @bitCast(@as(i8, @truncate(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 2)])))))))] == 0 and
                    ctx.*.mark_char_map[@as(u8, @bitCast(@as(i8, @truncate(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 3)])))))))] == 0)
                {
                    off += 4;
                }
                while (off < line.end and
                    ctx.*.mark_char_map[@as(u8, @bitCast(@as(i8, @truncate(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[@intCast(off)])))))))] == 0)
                {
                    off += 1;
                }

                if (off >= line.end)
                    break;

                const ch: MD_CHAR = ctx.*.text[@intCast(off)];

                // A backslash escape.
                if (ch == '\\' and off + 1 < ctx.*.size and (ISPUNCT_c(ctx.*.text[@intCast(off + 1)]) or (ctx.*.text[@intCast(off + 1)] == '\r' or ctx.*.text[@intCast(off + 1)] == '\n'))) {
                    if (!(ctx.*.text[@intCast(off + 1)] == '\r' or ctx.*.text[@intCast(off + 1)] == '\n') or line_index + 1 < n_lines) {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = off + 2;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = 0x10; // MD_MARK_RESOLVED
                    }
                    off += 2;
                    continue;
                }

                // A potential (string) emphasis start/end.
                if (ch == '*' or ch == '_') {
                    var tmp: MD_OFFSET = off + 1;
                    var left_level: c_int = undefined;
                    var right_level: c_int = undefined;

                    while (tmp < line.end and ctx.*.text[@intCast(tmp)] == ch)
                        tmp += 1;

                    if (off == line.beg or md_is_unicode_whitespace__(md_decode_utf8_before__(ctx, off)) != 0)
                        left_level = 0
                    else if (md_is_unicode_punct__(md_decode_utf8_before__(ctx, off)) != 0)
                        left_level = 1
                    else
                        left_level = 2;

                    if (tmp == line.end or md_is_unicode_whitespace__(md_decode_utf8__(ctx.*.text + @as(usize, @intCast(tmp)), ctx.*.size -% tmp, null)) != 0)
                        right_level = 0
                    else if (md_is_unicode_punct__(md_decode_utf8__(ctx.*.text + @as(usize, @intCast(tmp)), ctx.*.size -% tmp, null)) != 0)
                        right_level = 1
                    else
                        right_level = 2;

                    // Intra-word underscore doesn't have special meaning.
                    if (ch == '_' and left_level == 2 and right_level == 2) {
                        left_level = 0;
                        right_level = 0;
                    }

                    if (left_level != 0 or right_level != 0) {
                        var flags: u8 = 0;

                        if (left_level > 0 and left_level >= right_level)
                            flags |= 0x02; // MD_MARK_POTENTIAL_CLOSER
                        if (right_level > 0 and right_level >= left_level)
                            flags |= 0x01; // MD_MARK_POTENTIAL_OPENER
                        if (flags == (0x01 | 0x02))
                            flags |= 0x20; // MD_MARK_EMPH_OC

                        // Rule of three: remember original size modulo 3.
                        switch (@mod(tmp - off, 3)) {
                            0 => flags |= 0x40, // MD_MARK_EMPH_MOD3_0
                            1 => flags |= 0x80, // MD_MARK_EMPH_MOD3_1
                            2 => flags |= (0x40 | 0x80), // MD_MARK_EMPH_MOD3_2
                            else => {},
                        }

                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = tmp;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = flags;

                        // Dummy marks for splitting
                        off += 1;
                        while (off < tmp) {
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = off;
                            mark.*.end = off;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = 'D';
                            mark.*.flags = 0;
                            off += 1;
                        }
                        continue;
                    }

                    off = tmp;
                    continue;
                }

                // A potential code span start/end.
                if (ch == '`') {
                    var opener: MD_MARK = undefined;
                    var closer: MD_MARK = undefined;

                    const is_code_span = md_is_code_span(ctx, line, n_lines - line_index, off, &opener, &closer, &codespan_last_potential_closers, &codespan_scanned_till_paragraph_end);
                    if (is_code_span != 0) {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = opener.beg;
                        mark.*.end = opener.end;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = opener.ch;
                        mark.*.flags = opener.flags;
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = closer.beg;
                        mark.*.end = closer.end;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = closer.ch;
                        mark.*.flags = closer.flags;
                        md_resolve_range(ctx, ctx.*.n_marks - 2, ctx.*.n_marks - 1);
                        off = closer.end;

                        // Advance the current line accordingly.
                        if (off > line.end) {
                            const new_line = md_lookup_line(off, lines, n_lines, &line_index);
                            _ = new_line;
                        }
                        continue;
                    }

                    off = opener.end;
                    continue;
                }

                // A potential entity start.
                if (ch == '&') {
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off + 1;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = ch;
                    mark.*.flags = 0x01; // MD_MARK_POTENTIAL_OPENER
                    off += 1;
                    continue;
                }

                // A potential entity end.
                if (ch == ';') {
                    if (ctx.*.n_marks > 0 and ctx.*.marks[@intCast(ctx.*.n_marks - 1)].ch == '&') {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = off + 1;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = 0x02; // MD_MARK_POTENTIAL_CLOSER
                    }
                    off += 1;
                    continue;
                }

                // A potential autolink or raw HTML start/end.
                if (ch == '<') {
                    var is_autolink: c_int = undefined;
                    var autolink_end: MD_OFFSET = undefined;
                    var missing_mailto: c_int = undefined;

                    if ((ctx.*.parser.flags & 0x0040) == 0) { // !MD_FLAG_NOHTMLSPANS
                        var is_html: c_int = undefined;
                        var html_end: MD_OFFSET = undefined;

                        is_html = md_is_html_any(ctx, line, n_lines - line_index, off, lines[@intCast(n_lines - 1)].end, &html_end);
                        if (is_html != 0) {
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = off;
                            mark.*.end = off;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = '<';
                            mark.*.flags = 0x04 | 0x10; // OPENER | RESOLVED
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = html_end;
                            mark.*.end = html_end;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = '>';
                            mark.*.flags = 0x08 | 0x10; // CLOSER | RESOLVED
                            ctx.*.marks[@intCast(ctx.*.n_marks - 2)].next = ctx.*.n_marks - 1;
                            ctx.*.marks[@intCast(ctx.*.n_marks - 1)].prev = ctx.*.n_marks - 2;
                            off = html_end;

                            if (off > line.end) {
                                const new_line = md_lookup_line(off, lines, n_lines, &line_index);
                                _ = new_line;
                            }
                            continue;
                        }
                    }

                    is_autolink = md_is_autolink(ctx, off, lines[@intCast(n_lines - 1)].end, &autolink_end, &missing_mailto);
                    if (is_autolink != 0) {
                        var flags: u8 = 0x10 | 0x20; // RESOLVED | AUTOLINK
                        if (missing_mailto != 0)
                            flags |= 0x40; // AUTOLINK_MISSING_MAILTO

                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = off + 1;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = '<';
                        mark.*.flags = 0x04 | flags; // OPENER | flags
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = autolink_end - 1;
                        mark.*.end = autolink_end;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = '>';
                        mark.*.flags = 0x08 | flags; // CLOSER | flags
                        ctx.*.marks[@intCast(ctx.*.n_marks - 2)].next = ctx.*.n_marks - 1;
                        ctx.*.marks[@intCast(ctx.*.n_marks - 1)].prev = ctx.*.n_marks - 2;
                        off = autolink_end;
                        continue;
                    }

                    off += 1;
                    continue;
                }

                // A potential link or its part.
                if (ch == '[' or (ch == '!' and off + 1 < line.end and ctx.*.text[@intCast(off + 1)] == '[')) {
                    const tmp: MD_OFFSET = if (ch == '[') off + 1 else off + 2;
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = tmp;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = ch;
                    mark.*.flags = 0x01; // MD_MARK_POTENTIAL_OPENER
                    off = tmp;
                    // Two dummies for link data
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = 'D';
                    mark.*.flags = 0;
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = 'D';
                    mark.*.flags = 0;
                    continue;
                }
                if (ch == ']') {
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off + 1;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = ch;
                    mark.*.flags = 0x02; // MD_MARK_POTENTIAL_CLOSER
                    off += 1;
                    continue;
                }

                // A potential permissive e-mail autolink.
                if (ch == '@') {
                    if (line.beg + 1 <= off and
                        ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) >= 'a' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) <= 'z') or
                        (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) >= 'A' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) <= 'Z') or
                        (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) >= '0' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off - 1)]))) <= '9')) and
                        off + 3 < line.end and
                        ((@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) >= 'a' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) <= 'z') or
                        (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) >= 'A' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) <= 'Z') or
                        (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) >= '0' and @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[@intCast(off + 1)]))) <= '9')))
                    {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = off + 1;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = 0x01; // MD_MARK_POTENTIAL_OPENER
                        // Push a dummy as reserve for closer
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = line.beg;
                        mark.*.end = line.end;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = 'D';
                        mark.*.flags = 0;
                    }
                    off += 1;
                    continue;
                }

                // A potential permissive URL autolink.
                if (ch == ':') {
                    const SchemeEntry = struct {
                        scheme: [*c]const MD_CHAR,
                        scheme_size: MD_SIZE,
                        suffix: [*c]const MD_CHAR,
                        suffix_size: MD_SIZE,
                    };
                    const scheme_map = [_]SchemeEntry{
                        .{ .scheme = "http", .scheme_size = 4, .suffix = "//", .suffix_size = 2 },
                        .{ .scheme = "https", .scheme_size = 5, .suffix = "//", .suffix_size = 2 },
                        .{ .scheme = "ftp", .scheme_size = 3, .suffix = "//", .suffix_size = 2 },
                    };

                    var scheme_found = false;
                    for (scheme_map) |entry| {
                        if (line.beg + entry.scheme_size <= off and
                            md_ascii_eq(ctx.*.text + @as(usize, @intCast(off - entry.scheme_size)), entry.scheme, entry.scheme_size) != 0 and
                            off + 1 + entry.suffix_size < line.end and
                            md_ascii_eq(ctx.*.text + @as(usize, @intCast(off + 1)), entry.suffix, entry.suffix_size) != 0)
                        {
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = off - entry.scheme_size;
                            mark.*.end = off + 1 + entry.suffix_size;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = ch;
                            mark.*.flags = 0x01; // MD_MARK_POTENTIAL_OPENER
                            // Push a dummy as reserve for closer
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = line.beg;
                            mark.*.end = line.end;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = 'D';
                            mark.*.flags = 0;
                            off += 1 + entry.suffix_size;
                            scheme_found = true;
                            break;
                        }
                    }

                    if (!scheme_found)
                        off += 1;
                    continue;
                }

                // A potential permissive WWW autolink.
                if (ch == '.') {
                    if (line.beg + 3 <= off and
                        md_ascii_eq(ctx.*.text + @as(usize, @intCast(off - 3)), "www", 3) != 0 and
                        (off - 3 == line.beg or md_is_unicode_whitespace__(md_decode_utf8_before__(ctx, off - 3)) != 0 or md_is_unicode_punct__(md_decode_utf8_before__(ctx, off - 3)) != 0))
                    {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off - 3;
                        mark.*.end = off + 1;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = 0x01; // MD_MARK_POTENTIAL_OPENER
                        // Push a dummy as reserve for closer
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = line.beg;
                        mark.*.end = line.end;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = 'D';
                        mark.*.flags = 0;
                        off += 1;
                        continue;
                    }
                    off += 1;
                    continue;
                }

                // A potential table cell boundary or wiki link label delimiter.
                if ((table_mode != 0 or (ctx.*.parser.flags & 0x2000) != 0) and ch == '|') { // MD_FLAG_WIKILINKS
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off + 1;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = ch;
                    mark.*.flags = 0;
                    off += 1;
                    continue;
                }

                // A potential strikethrough/equation start/end.
                if (ch == '$' or ch == '~') {
                    var tmp: MD_OFFSET = off + 1;

                    while (tmp < line.end and ctx.*.text[@intCast(tmp)] == ch)
                        tmp += 1;

                    if (tmp - off <= 2) {
                        var flags: u8 = 0x01 | 0x02; // POTENTIAL_OPENER | POTENTIAL_CLOSER

                        if (off > line.beg and md_is_unicode_whitespace__(md_decode_utf8_before__(ctx, off)) == 0 and md_is_unicode_punct__(md_decode_utf8_before__(ctx, off)) == 0)
                            flags &= ~@as(u8, 0x01); // clear POTENTIAL_OPENER
                        if (tmp < line.end and md_is_unicode_whitespace__(md_decode_utf8__(ctx.*.text + @as(usize, @intCast(tmp)), ctx.*.size -% tmp, null)) == 0 and md_is_unicode_punct__(md_decode_utf8__(ctx.*.text + @as(usize, @intCast(tmp)), ctx.*.size -% tmp, null)) == 0)
                            flags &= ~@as(u8, 0x02); // clear POTENTIAL_CLOSER
                        if (flags != 0) {
                            if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                            mark.*.beg = off;
                            mark.*.end = tmp;
                            mark.*.prev = -1;
                            mark.*.next = -1;
                            mark.*.ch = ch;
                            mark.*.flags = flags;
                        }
                    }

                    off = tmp;
                    continue;
                }

                // Turn non-trivial whitespace into single space.
                if (ch == ' ' or ch == '\t' or ch == '\x0b' or ch == '\x0c') {
                    var tmp: MD_OFFSET = off + 1;

                    while (tmp < line.end and (ctx.*.text[@intCast(tmp)] == ' ' or ctx.*.text[@intCast(tmp)] == '\t' or ctx.*.text[@intCast(tmp)] == '\x0b' or ctx.*.text[@intCast(tmp)] == '\x0c'))
                        tmp += 1;

                    if (tmp - off > 1 or ch != ' ') {
                        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                        mark.*.beg = off;
                        mark.*.end = tmp;
                        mark.*.prev = -1;
                        mark.*.next = -1;
                        mark.*.ch = ch;
                        mark.*.flags = 0x10; // MD_MARK_RESOLVED
                    }

                    off = tmp;
                    continue;
                }

                // NULL character.
                if (ch == 0) {
                    if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
                    mark.*.beg = off;
                    mark.*.end = off + 1;
                    mark.*.prev = -1;
                    mark.*.next = -1;
                    mark.*.ch = ch;
                    mark.*.flags = 0x10; // MD_MARK_RESOLVED
                    off += 1;
                    continue;
                }

                off += 1;
            }
        }

        // Add a dummy mark at the end of the mark vector.
        if (ADD_MARK_(ctx, &mark) < 0) break :abort -1;
        mark.*.beg = ctx.*.size;
        mark.*.end = ctx.*.size;
        mark.*.prev = -1;
        mark.*.next = -1;
        mark.*.ch = 127;
        mark.*.flags = 0x10; // MD_MARK_RESOLVED

        break :abort 0;
    };

    return ret;
}

pub fn md_analyze_bracket(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 1)) != 0) {
        if (ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))].top >= @as(c_int, 0)) {
            (blk: {
                const tmp = ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))].top;
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 32)))));
        }
        md_mark_stack_push(ctx, &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))], mark_index);
        return;
    }
    if (ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))].top >= @as(c_int, 0)) {
        var opener_index: c_int = md_mark_stack_pop(ctx, &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 14)))]);
        _ = &opener_index;
        var opener: [*c]MD_MARK = &(blk: {
            const tmp = opener_index;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &opener;
        opener.*.next = mark_index;
        mark.*.prev = opener_index;
        if (ctx.*.unresolved_link_tail >= @as(c_int, 0)) {
            (blk: {
                const tmp = ctx.*.unresolved_link_tail;
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*.prev = opener_index;
        } else {
            ctx.*.unresolved_link_head = opener_index;
        }
        ctx.*.unresolved_link_tail = opener_index;
        opener.*.prev = -@as(c_int, 1);
    }
}

pub fn md_analyze_link_contents(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_mark_beg: c_int, arg_mark_end: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var mark_beg = arg_mark_beg;
    _ = &mark_beg;
    var mark_end = arg_mark_end;
    _ = &mark_end;
    var i: c_int = undefined;
    _ = &i;
    md_analyze_marks(ctx, lines, n_lines, mark_beg, mark_end, "&", @as(c_uint, @bitCast(@as(c_int, 0))));
    md_analyze_marks(ctx, lines, n_lines, mark_beg, mark_end, "*_~$", @as(c_uint, @bitCast(@as(c_int, 0))));
    if ((ctx.*.parser.flags & @as(c_uint, @bitCast((@as(c_int, 8) | @as(c_int, 4)) | @as(c_int, 1024)))) != @as(c_uint, @bitCast(@as(c_int, 0)))) {
        md_analyze_marks(ctx, lines, n_lines, mark_beg, mark_end, "@:.", @as(c_uint, @bitCast(@as(c_int, 1))));
    }
    {
        i = 0;
        while (i < @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf([16]MD_MARKSTACK) / @sizeOf(MD_MARKSTACK)))))) : (i += 1) {
            ctx.*.opener_stacks[@as(c_uint, @intCast(i))].top = -@as(c_int, 1);
        }
    }
}

pub fn md_resolve_links(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var opener_index: c_int = ctx.*.unresolved_link_head;
    _ = &opener_index;
    var last_link_beg: MD_OFFSET = 0;
    _ = &last_link_beg;
    var last_link_end: MD_OFFSET = 0;
    _ = &last_link_end;
    var last_img_beg: MD_OFFSET = 0;
    _ = &last_img_beg;
    var last_img_end: MD_OFFSET = 0;
    _ = &last_img_end;
    while (opener_index >= @as(c_int, 0)) {
        var opener: [*c]MD_MARK = &(blk: {
            const tmp = opener_index;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &opener;
        var closer_index: c_int = opener.*.next;
        _ = &closer_index;
        var closer: [*c]MD_MARK = &(blk: {
            const tmp = closer_index;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &closer;
        var next_index: c_int = opener.*.prev;
        _ = &next_index;
        var next_opener: [*c]MD_MARK = undefined;
        _ = &next_opener;
        var next_closer: [*c]MD_MARK = undefined;
        _ = &next_closer;
        var attr: MD_LINK_ATTR = undefined;
        _ = &attr;
        var is_link: c_int = 0;
        _ = &is_link;
        if (next_index >= @as(c_int, 0)) {
            next_opener = &(blk: {
                const tmp = next_index;
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*;
            next_closer = &(blk: {
                const tmp = next_opener.*.next;
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*;
        } else {
            next_opener = null;
            next_closer = null;
        }
        if ((((opener.*.beg < last_link_beg) and (closer.*.end < last_link_end)) or ((opener.*.beg < last_img_beg) and (closer.*.end < last_img_end))) or ((opener.*.beg < last_link_end) and (@as(c_int, @bitCast(@as(c_uint, opener.*.ch))) == @as(c_int, '[')))) {
            opener_index = next_index;
            continue;
        }
        if (((((((((((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 8192)))) != 0) and ((opener.*.end -% opener.*.beg) == @as(MD_OFFSET, @bitCast(@as(c_int, 1))))) and (next_opener != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) and (@as(c_int, @bitCast(@as(c_uint, next_opener.*.ch))) == @as(c_int, '['))) and (next_opener.*.beg == (opener.*.beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))))) and ((next_opener.*.end -% next_opener.*.beg) == @as(MD_OFFSET, @bitCast(@as(c_int, 1))))) and (next_closer != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) and (@as(c_int, @bitCast(@as(c_uint, next_closer.*.ch))) == @as(c_int, ']'))) and (next_closer.*.beg == (closer.*.beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))))) and ((next_closer.*.end -% next_closer.*.beg) == @as(MD_OFFSET, @bitCast(@as(c_int, 1))))) {
            var delim: [*c]MD_MARK = null;
            _ = &delim;
            var delim_index: c_int = undefined;
            _ = &delim_index;
            var dest_beg: MD_OFFSET = undefined;
            _ = &dest_beg;
            var dest_end: MD_OFFSET = undefined;
            _ = &dest_end;
            is_link = 1;
            delim_index = opener_index + @as(c_int, 1);
            while (delim_index < closer_index) {
                var m: [*c]MD_MARK = &(blk: {
                    const tmp = delim_index;
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*;
                _ = &m;
                if (@as(c_int, @bitCast(@as(c_uint, m.*.ch))) == @as(c_int, '|')) {
                    delim = m;
                    break;
                }
                if (@as(c_int, @bitCast(@as(c_uint, m.*.ch))) != @as(c_int, 'D')) {
                    if ((m.*.beg -% opener.*.end) > @as(MD_OFFSET, @bitCast(@as(c_int, 100)))) break;
                    if ((@as(c_int, @bitCast(@as(c_uint, m.*.ch))) != @as(c_int, 'D')) and ((@as(c_int, @bitCast(@as(c_uint, m.*.flags))) & @as(c_int, 4)) != 0)) {
                        delim_index = m.*.next;
                    }
                }
                delim_index += 1;
            }
            dest_beg = opener.*.end;
            dest_end = if (delim != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) delim.*.beg else closer.*.beg;
            if (((dest_end -% dest_beg) == @as(MD_OFFSET, @bitCast(@as(c_int, 0)))) or ((dest_end -% dest_beg) > @as(MD_OFFSET, @bitCast(@as(c_int, 100))))) {
                is_link = 0;
            }
            if (is_link != 0) {
                var off: MD_OFFSET = undefined;
                _ = &off;
                {
                    off = dest_beg;
                    while (off < dest_end) : (off +%= 1) {
                        if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n'))) {
                            is_link = 0;
                            break;
                        }
                    }
                }
            }
            if (is_link != 0) {
                if (delim != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    if (delim.*.end < closer.*.beg) {
                        md_rollback(ctx, opener_index, delim_index, @as(c_int, 1));
                        md_rollback(ctx, delim_index, closer_index, @as(c_int, 0));
                        delim.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 16)))));
                        opener.*.end = delim.*.beg;
                    } else {
                        md_rollback(ctx, opener_index, closer_index, @as(c_int, 1));
                        closer.*.beg = delim.*.beg;
                        delim = null;
                    }
                }
                opener.*.beg = next_opener.*.beg;
                opener.*.next = closer_index;
                opener.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 4) | @as(c_int, 16)))));
                closer.*.end = next_closer.*.end;
                closer.*.prev = opener_index;
                closer.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 8) | @as(c_int, 16)))));
                last_link_beg = opener.*.beg;
                last_link_end = closer.*.end;
                if (delim != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    md_analyze_link_contents(ctx, lines, n_lines, delim_index + @as(c_int, 1), closer_index);
                }
                opener_index = next_opener.*.prev;
                continue;
            }
        }
        if ((next_opener != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) and (next_opener.*.beg == closer.*.end)) {
            if (next_closer.*.beg > (closer.*.end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1))))) {
                if (!((@as(c_int, @bitCast(@as(c_uint, next_opener.*.flags))) & @as(c_int, 32)) != 0)) {
                    is_link = md_is_link_reference(ctx, lines, n_lines, next_opener.*.beg, next_closer.*.end, &attr);
                }
            } else {
                if (!((@as(c_int, @bitCast(@as(c_uint, opener.*.flags))) & @as(c_int, 32)) != 0)) {
                    is_link = md_is_link_reference(ctx, lines, n_lines, opener.*.beg, closer.*.end, &attr);
                }
            }
            if (is_link < @as(c_int, 0)) return -@as(c_int, 1);
            if (is_link != 0) {
                closer.*.end = next_closer.*.end;
                next_index = (blk: {
                    const tmp = next_index;
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.prev;
            }
        } else {
            if ((closer.*.end < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[closer.*.end]))) == @as(c_int, '('))) {
                var inline_link_end: MD_OFFSET = (@as(c_uint, @bitCast(@as(c_int, 2147483647))) *% @as(c_uint, 2)) +% @as(c_uint, 1);
                _ = &inline_link_end;
                is_link = md_is_inline_link_spec(ctx, lines, n_lines, closer.*.end, &inline_link_end, &attr);
                if (is_link < @as(c_int, 0)) return -@as(c_int, 1);
                if (is_link != 0) {
                    var i: c_int = closer_index + @as(c_int, 1);
                    _ = &i;
                    while (i < ctx.*.n_marks) {
                        var mark: [*c]MD_MARK = &(blk: {
                            const tmp = i;
                            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                        }).*;
                        _ = &mark;
                        if (mark.*.beg >= inline_link_end) break;
                        if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & (@as(c_int, 4) | @as(c_int, 16))) == (@as(c_int, 4) | @as(c_int, 16))) {
                            if ((blk: {
                                const tmp = mark.*.next;
                                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                            }).*.beg >= inline_link_end) {
                                if (attr.title_needs_free != 0) {
                                    free(@as(?*anyopaque, @ptrCast(attr.title)));
                                }
                                is_link = 0;
                                break;
                            }
                            i = mark.*.next + @as(c_int, 1);
                        } else {
                            i += 1;
                        }
                    }
                }
                if (is_link != 0) {
                    closer.*.end = inline_link_end;
                }
            }
            if (!(is_link != 0)) {
                if (!((@as(c_int, @bitCast(@as(c_uint, opener.*.flags))) & @as(c_int, 32)) != 0)) {
                    is_link = md_is_link_reference(ctx, lines, n_lines, opener.*.beg, closer.*.end, &attr);
                }
                if (is_link < @as(c_int, 0)) return -@as(c_int, 1);
            }
        }
        if (is_link != 0) {
            opener.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 4) | @as(c_int, 16)))));
            closer.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 8) | @as(c_int, 16)))));
            while (true) {
                if (!(@as(c_int, @bitCast(@as(c_uint, (blk: {
                    const tmp = opener_index + @as(c_int, 1);
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.ch))) == @as(c_int, 'D'))) {
                    unreachable;
                }
                if (!false) break;
            }
            (blk: {
                const tmp = opener_index + @as(c_int, 1);
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*.beg = attr.dest_beg;
            (blk: {
                const tmp = opener_index + @as(c_int, 1);
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*.end = attr.dest_end;
            while (true) {
                if (!(@as(c_int, @bitCast(@as(c_uint, (blk: {
                    const tmp = opener_index + @as(c_int, 2);
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.ch))) == @as(c_int, 'D'))) {
                    unreachable;
                }
                if (!false) break;
            }
            md_mark_store_ptr(ctx, opener_index + @as(c_int, 2), @as(?*anyopaque, @ptrCast(attr.title)));
            if (attr.title_needs_free != 0) {
                md_mark_stack_push(ctx, &ctx.*.ptr_stack, opener_index + @as(c_int, 2));
            }
            (blk: {
                const tmp = opener_index + @as(c_int, 2);
                if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
            }).*.prev = @as(c_int, @bitCast(attr.title_size));
            if (@as(c_int, @bitCast(@as(c_uint, opener.*.ch))) == @as(c_int, '[')) {
                last_link_beg = opener.*.beg;
                last_link_end = closer.*.end;
            } else {
                last_img_beg = opener.*.beg;
                last_img_end = closer.*.end;
            }
            md_analyze_link_contents(ctx, lines, n_lines, opener_index + @as(c_int, 1), closer_index);
            if ((ctx.*.parser.flags & @as(c_uint, @bitCast((@as(c_int, 8) | @as(c_int, 4)) | @as(c_int, 1024)))) != 0) {
                var first_nested: [*c]MD_MARK = undefined;
                _ = &first_nested;
                var last_nested: [*c]MD_MARK = undefined;
                _ = &last_nested;
                first_nested = opener + @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 1)))));
                while ((@as(c_int, @bitCast(@as(c_uint, first_nested.*.ch))) == @as(c_int, 'D')) and (first_nested < closer)) {
                    first_nested += 1;
                }
                last_nested = closer - @as(usize, @bitCast(@as(isize, @intCast(@as(c_int, 1)))));
                while ((@as(c_int, @bitCast(@as(c_uint, first_nested.*.ch))) == @as(c_int, 'D')) and (last_nested > opener)) {
                    last_nested -= 1;
                }
                if ((((((@as(c_int, @bitCast(@as(c_uint, first_nested.*.flags))) & @as(c_int, 16)) != 0) and (first_nested.*.beg == opener.*.end)) and ((@as(c_int, @bitCast(@as(c_uint, first_nested.*.ch))) != @as(c_int, '\x00')) and (strchr("@:.", @as(c_int, @bitCast(@as(c_uint, first_nested.*.ch)))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) and (@as(c_long, @bitCast(@as(c_long, first_nested.*.next))) == @divExact(@as(c_long, @bitCast(@intFromPtr(last_nested) -% @intFromPtr(ctx.*.marks))), @sizeOf(MD_MARK)))) and (last_nested.*.end == closer.*.beg)) {
                    first_nested.*.ch = 'D';
                    first_nested.*.flags &= @as(u8, @bitCast(@as(i8, @truncate(~@as(c_int, 16)))));
                    last_nested.*.ch = 'D';
                    last_nested.*.flags &= @as(u8, @bitCast(@as(i8, @truncate(~@as(c_int, 16)))));
                }
            }
        }
        opener_index = next_index;
    }
    return 0;
}

pub fn md_analyze_entity(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var opener: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &opener;
    var closer: [*c]MD_MARK = undefined;
    _ = &closer;
    var off: MD_OFFSET = undefined;
    _ = &off;
    if ((mark_index + @as(c_int, 1)) >= ctx.*.n_marks) return;
    closer = &(blk: {
        const tmp = mark_index + @as(c_int, 1);
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    if (@as(c_int, @bitCast(@as(c_uint, closer.*.ch))) != @as(c_int, ';')) return;
    if (md_is_entity(ctx, opener.*.beg, closer.*.end, &off) != 0) {
        while (true) {
            if (!(off == closer.*.end)) {
                unreachable;
            }
            if (!false) break;
        }
        md_resolve_range(ctx, mark_index, mark_index + @as(c_int, 1));
        opener.*.end = closer.*.end;
    }
}

pub fn md_analyze_table_cell_boundary(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    mark.*.flags |= @as(u8, @bitCast(@as(i8, @truncate(@as(c_int, 16)))));
    mark.*.next = -@as(c_int, 1);
    if (ctx.*.table_cell_boundaries_head < @as(c_int, 0)) {
        ctx.*.table_cell_boundaries_head = mark_index;
    } else {
        (blk: {
            const tmp = ctx.*.table_cell_boundaries_tail;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*.next = mark_index;
    }
    ctx.*.table_cell_boundaries_tail = mark_index;
    ctx.*.n_table_cell_boundaries += 1;
}

pub fn md_split_emph_mark(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int, arg_n: MD_SIZE) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var n = arg_n;
    _ = &n;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    var new_mark_index: c_int = @as(c_int, @bitCast(@as(c_uint, @bitCast(mark_index)) +% ((mark.*.end -% mark.*.beg) -% n)));
    _ = &new_mark_index;
    var dummy: [*c]MD_MARK = &(blk: {
        const tmp = new_mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &dummy;
    while (true) {
        if (!((mark.*.end -% mark.*.beg) > n)) {
            unreachable;
        }
        if (!false) break;
    }
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, dummy.*.ch))) == @as(c_int, 'D'))) {
            unreachable;
        }
        if (!false) break;
    }
    _ = memcpy(@as(?*anyopaque, @ptrCast(dummy)), @as(?*const anyopaque, @ptrCast(mark)), @sizeOf(MD_MARK));
    mark.*.end -%= @as(MD_OFFSET, @bitCast(n));
    dummy.*.beg = mark.*.end;
    return new_mark_index;
}

pub fn md_analyze_emph(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 2)) != 0) {
        var opener: [*c]MD_MARK = null;
        _ = &opener;
        var opener_index: c_int = 0;
        _ = &opener_index;
        var opener_stacks: [6][*c]MD_MARKSTACK = undefined;
        _ = &opener_stacks;
        var i: c_int = undefined;
        _ = &i;
        var n_opener_stacks: c_int = undefined;
        _ = &n_opener_stacks;
        var flags: c_uint = @as(c_uint, @bitCast(@as(c_uint, mark.*.flags)));
        _ = &flags;
        n_opener_stacks = 0;
        opener_stacks[@as(c_uint, @intCast(blk: {
                const ref = &n_opener_stacks;
                const tmp = ref.*;
                ref.* += 1;
                break :blk tmp;
            }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 32))));
        if ((flags & @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) != @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) {
            opener_stacks[@as(c_uint, @intCast(blk: {
                    const ref = &n_opener_stacks;
                    const tmp = ref.*;
                    ref.* += 1;
                    break :blk tmp;
                }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_int, 128) | @as(c_int, 32))));
        }
        if ((flags & @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) != @as(c_uint, @bitCast(@as(c_int, 128)))) {
            opener_stacks[@as(c_uint, @intCast(blk: {
                    const ref = &n_opener_stacks;
                    const tmp = ref.*;
                    ref.* += 1;
                    break :blk tmp;
                }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast((@as(c_int, 64) | @as(c_int, 128)) | @as(c_int, 32))));
        }
        opener_stacks[@as(c_uint, @intCast(blk: {
                const ref = &n_opener_stacks;
                const tmp = ref.*;
                ref.* += 1;
                break :blk tmp;
            }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_int, 64))));
        if (!((flags & @as(c_uint, @bitCast(@as(c_int, 32)))) != 0) or ((flags & @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) != @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128))))) {
            opener_stacks[@as(c_uint, @intCast(blk: {
                    const ref = &n_opener_stacks;
                    const tmp = ref.*;
                    ref.* += 1;
                    break :blk tmp;
                }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_int, 128))));
        }
        if (!((flags & @as(c_uint, @bitCast(@as(c_int, 32)))) != 0) or ((flags & @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128)))) != @as(c_uint, @bitCast(@as(c_int, 128))))) {
            opener_stacks[@as(c_uint, @intCast(blk: {
                    const ref = &n_opener_stacks;
                    const tmp = ref.*;
                    ref.* += 1;
                    break :blk tmp;
                }))] = md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_int, 64) | @as(c_int, 128))));
        }
        {
            i = 0;
            while (i < n_opener_stacks) : (i += 1) {
                if (opener_stacks[@as(c_uint, @intCast(i))].*.top >= @as(c_int, 0)) {
                    var m_index: c_int = opener_stacks[@as(c_uint, @intCast(i))].*.top;
                    _ = &m_index;
                    var m: [*c]MD_MARK = &(blk: {
                        const tmp = m_index;
                        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                    }).*;
                    _ = &m;
                    if ((opener == @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) or (m.*.end > opener.*.end)) {
                        opener_index = m_index;
                        opener = m;
                    }
                }
            }
        }
        if (opener != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            var opener_size: MD_SIZE = opener.*.end -% opener.*.beg;
            _ = &opener_size;
            var closer_size: MD_SIZE = mark.*.end -% mark.*.beg;
            _ = &closer_size;
            var stack: [*c]MD_MARKSTACK = md_opener_stack(ctx, opener_index);
            _ = &stack;
            if (opener_size > closer_size) {
                opener_index = md_split_emph_mark(ctx, opener_index, closer_size);
                md_mark_stack_push(ctx, stack, opener_index);
            } else if (opener_size < closer_size) {
                _ = md_split_emph_mark(ctx, mark_index, closer_size -% opener_size);
            }
            _ = md_mark_stack_pop(ctx, stack);
            md_rollback(ctx, opener_index, mark_index, @as(c_int, 0));
            md_resolve_range(ctx, opener_index, mark_index);
            return;
        }
    }
    if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 1)) != 0) {
        md_mark_stack_push(ctx, md_emph_stack(ctx, mark.*.ch, @as(c_uint, @bitCast(@as(c_uint, mark.*.flags)))), mark_index);
    }
}

pub fn md_analyze_tilde(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    var stack: [*c]MD_MARKSTACK = md_opener_stack(ctx, mark_index);
    _ = &stack;
    if (((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 2)) != 0) and (stack.*.top >= @as(c_int, 0))) {
        var opener_index: c_int = stack.*.top;
        _ = &opener_index;
        _ = md_mark_stack_pop(ctx, stack);
        md_rollback(ctx, opener_index, mark_index, @as(c_int, 0));
        md_resolve_range(ctx, opener_index, mark_index);
        return;
    }
    if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 1)) != 0) {
        md_mark_stack_push(ctx, stack, mark_index);
    }
}

pub fn md_analyze_dollar(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    var mark: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &mark;
    if (((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 2)) != 0) and (ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))].top >= @as(c_int, 0))) {
        var opener: [*c]MD_MARK = &(blk: {
            const tmp = ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))].top;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &opener;
        var opener_index: c_int = ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))].top;
        _ = &opener_index;
        var closer: [*c]MD_MARK = mark;
        _ = &closer;
        var closer_index: c_int = mark_index;
        _ = &closer_index;
        if ((opener.*.end -% opener.*.beg) == (closer.*.end -% closer.*.beg)) {
            _ = md_mark_stack_pop(ctx, &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))]);
            md_rollback(ctx, opener_index, closer_index, @as(c_int, 1));
            md_resolve_range(ctx, opener_index, closer_index);
            ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))].top = -@as(c_int, 1);
            return;
        }
    }
    if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 1)) != 0) {
        md_mark_stack_push(ctx, &ctx.*.opener_stacks[@as(c_uint, @intCast(@as(c_int, 15)))], mark_index);
    }
}

pub fn md_scan_left_for_resolved_mark(arg_ctx: [*c]MD_CTX, arg_mark_from: [*c]MD_MARK, arg_off: MD_OFFSET, arg_p_cursor: [*c][*c]MD_MARK) callconv(.C) [*c]MD_MARK {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_from = arg_mark_from;
    _ = &mark_from;
    var off = arg_off;
    _ = &off;
    var p_cursor = arg_p_cursor;
    _ = &p_cursor;
    var mark: [*c]MD_MARK = undefined;
    _ = &mark;
    {
        mark = mark_from;
        while (mark >= ctx.*.marks) : (mark -= 1) {
            if ((@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) == @as(c_int, 'D')) or (mark.*.beg > off)) continue;
            if (((mark.*.beg <= off) and (off < mark.*.end)) and ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 16)) != 0)) {
                if (p_cursor != @as([*c][*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_cursor.* = mark;
                }
                return mark;
            }
            if (mark.*.end <= off) break;
        }
    }
    if (p_cursor != @as([*c][*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
        p_cursor.* = mark;
    }
    return null;
}

pub fn md_scan_right_for_resolved_mark(arg_ctx: [*c]MD_CTX, arg_mark_from: [*c]MD_MARK, arg_off: MD_OFFSET, arg_p_cursor: [*c][*c]MD_MARK) callconv(.C) [*c]MD_MARK {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_from = arg_mark_from;
    _ = &mark_from;
    var off = arg_off;
    _ = &off;
    var p_cursor = arg_p_cursor;
    _ = &p_cursor;
    var mark: [*c]MD_MARK = undefined;
    _ = &mark;
    {
        mark = mark_from;
        while (mark < (ctx.*.marks + @as(usize, @bitCast(@as(isize, @intCast(ctx.*.n_marks)))))) : (mark += 1) {
            if ((@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) == @as(c_int, 'D')) or (mark.*.end <= off)) continue;
            if (((mark.*.beg <= off) and (off < mark.*.end)) and ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 16)) != 0)) {
                if (p_cursor != @as([*c][*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    p_cursor.* = mark;
                }
                return mark;
            }
            if (mark.*.beg > off) break;
        }
    }
    if (p_cursor != @as([*c][*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
        p_cursor.* = mark;
    }
    return null;
}

pub fn md_analyze_permissive_autolink(arg_ctx: [*c]MD_CTX, arg_mark_index: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var mark_index = arg_mark_index;
    _ = &mark_index;
    const struct_unnamed_6 = extern struct {
        start_char: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
        delim_char: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
        allowed_nonalnum_chars: [*c]const MD_CHAR = @import("std").mem.zeroes([*c]const MD_CHAR),
        min_components: c_int = @import("std").mem.zeroes(c_int),
        optional_end_char: MD_CHAR = @import("std").mem.zeroes(MD_CHAR),
    };
    _ = &struct_unnamed_6;
    const URL_MAP = struct {
        const static: [4]struct_unnamed_6 = [4]struct_unnamed_6{
            struct_unnamed_6{
                .start_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '\x00'))))),
                .delim_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '.'))))),
                .allowed_nonalnum_chars = ".-_",
                .min_components = @as(c_int, 2),
                .optional_end_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '\x00'))))),
            },
            struct_unnamed_6{
                .start_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '/'))))),
                .delim_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '/'))))),
                .allowed_nonalnum_chars = "/.-_",
                .min_components = @as(c_int, 0),
                .optional_end_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '/'))))),
            },
            struct_unnamed_6{
                .start_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '?'))))),
                .delim_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '&'))))),
                .allowed_nonalnum_chars = "&.-+_=()",
                .min_components = @as(c_int, 1),
                .optional_end_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '\x00'))))),
            },
            struct_unnamed_6{
                .start_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '#'))))),
                .delim_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '\x00'))))),
                .allowed_nonalnum_chars = ".-+_",
                .min_components = @as(c_int, 1),
                .optional_end_char = @as(MD_CHAR, @bitCast(@as(i8, @truncate(@as(c_int, '\x00'))))),
            },
        };
    };
    _ = &URL_MAP;
    var opener: [*c]MD_MARK = &(blk: {
        const tmp = mark_index;
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &opener;
    var closer: [*c]MD_MARK = &(blk: {
        const tmp = mark_index + @as(c_int, 1);
        if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*;
    _ = &closer;
    var line_beg: MD_OFFSET = closer.*.beg;
    _ = &line_beg;
    var line_end: MD_OFFSET = closer.*.end;
    _ = &line_end;
    var beg: MD_OFFSET = opener.*.beg;
    _ = &beg;
    var end: MD_OFFSET = opener.*.end;
    _ = &end;
    var left_cursor: [*c]MD_MARK = opener;
    _ = &left_cursor;
    var left_boundary_ok: c_int = 0;
    _ = &left_boundary_ok;
    var right_cursor: [*c]MD_MARK = opener;
    _ = &right_cursor;
    var right_boundary_ok: c_int = 0;
    _ = &right_boundary_ok;
    var i: c_uint = undefined;
    _ = &i;
    while (true) {
        if (!(@as(c_int, @bitCast(@as(c_uint, closer.*.ch))) == @as(c_int, 'D'))) {
            unreachable;
        }
        if (!false) break;
    }
    if (@as(c_int, @bitCast(@as(c_uint, opener.*.ch))) == @as(c_int, '@')) {
        while (true) {
            if (!(@as(c_int, @bitCast(@as(c_uint, ctx.*.text[opener.*.beg]))) == @as(c_int, '@'))) {
                unreachable;
            }
            if (!false) break;
        }
        while (beg > line_beg) {
            if ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) {
                beg -%= 1;
            } else if (((((beg >= (line_beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 2))))) and ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 2)))]))) <= @as(c_uint, @bitCast(@as(c_int, '9'))))))) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '\x00')) and (strchr(".-_+", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) and (md_scan_left_for_resolved_mark(ctx, left_cursor, beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1))), &left_cursor) == @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) and ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[beg]))) <= @as(c_uint, @bitCast(@as(c_int, '9'))))))) {
                beg -%= 1;
            } else break;
        }
        if (beg == opener.*.beg) return;
    }
    if (((beg == line_beg) or (md_is_unicode_whitespace__(md_decode_utf8_before__(ctx, beg)) != 0)) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '\x00')) and (strchr("({[", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) {
        left_boundary_ok = 1;
    } else if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) != @as(c_int, '\x00')) and (strchr("*_~", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) {
        var left_mark: [*c]MD_MARK = undefined;
        _ = &left_mark;
        left_mark = md_scan_left_for_resolved_mark(ctx, left_cursor, beg -% @as(MD_OFFSET, @bitCast(@as(c_int, 1))), &left_cursor);
        if ((left_mark != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) and ((@as(c_int, @bitCast(@as(c_uint, left_mark.*.flags))) & @as(c_int, 4)) != 0)) {
            left_boundary_ok = 1;
        }
    }
    if (!(left_boundary_ok != 0)) return;
    {
        i = 0;
        while (@as(c_ulong, @bitCast(@as(c_ulong, i))) < (@sizeOf([4]struct_unnamed_6) / @sizeOf(struct_unnamed_6))) : (i +%= 1) {
            var n_components: c_int = 0;
            _ = &n_components;
            var n_open_brackets: c_int = 0;
            _ = &n_open_brackets;
            if (@as(c_int, @bitCast(@as(c_uint, URL_MAP.static[i].start_char))) != @as(c_int, '\x00')) {
                if ((end >= line_end) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) != @as(c_int, @bitCast(@as(c_uint, URL_MAP.static[i].start_char))))) continue;
                if ((URL_MAP.static[i].min_components > @as(c_int, 0)) and (((end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) >= line_end) or !((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))))) continue;
                end +%= 1;
            }
            while (end < line_end) {
                if ((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) {
                    if (n_components == @as(c_int, 0)) {
                        n_components += 1;
                    }
                    end +%= 1;
                } else if (((((end < line_end) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) != @as(c_int, '\x00')) and (strchr(URL_MAP.static[i].allowed_nonalnum_chars, @as(c_int, @bitCast(@as(c_uint, ctx.*.text[end])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) and (md_scan_right_for_resolved_mark(ctx, right_cursor, end, &right_cursor) == @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) and (((end > line_beg) and (((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, ')')))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, '(')))) and ((((end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) < line_end) and (((((@as(c_uint, @bitCast(@as(c_int, 'A'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'Z'))))) or ((@as(c_uint, @bitCast(@as(c_int, 'a'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, 'z')))))) or ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '(')))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, ')')))) {
                    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, @bitCast(@as(c_uint, URL_MAP.static[i].delim_char)))) {
                        n_components += 1;
                    }
                    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, '(')) {
                        n_open_brackets += 1;
                    } else if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, ')')) {
                        if (n_open_brackets <= @as(c_int, 0)) break;
                        n_open_brackets -= 1;
                    }
                    end +%= 1;
                } else {
                    break;
                }
            }
            if (((end < line_end) and (@as(c_int, @bitCast(@as(c_uint, URL_MAP.static[i].optional_end_char))) != @as(c_int, '\x00'))) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) == @as(c_int, @bitCast(@as(c_uint, URL_MAP.static[i].optional_end_char))))) {
                end +%= 1;
            }
            if ((n_components < URL_MAP.static[i].min_components) or (n_open_brackets != @as(c_int, 0))) return;
            if (@as(c_int, @bitCast(@as(c_uint, opener.*.ch))) == @as(c_int, '@')) break;
        }
    }
    if (((end == line_end) or (md_is_unicode_whitespace__(md_decode_utf8__(ctx.*.text + end, ctx.*.size -% end, null)) != 0)) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[end]))) != @as(c_int, '\x00')) and (strchr(")}].!?,;", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[end])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) {
        right_boundary_ok = 1;
    } else {
        var right_mark: [*c]MD_MARK = undefined;
        _ = &right_mark;
        right_mark = md_scan_right_for_resolved_mark(ctx, right_cursor, end, &right_cursor);
        if ((right_mark != @as([*c]MD_MARK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) and ((@as(c_int, @bitCast(@as(c_uint, right_mark.*.flags))) & @as(c_int, 8)) != 0)) {
            right_boundary_ok = 1;
        }
    }
    if (!(right_boundary_ok != 0)) return;
    opener.*.beg = beg;
    opener.*.end = beg;
    closer.*.beg = end;
    closer.*.end = end;
    closer.*.ch = opener.*.ch;
    md_resolve_range(ctx, mark_index, mark_index + @as(c_int, 1));
}

pub fn md_analyze_marks(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_mark_beg: c_int, arg_mark_end: c_int, arg_mark_chars: [*c]const MD_CHAR, arg_flags: c_uint) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    var mark_beg = arg_mark_beg;
    _ = &mark_beg;
    var mark_end = arg_mark_end;
    _ = &mark_end;
    var mark_chars = arg_mark_chars;
    _ = &mark_chars;
    var flags = arg_flags;
    _ = &flags;
    var i: c_int = mark_beg;
    _ = &i;
    var last_end: MD_OFFSET = lines[@as(c_uint, @intCast(@as(c_int, 0)))].beg;
    _ = &last_end;
    _ = &lines;
    _ = &n_lines;
    while (i < mark_end) {
        var mark: [*c]MD_MARK = &(blk: {
            const tmp = i;
            if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
        }).*;
        _ = &mark;
        if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 16)) != 0) {
            if (((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 4)) != 0) and !(((flags & @as(c_uint, @bitCast(@as(c_int, 1)))) != 0) and ((@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) != @as(c_int, '\x00')) and (strchr("*_~", @as(c_int, @bitCast(@as(c_uint, mark.*.ch)))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))))) {
                while (true) {
                    if (!(i < mark.*.next)) {
                        unreachable;
                    }
                    if (!false) break;
                }
                i = mark.*.next + @as(c_int, 1);
            } else {
                i += 1;
            }
            continue;
        }
        if (!((@as(c_int, @bitCast(@as(c_uint, mark.*.ch))) != @as(c_int, '\x00')) and (strchr(mark_chars, @as(c_int, @bitCast(@as(c_uint, mark.*.ch)))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))))) {
            i += 1;
            continue;
        }
        if (mark.*.beg < last_end) {
            i += 1;
            continue;
        }
        while (true) {
            switch (@as(c_int, @bitCast(@as(c_uint, mark.*.ch)))) {
                @as(c_int, 91), @as(c_int, 33), @as(c_int, 93) => {
                    md_analyze_bracket(ctx, i);
                    break;
                },
                @as(c_int, 38) => {
                    md_analyze_entity(ctx, i);
                    break;
                },
                @as(c_int, 124) => {
                    md_analyze_table_cell_boundary(ctx, i);
                    break;
                },
                @as(c_int, 95), @as(c_int, 42) => {
                    md_analyze_emph(ctx, i);
                    break;
                },
                @as(c_int, 126) => {
                    md_analyze_tilde(ctx, i);
                    break;
                },
                @as(c_int, 36) => {
                    md_analyze_dollar(ctx, i);
                    break;
                },
                @as(c_int, 46), @as(c_int, 58), @as(c_int, 64) => {
                    md_analyze_permissive_autolink(ctx, i);
                    break;
                },
                else => {},
            }
            break;
        }
        if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 16)) != 0) {
            if ((@as(c_int, @bitCast(@as(c_uint, mark.*.flags))) & @as(c_int, 4)) != 0) {
                last_end = (blk: {
                    const tmp = mark.*.next;
                    if (tmp >= 0) break :blk ctx.*.marks + @as(usize, @intCast(tmp)) else break :blk ctx.*.marks - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
                }).*.end;
            } else {
                last_end = mark.*.end;
            }
        }
        i += 1;
    }
}

pub fn md_analyze_inlines(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE, arg_table_mode: c_int) callconv(.C) c_int {
    const ctx = arg_ctx;
    const lines = arg_lines;
    const n_lines = arg_n_lines;
    const table_mode = arg_table_mode;
    var ret: c_int = 0;

    ret = abort: {
        ctx.*.n_marks = 0;
        ret = md_collect_marks(ctx, lines, n_lines, table_mode);
        if (ret < 0) break :abort ret;
        md_analyze_marks(ctx, lines, n_lines, 0, ctx.*.n_marks, "[]!", 0);
        ret = md_resolve_links(ctx, lines, n_lines);
        if (ret < 0) break :abort ret;
        ctx.*.opener_stacks[14].top = -1;
        ctx.*.unresolved_link_head = -1;
        ctx.*.unresolved_link_tail = -1;
        if (table_mode != 0) {
            ctx.*.n_table_cell_boundaries = 0;
            md_analyze_marks(ctx, lines, n_lines, 0, ctx.*.n_marks, "|", 0);
            break :abort ret;
        }
        md_analyze_link_contents(ctx, lines, n_lines, 0, ctx.*.n_marks);
        break :abort ret;
    };
    return ret;
}

pub fn md_enter_leave_span_a(arg_ctx: [*c]MD_CTX, arg_enter: c_int, arg_type: MD_SPANTYPE, arg_dest: [*c]const MD_CHAR, arg_dest_size: MD_SIZE, arg_is_autolink: c_int, arg_title: [*c]const MD_CHAR, arg_title_size: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    var href_build: MD_ATTRIBUTE_BUILD = @import("std").mem.zeroes(MD_ATTRIBUTE_BUILD);
    var title_build: MD_ATTRIBUTE_BUILD = @import("std").mem.zeroes(MD_ATTRIBUTE_BUILD);
    var det: MD_SPAN_A_DETAIL = @import("std").mem.zeroes(MD_SPAN_A_DETAIL);
    var ret: c_int = 0;
    _ = memset(@ptrCast(&det), 0, @sizeOf(MD_SPAN_A_DETAIL));
    ret = abort: {
        ret = md_build_attribute(ctx, arg_dest, arg_dest_size, if (arg_is_autolink != 0) @as(c_uint, MD_BUILD_ATTR_NO_ESCAPES) else 0, &det.href, &href_build);
        if (ret < 0) break :abort ret;
        ret = md_build_attribute(ctx, arg_title, arg_title_size, 0, &det.title, &title_build);
        if (ret < 0) break :abort ret;
        det.is_autolink = arg_is_autolink;
        if (arg_enter != 0) {
            ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(arg_type)), @ptrCast(&det), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from enter_span() callback."); break :abort ret; }
        } else {
            ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(arg_type)), @ptrCast(&det), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from leave_span() callback."); break :abort ret; }
        }
        break :abort ret;
    };
    md_free_attribute(ctx, &href_build);
    md_free_attribute(ctx, &title_build);
    return ret;
}

pub fn md_enter_leave_span_wikilink(arg_ctx: [*c]MD_CTX, arg_enter: c_int, arg_target: [*c]const MD_CHAR, arg_target_size: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    var target_build: MD_ATTRIBUTE_BUILD = @import("std").mem.zeroes(MD_ATTRIBUTE_BUILD);
    var det: MD_SPAN_WIKILINK_DETAIL = @import("std").mem.zeroes(MD_SPAN_WIKILINK_DETAIL);
    var ret: c_int = 0;
    _ = memset(@ptrCast(&det), 0, @sizeOf(MD_SPAN_WIKILINK_DETAIL));
    ret = abort: {
        ret = md_build_attribute(ctx, arg_target, arg_target_size, 0, &det.target, &target_build);
        if (ret < 0) break :abort ret;
        if (arg_enter != 0) {
            ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_WIKILINK)), @ptrCast(&det), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from enter_span() callback."); break :abort ret; }
        } else {
            ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_WIKILINK)), @ptrCast(&det), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from leave_span() callback."); break :abort ret; }
        }
        break :abort ret;
    };
    md_free_attribute(ctx, &target_build);
    return ret;
}

pub fn md_process_inlines(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    var text_type: MD_TEXTTYPE = @as(c_uint, @bitCast(MD_TEXT_NORMAL));
    var line: [*c]const MD_LINE = arg_lines;
    var prev_mark: ?[*c]MD_MARK = null;
    var mark: [*c]MD_MARK = undefined;
    var off: MD_OFFSET = arg_lines[0].beg;
    const end: MD_OFFSET = arg_lines[@as(usize, @intCast(arg_n_lines -% 1))].end;
    var tmp: MD_OFFSET = undefined;
    var enforce_hardbreak: c_int = 0;
    var ret: c_int = 0;

    // Find first resolved mark
    mark = ctx.*.marks;
    while ((mark.*.flags & 0x10) == 0) // MD_MARK_RESOLVED
        mark += 1;

    ret = abort: {
        while (true) {
            // Process text up to next mark or end-of-line
            tmp = if (line.*.end < mark.*.beg) line.*.end else mark.*.beg;
            if (tmp > off) {
                ret = ctx.*.parser.text.?(text_type, ctx.*.text + off, tmp - off, ctx.*.userdata);
                if (ret != 0) break :abort ret;
                off = tmp;
            }

            // If reached the mark, process it and move to next one
            if (off >= mark.*.beg) {
                const mark_ch = mark.*.ch;
                if (mark_ch == '\\') {
                    // Backslash escape
                    const next_off = mark.*.beg +% 1;
                    if (next_off < ctx.*.size and (ctx.*.text[next_off] == '\n' or ctx.*.text[next_off] == '\r')) {
                        enforce_hardbreak = 1;
                    } else {
                        ret = ctx.*.parser.text.?(text_type, ctx.*.text + (mark.*.beg +% 1), 1, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    }
                } else if (mark_ch == ' ') {
                    // Non-trivial space
                    ret = ctx.*.parser.text.?(text_type, " ", 1, ctx.*.userdata);
                    if (ret != 0) break :abort ret;
                } else if (mark_ch == '`') {
                    // Code span
                    if ((mark.*.flags & 0x04) != 0) { // OPENER
                        ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_CODE)), null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                        text_type = @as(c_uint, @bitCast(MD_TEXT_CODE));
                    } else {
                        ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_CODE)), null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                        text_type = @as(c_uint, @bitCast(MD_TEXT_NORMAL));
                    }
                } else if (mark_ch == '_') {
                    // Underline or emphasis
                    if ((ctx.*.parser.flags & 0x4000) != 0) { // MD_FLAG_UNDERLINE
                        if ((mark.*.flags & 0x04) != 0) { // OPENER
                            while (off < mark.*.end) {
                                ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_U)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 1;
                            }
                        } else {
                            while (off < mark.*.end) {
                                ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_U)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 1;
                            }
                        }
                    } else {
                        // Fall through to emphasis handling (same as '*')
                        if ((mark.*.flags & 0x04) != 0) { // OPENER
                            if ((mark.*.end -% off) % 2 != 0) {
                                ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_EM)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 1;
                            }
                            while (off +% 1 < mark.*.end) {
                                ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_STRONG)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 2;
                            }
                        } else {
                            while (off +% 1 < mark.*.end) {
                                ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_STRONG)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 2;
                            }
                            if ((mark.*.end -% off) % 2 != 0) {
                                ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_EM)), null, ctx.*.userdata);
                                if (ret != 0) break :abort ret;
                                off +%= 1;
                            }
                        }
                    }
                } else if (mark_ch == '*') {
                    // Emphasis, strong emphasis
                    if ((mark.*.flags & 0x04) != 0) { // OPENER
                        if ((mark.*.end -% off) % 2 != 0) {
                            ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_EM)), null, ctx.*.userdata);
                            if (ret != 0) break :abort ret;
                            off +%= 1;
                        }
                        while (off +% 1 < mark.*.end) {
                            ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_STRONG)), null, ctx.*.userdata);
                            if (ret != 0) break :abort ret;
                            off +%= 2;
                        }
                    } else {
                        while (off +% 1 < mark.*.end) {
                            ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_STRONG)), null, ctx.*.userdata);
                            if (ret != 0) break :abort ret;
                            off +%= 2;
                        }
                        if ((mark.*.end -% off) % 2 != 0) {
                            ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_EM)), null, ctx.*.userdata);
                            if (ret != 0) break :abort ret;
                            off +%= 1;
                        }
                    }
                } else if (mark_ch == '~') {
                    if ((mark.*.flags & 0x04) != 0) { // OPENER
                        ret = ctx.*.parser.enter_span.?(@as(c_uint, @bitCast(MD_SPAN_DEL)), null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    } else {
                        ret = ctx.*.parser.leave_span.?(@as(c_uint, @bitCast(MD_SPAN_DEL)), null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    }
                } else if (mark_ch == '$') {
                    const span_type: MD_SPANTYPE = if ((mark.*.end -% off) % 2 != 0) @as(c_uint, @bitCast(MD_SPAN_LATEXMATH)) else @as(c_uint, @bitCast(MD_SPAN_LATEXMATH_DISPLAY));
                    if ((mark.*.flags & 0x04) != 0) { // OPENER
                        ret = ctx.*.parser.enter_span.?(span_type, null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                        text_type = @as(c_uint, @bitCast(MD_TEXT_LATEXMATH));
                    } else {
                        ret = ctx.*.parser.leave_span.?(span_type, null, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                        text_type = @as(c_uint, @bitCast(MD_TEXT_NORMAL));
                    }
                } else if (mark_ch == '[' or mark_ch == '!' or mark_ch == ']') {
                    // Link, wiki link, image
                    const opener: [*c]const MD_MARK = if (mark_ch != ']') mark else &ctx.*.marks[@as(usize, @intCast(mark.*.prev))];
                    const closer: [*c]const MD_MARK = &ctx.*.marks[@as(usize, @intCast(opener.*.next))];

                    if ((opener.*.ch == '[' and closer.*.ch == ']') and
                        opener.*.end -% opener.*.beg >= 2 and
                        closer.*.end -% closer.*.beg >= 2)
                    {
                        // Wikilink
                        const has_label: c_int = if (opener.*.end -% opener.*.beg > 2) @as(c_int, 1) else @as(c_int, 0);
                        var target_sz: MD_SIZE = undefined;

                        if (has_label != 0)
                            target_sz = opener.*.end -% (opener.*.beg +% 2)
                        else
                            target_sz = closer.*.beg -% opener.*.end;

                        ret = md_enter_leave_span_wikilink(ctx, @intFromBool(mark_ch != ']'), if (has_label != 0) ctx.*.text + (opener.*.beg +% 2) else ctx.*.text + opener.*.end, target_sz);
                        if (ret < 0) break :abort ret;
                    } else {
                        // Regular link/image
                        const dest_mark: [*c]const MD_MARK = opener + @as(usize, 1);
                        const title_mark: [*c]const MD_MARK = opener + @as(usize, 2);
                        const title_mark_index: c_int = @intCast(@divExact(@intFromPtr(@as([*c]const u8, @ptrCast(title_mark))) - @intFromPtr(@as([*c]const u8, @ptrCast(ctx.*.marks))), @sizeOf(MD_MARK)));

                        ret = md_enter_leave_span_a(ctx, @intFromBool(mark_ch != ']'), if (opener.*.ch == '!') @as(c_uint, @bitCast(MD_SPAN_IMG)) else @as(c_uint, @bitCast(MD_SPAN_A)), ctx.*.text + dest_mark.*.beg, dest_mark.*.end -% dest_mark.*.beg, FALSE, @ptrCast(md_mark_get_ptr(ctx, title_mark_index)), @bitCast(title_mark.*.prev));
                        if (ret < 0) break :abort ret;

                        // link/image closer may span multiple lines
                        if (mark_ch == ']') {
                            while (mark.*.end > line.*.end)
                                line += 1;
                        }
                    }
                } else if (mark_ch == '<' or mark_ch == '>') {
                    // Autolink or raw HTML
                    if ((mark.*.flags & 0x20) == 0) { // not MD_MARK_AUTOLINK
                        // Raw HTML
                        if ((mark.*.flags & 0x04) != 0) // OPENER
                            text_type = @as(c_uint, @bitCast(MD_TEXT_HTML))
                        else
                            text_type = @as(c_uint, @bitCast(MD_TEXT_NORMAL));
                    } else {
                        // Auto-link: fall through to permissive autolink handling
                        const al_opener: [*c]MD_MARK = if ((mark.*.flags & 0x04) != 0) mark else &ctx.*.marks[@as(usize, @intCast(mark.*.prev))];
                        const al_closer: [*c]MD_MARK = &ctx.*.marks[@as(usize, @intCast(al_opener.*.next))];
                        var dest: [*c]const MD_CHAR = ctx.*.text + al_opener.*.end;
                        var dest_size: MD_SIZE = al_closer.*.beg -% al_opener.*.end;

                        if ((mark.*.flags & 0x04) != 0) // OPENER
                            al_closer.*.flags |= 0x80; // MD_MARK_VALIDPERMISSIVEAUTOLINK

                        if (al_opener.*.ch == '@' or al_opener.*.ch == '.' or
                            (al_opener.*.ch == '<' and (al_opener.*.flags & 0x40) != 0)) // MD_MARK_AUTOLINK_MISSING_MAILTO
                        {
                            dest_size +%= 7;
                            // MD_TEMP_BUFFER
                            if (dest_size > ctx.*.alloc_buffer) {
                                const new_buf = @as([*c]MD_CHAR, @ptrCast(realloc(@ptrCast(ctx.*.buffer), @as(usize, dest_size))));
                                if (new_buf == @as([*c]MD_CHAR, @ptrFromInt(0))) {
                                    md_log(ctx, "realloc() failed.");
                                    break :abort -1;
                                }
                                ctx.*.buffer = new_buf;
                                ctx.*.alloc_buffer = dest_size;
                            }
                            const prefix: [*c]const u8 = if (al_opener.*.ch == '.') "http://" else "mailto:";
                            _ = memcpy(@ptrCast(ctx.*.buffer), @as(?*const anyopaque, @ptrCast(prefix)), 7);
                            _ = memcpy(@ptrCast(ctx.*.buffer + @as(usize, 7)), @as(?*const anyopaque, @ptrCast(dest)), dest_size -% 7);
                            dest = ctx.*.buffer;
                        }

                        if ((al_closer.*.flags & 0x80) != 0) { // MD_MARK_VALIDPERMISSIVEAUTOLINK
                            ret = md_enter_leave_span_a(ctx, @as(c_int, @intCast(mark.*.flags & 0x04)), @as(c_uint, @bitCast(MD_SPAN_A)), dest, dest_size, TRUE, null, 0);
                            if (ret < 0) break :abort ret;
                        }
                    }
                } else if (mark_ch == '@' or mark_ch == ':' or mark_ch == '.') {
                    // Permissive autolinks
                    const al_opener: [*c]MD_MARK = if ((mark.*.flags & 0x04) != 0) mark else &ctx.*.marks[@as(usize, @intCast(mark.*.prev))];
                    const al_closer: [*c]MD_MARK = &ctx.*.marks[@as(usize, @intCast(al_opener.*.next))];
                    var dest: [*c]const MD_CHAR = ctx.*.text + al_opener.*.end;
                    var dest_size: MD_SIZE = al_closer.*.beg -% al_opener.*.end;

                    if ((mark.*.flags & 0x04) != 0) // OPENER
                        al_closer.*.flags |= 0x80; // MD_MARK_VALIDPERMISSIVEAUTOLINK

                    if (al_opener.*.ch == '@' or al_opener.*.ch == '.') {
                        dest_size +%= 7;
                        // MD_TEMP_BUFFER
                        if (dest_size > ctx.*.alloc_buffer) {
                            const new_buf = @as([*c]MD_CHAR, @ptrCast(realloc(@ptrCast(ctx.*.buffer), @as(usize, dest_size))));
                            if (new_buf == @as([*c]MD_CHAR, @ptrFromInt(0))) {
                                md_log(ctx, "realloc() failed.");
                                break :abort -1;
                            }
                            ctx.*.buffer = new_buf;
                            ctx.*.alloc_buffer = dest_size;
                        }
                        const prefix: [*c]const u8 = if (al_opener.*.ch == '.') "http://" else "mailto:";
                        _ = memcpy(@ptrCast(ctx.*.buffer), @as(?*const anyopaque, @ptrCast(prefix)), 7);
                        _ = memcpy(@ptrCast(ctx.*.buffer + @as(usize, 7)), @as(?*const anyopaque, @ptrCast(dest)), dest_size -% 7);
                        dest = ctx.*.buffer;
                    }

                    if ((al_closer.*.flags & 0x80) != 0) { // MD_MARK_VALIDPERMISSIVEAUTOLINK
                        ret = md_enter_leave_span_a(ctx, @as(c_int, @intCast(mark.*.flags & 0x04)), @as(c_uint, @bitCast(MD_SPAN_A)), dest, dest_size, TRUE, null, 0);
                        if (ret < 0) break :abort ret;
                    }
                } else if (mark_ch == '&') {
                    // Entity
                    ret = ctx.*.parser.text.?(@as(c_uint, @bitCast(MD_TEXT_ENTITY)), ctx.*.text + mark.*.beg, mark.*.end -% mark.*.beg, ctx.*.userdata);
                    if (ret != 0) break :abort ret;
                } else if (mark_ch == 0) {
                    // Null char
                    ret = ctx.*.parser.text.?(@as(c_uint, @bitCast(MD_TEXT_NULLCHAR)), "", 1, ctx.*.userdata);
                    if (ret != 0) break :abort ret;
                } else if (mark_ch == 127) {
                    break :abort ret;
                }

                off = mark.*.end;

                // Move to next resolved mark
                prev_mark = mark;
                mark += 1;
                while ((mark.*.flags & 0x10) == 0 or mark.*.beg < off) // MD_MARK_RESOLVED
                    mark += 1;
            }

            // If reached end of line, move to next one
            if (off >= line.*.end) {
                // If it is the last line, we are done
                if (off >= end)
                    break;

                if (text_type == @as(c_uint, @bitCast(MD_TEXT_CODE)) or text_type == @as(c_uint, @bitCast(MD_TEXT_LATEXMATH))) {
                    // Inside a code span, trailing line whitespace has to be outputted
                    tmp = off;
                    while (off < ctx.*.size and (ctx.*.text[off] == ' ' or ctx.*.text[off] == '\t'))
                        off +%= 1;
                    if (off > tmp) {
                        ret = ctx.*.parser.text.?(text_type, ctx.*.text + tmp, off -% tmp, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    }
                    // and new lines are transformed into single spaces
                    if (off == line.*.end) {
                        ret = ctx.*.parser.text.?(text_type, " ", 1, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    }
                } else if (text_type == @as(c_uint, @bitCast(MD_TEXT_HTML))) {
                    // Inside raw HTML, output new line verbatim
                    tmp = off;
                    while (tmp < end and (ctx.*.text[tmp] == ' ' or ctx.*.text[tmp] == '\t'))
                        tmp +%= 1;
                    if (tmp > off) {
                        ret = ctx.*.parser.text.?(@as(c_uint, @bitCast(MD_TEXT_HTML)), ctx.*.text + off, tmp -% off, ctx.*.userdata);
                        if (ret != 0) break :abort ret;
                    }
                    ret = ctx.*.parser.text.?(@as(c_uint, @bitCast(MD_TEXT_HTML)), "\n", 1, ctx.*.userdata);
                    if (ret != 0) break :abort ret;
                } else {
                    // Output soft or hard line break
                    var break_type: MD_TEXTTYPE = @as(c_uint, @bitCast(MD_TEXT_SOFTBR));

                    if (text_type == @as(c_uint, @bitCast(MD_TEXT_NORMAL))) {
                        if (enforce_hardbreak != 0 or (ctx.*.parser.flags & 0x8000) != 0) { // MD_FLAG_HARD_SOFT_BREAKS
                            break_type = @as(c_uint, @bitCast(MD_TEXT_BR));
                        } else {
                            while (off < ctx.*.size and (ctx.*.text[off] == ' ' or ctx.*.text[off] == '\t'))
                                off +%= 1;
                            if (off >= line.*.end +% 2 and ctx.*.text[off -% 2] == ' ' and ctx.*.text[off -% 1] == ' ' and (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r'))
                                break_type = @as(c_uint, @bitCast(MD_TEXT_BR));
                        }
                    }

                    ret = ctx.*.parser.text.?(break_type, "\n", 1, ctx.*.userdata);
                    if (ret != 0) break :abort ret;
                }

                // Move to the next line
                line += 1;
                off = line.*.beg;

                enforce_hardbreak = 0;
            }
        }
        break :abort 0;
    };
    return ret;
}

pub fn md_analyze_table_alignment(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_end: MD_OFFSET, arg_align: [*c]MD_ALIGN, arg_n_align: c_int) callconv(.C) void {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var end = arg_end;
    _ = &end;
    var @"align" = arg_align;
    _ = &@"align";
    var n_align = arg_n_align;
    _ = &n_align;
    const align_map = struct {
        const static: [4]MD_ALIGN = [4]MD_ALIGN{
            @as(c_uint, @bitCast(MD_ALIGN_DEFAULT)),
            @as(c_uint, @bitCast(MD_ALIGN_LEFT)),
            @as(c_uint, @bitCast(MD_ALIGN_RIGHT)),
            @as(c_uint, @bitCast(MD_ALIGN_CENTER)),
        };
    };
    _ = &align_map;
    var off: MD_OFFSET = beg;
    _ = &off;
    while (n_align > @as(c_int, 0)) {
        var index_1: c_int = 0;
        _ = &index_1;
        while (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '-')) {
            off +%= 1;
        }
        if ((off > beg) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off -% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, ':'))) {
            index_1 |= @as(c_int, 1);
        }
        while ((off < end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '-'))) {
            off +%= 1;
        }
        if ((off < end) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ':'))) {
            index_1 |= @as(c_int, 2);
        }
        @"align".* = align_map.static[@as(c_uint, @intCast(index_1))];
        @"align" += 1;
        n_align -= 1;
    }
}

pub fn md_process_normal_block_contents(arg_ctx: [*c]MD_CTX, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = undefined;

    ret = abort: {
        ret = md_analyze_inlines(ctx, arg_lines, arg_n_lines, FALSE);
        if (ret < 0) break :abort ret;
        ret = md_process_inlines(ctx, arg_lines, arg_n_lines);
        if (ret < 0) break :abort ret;
        break :abort ret;
    };

    // Free any temporary memory blocks stored within some dummy marks.
    var i: c_int = ctx.*.ptr_stack.top;
    while (i >= 0) {
        const next = ctx.*.marks[@as(usize, @intCast(i))].next;
        free(md_mark_get_ptr(ctx, i));
        i = next;
    }
    ctx.*.ptr_stack.top = -1;

    return ret;
}

pub fn md_process_table_cell(arg_ctx: [*c]MD_CTX, arg_cell_type: MD_BLOCKTYPE, arg_align: MD_ALIGN, arg_beg: MD_OFFSET, arg_end: MD_OFFSET) callconv(.C) c_int {
    const ctx = arg_ctx;
    var line: MD_LINE = @import("std").mem.zeroes(MD_LINE);
    var det: MD_BLOCK_TD_DETAIL = @import("std").mem.zeroes(MD_BLOCK_TD_DETAIL);
    var ret: c_int = 0;
    var beg = arg_beg;
    var end_v = arg_end;

    while (beg < end_v and (ctx.*.text[beg] == ' ' or ctx.*.text[beg] == '\t' or ctx.*.text[beg] == '\n' or ctx.*.text[beg] == '\r'))
        beg +%= 1;
    while (end_v > beg and (ctx.*.text[end_v -% 1] == ' ' or ctx.*.text[end_v -% 1] == '\t' or ctx.*.text[end_v -% 1] == '\n' or ctx.*.text[end_v -% 1] == '\r'))
        end_v -%= 1;

    det.@"align" = arg_align;
    line.beg = beg;
    line.end = end_v;

    ret = abort: {
        ret = ctx.*.parser.enter_block.?(arg_cell_type, @ptrCast(&det), ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }
        ret = md_process_normal_block_contents(ctx, &line, 1);
        if (ret < 0) break :abort ret;
        ret = ctx.*.parser.leave_block.?(arg_cell_type, @ptrCast(&det), ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }
        break :abort ret;
    };
    return ret;
}

pub fn md_process_table_row(arg_ctx: [*c]MD_CTX, arg_cell_type: MD_BLOCKTYPE, arg_beg: MD_OFFSET, arg_end: MD_OFFSET, arg_align: [*c]const MD_ALIGN, arg_col_count: c_int) callconv(.C) c_int {
    const ctx = arg_ctx;
    var line: MD_LINE = @import("std").mem.zeroes(MD_LINE);
    var pipe_offs: [*c]MD_OFFSET = null;
    var ret: c_int = 0;

    line.beg = arg_beg;
    line.end = arg_end;

    ret = abort: {
        ret = md_analyze_inlines(ctx, &line, 1, TRUE);
        if (ret < 0) break :abort ret;

        // Remember cell boundaries in local buffer
        const n: c_int = ctx.*.n_table_cell_boundaries + 2;
        pipe_offs = @ptrCast(@alignCast(malloc(@as(usize, @intCast(n)) * @sizeOf(MD_OFFSET))));
        if (pipe_offs == @as([*c]MD_OFFSET, @ptrFromInt(0))) {
            md_log(ctx, "malloc() failed.");
            break :abort -1;
        }
        var j: c_int = 0;
        pipe_offs[@as(usize, @intCast(j))] = arg_beg;
        j += 1;
        var mi: c_int = ctx.*.table_cell_boundaries_head;
        while (mi >= 0) {
            const m: [*c]MD_MARK = &ctx.*.marks[@as(usize, @intCast(mi))];
            pipe_offs[@as(usize, @intCast(j))] = m.*.end;
            j += 1;
            mi = m.*.next;
        }
        pipe_offs[@as(usize, @intCast(j))] = arg_end +% 1;
        j += 1;

        // Process cells
        ret = ctx.*.parser.enter_block.?(@as(c_uint, @bitCast(MD_BLOCK_TR)), null, ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }
        var k: c_int = 0;
        var i: c_int = 0;
        while (i < j - 1 and k < arg_col_count) : (i += 1) {
            const iu: usize = @intCast(i);
            if (pipe_offs[iu] < pipe_offs[iu + 1] -% 1) {
                ret = md_process_table_cell(ctx, arg_cell_type, arg_align[@as(usize, @intCast(k))], pipe_offs[iu], pipe_offs[iu + 1] -% 1);
                if (ret < 0) break :abort ret;
                k += 1;
            }
        }
        // Make sure we call enough table cells even if current table has too few
        while (k < arg_col_count) {
            ret = md_process_table_cell(ctx, arg_cell_type, arg_align[@as(usize, @intCast(k))], 0, 0);
            if (ret < 0) break :abort ret;
            k += 1;
        }
        ret = ctx.*.parser.leave_block.?(@as(c_uint, @bitCast(MD_BLOCK_TR)), null, ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }
        break :abort ret;
    };

    free(@ptrCast(pipe_offs));
    ctx.*.table_cell_boundaries_head = -1;
    ctx.*.table_cell_boundaries_tail = -1;

    return ret;
}

pub fn md_process_table_block_contents(arg_ctx: [*c]MD_CTX, arg_col_count: c_int, arg_lines: [*c]const MD_LINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    var @"align": [*c]MD_ALIGN = undefined;
    var ret: c_int = 0;

    @"align" = @ptrCast(@alignCast(malloc(@as(usize, @intCast(arg_col_count)) * @sizeOf(MD_ALIGN))));
    if (@"align" == @as([*c]MD_ALIGN, @ptrFromInt(0))) {
        md_log(ctx, "malloc() failed.");
        ret = -1;
        // Go directly to cleanup
        free(@ptrCast(@"align"));
        return ret;
    }

    md_analyze_table_alignment(ctx, arg_lines[1].beg, arg_lines[1].end, @"align", arg_col_count);

    ret = abort: {
        ret = ctx.*.parser.enter_block.?(@as(c_uint, @bitCast(MD_BLOCK_THEAD)), null, ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }
        ret = md_process_table_row(ctx, @as(c_uint, @bitCast(MD_BLOCK_TH)), arg_lines[0].beg, arg_lines[0].end, @"align", arg_col_count);
        if (ret < 0) break :abort ret;
        ret = ctx.*.parser.leave_block.?(@as(c_uint, @bitCast(MD_BLOCK_THEAD)), null, ctx.*.userdata);
        if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }

        if (arg_n_lines > 2) {
            ret = ctx.*.parser.enter_block.?(@as(c_uint, @bitCast(MD_BLOCK_TBODY)), null, ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }
            var line_index: MD_SIZE = 2;
            while (line_index < arg_n_lines) : (line_index += 1) {
                ret = md_process_table_row(ctx, @as(c_uint, @bitCast(MD_BLOCK_TD)), arg_lines[line_index].beg, arg_lines[line_index].end, @"align", arg_col_count);
                if (ret < 0) break :abort ret;
            }
            ret = ctx.*.parser.leave_block.?(@as(c_uint, @bitCast(MD_BLOCK_TBODY)), null, ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }
        }
        break :abort ret;
    };

    free(@ptrCast(@"align"));
    return ret;
}

pub fn md_process_verbatim_block_contents(arg_ctx: [*c]MD_CTX, arg_text_type: MD_TEXTTYPE, arg_lines: [*c]const MD_VERBATIMLINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    const ctx = arg_ctx;
    const indent_chunk_str = "                "; // 16 spaces
    const indent_chunk_size: c_int = 16;
    var ret: c_int = 0;

    ret = abort: {
        var line_index: MD_SIZE = 0;
        while (line_index < arg_n_lines) : (line_index += 1) {
            const vline: [*c]const MD_VERBATIMLINE = &arg_lines[line_index];
            var indent: c_int = @bitCast(vline.*.indent);

            // Output code indentation
            while (indent > indent_chunk_size) {
                ret = ctx.*.parser.text.?(arg_text_type, indent_chunk_str, @bitCast(indent_chunk_size), ctx.*.userdata);
                if (ret != 0) break :abort ret;
                indent -= indent_chunk_size;
            }
            if (indent > 0) {
                ret = ctx.*.parser.text.?(arg_text_type, indent_chunk_str, @bitCast(indent), ctx.*.userdata);
                if (ret != 0) break :abort ret;
            }

            // Output the code line itself (MD_TEXT_INSECURE - direct text callback)
            ret = ctx.*.parser.text.?(arg_text_type, ctx.*.text + vline.*.beg, vline.*.end -% vline.*.beg, ctx.*.userdata);
            if (ret != 0) break :abort ret;

            // Enforce end-of-line
            ret = ctx.*.parser.text.?(arg_text_type, "\n", 1, ctx.*.userdata);
            if (ret != 0) break :abort ret;
        }
        break :abort ret;
    };
    return ret;
}

pub fn md_process_code_block_contents(arg_ctx: [*c]MD_CTX, arg_is_fenced: c_int, arg_lines: [*c]const MD_VERBATIMLINE, arg_n_lines: MD_SIZE) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var is_fenced = arg_is_fenced;
    _ = &is_fenced;
    var lines = arg_lines;
    _ = &lines;
    var n_lines = arg_n_lines;
    _ = &n_lines;
    if (is_fenced != 0) {
        lines += 1;
        n_lines -%= 1;
    } else {
        while ((n_lines > @as(MD_SIZE, @bitCast(@as(c_int, 0)))) and (lines[@as(c_uint, @intCast(@as(c_int, 0)))].beg == lines[@as(c_uint, @intCast(@as(c_int, 0)))].end)) {
            lines += 1;
            n_lines -%= 1;
        }
        while ((n_lines > @as(MD_SIZE, @bitCast(@as(c_int, 0)))) and (lines[n_lines -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))].beg == lines[n_lines -% @as(MD_SIZE, @bitCast(@as(c_int, 1)))].end)) {
            n_lines -%= 1;
        }
    }
    if (n_lines == @as(MD_SIZE, @bitCast(@as(c_int, 0)))) return 0;
    return md_process_verbatim_block_contents(ctx, @as(c_uint, @bitCast(MD_TEXT_CODE)), lines, n_lines);
}

pub fn md_setup_fenced_code_detail(arg_ctx: [*c]MD_CTX, arg_block: ?*const MD_BLOCK, arg_det: [*c]MD_BLOCK_CODE_DETAIL, arg_info_build: [*c]MD_ATTRIBUTE_BUILD, arg_lang_build: [*c]MD_ATTRIBUTE_BUILD) callconv(.C) c_int {
    const ctx = arg_ctx;
    const fence_line: [*c]const MD_VERBATIMLINE = @ptrCast(@alignCast(@as([*c]const u8, @ptrCast(arg_block.?)) + @sizeOf(MD_BLOCK)));
    var beg: MD_OFFSET = fence_line.*.beg;
    var end_v: MD_OFFSET = fence_line.*.end;
    var lang_end: MD_OFFSET = undefined;
    const fence_ch: MD_CHAR = ctx.*.text[fence_line.*.beg];
    var ret: c_int = 0;

    // Skip the fence itself
    while (beg < ctx.*.size and ctx.*.text[beg] == fence_ch)
        beg +%= 1;
    // Trim initial spaces
    while (beg < ctx.*.size and ctx.*.text[beg] == ' ')
        beg +%= 1;
    // Trim trailing spaces
    while (end_v > beg and ctx.*.text[end_v -% 1] == ' ')
        end_v -%= 1;

    ret = abort: {
        // Build info string attribute
        ret = md_build_attribute(ctx, ctx.*.text + beg, end_v -% beg, 0, &arg_det.*.info, arg_info_build);
        if (ret < 0) break :abort ret;

        // Build lang string attribute
        lang_end = beg;
        while (lang_end < end_v and ctx.*.text[lang_end] != ' ' and ctx.*.text[lang_end] != '\t' and ctx.*.text[lang_end] != '\n' and ctx.*.text[lang_end] != '\r')
            lang_end +%= 1;
        ret = md_build_attribute(ctx, ctx.*.text + beg, lang_end -% beg, 0, &arg_det.*.lang, arg_lang_build);
        if (ret < 0) break :abort ret;

        arg_det.*.fence_char = fence_ch;
        break :abort ret;
    };
    return ret;
}

pub fn md_process_leaf_block(arg_ctx: [*c]MD_CTX, arg_block: ?*const MD_BLOCK) callconv(.C) c_int {
    const ctx = arg_ctx;
    const block = arg_block.?;
    const block_type: u8 = block.getType();
    const block_data: u16 = block.getData();
    _ = block.getFlags();

    // Use a byte buffer large enough to hold the largest detail union
    var det_bytes: [@sizeOf(MD_BLOCK_CODE_DETAIL) + 16]u8 = @import("std").mem.zeroes([@sizeOf(MD_BLOCK_CODE_DETAIL) + 16]u8);
    var info_build: MD_ATTRIBUTE_BUILD = @import("std").mem.zeroes(MD_ATTRIBUTE_BUILD);
    var lang_build: MD_ATTRIBUTE_BUILD = @import("std").mem.zeroes(MD_ATTRIBUTE_BUILD);
    var is_in_tight_list: c_int = undefined;
    var clean_fence_code_detail: c_int = FALSE;
    var ret: c_int = 0;

    if (ctx.*.n_containers == 0) {
        is_in_tight_list = FALSE;
    } else {
        const containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));
        is_in_tight_list = if (containers[@as(usize, @intCast(ctx.*.n_containers - 1))].is_loose == 0) TRUE else FALSE;
    }

    if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_H))))) {
        const det_h: *MD_BLOCK_H_DETAIL = @ptrCast(@alignCast(&det_bytes));
        det_h.*.level = @as(c_uint, block_data);
    } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_CODE))))) {
        if (block_data != 0) {
            _ = memset(&det_bytes, 0, @sizeOf(MD_BLOCK_CODE_DETAIL));
            clean_fence_code_detail = TRUE;
            const det_code: [*c]MD_BLOCK_CODE_DETAIL = @ptrCast(@alignCast(&det_bytes));
            ret = md_setup_fenced_code_detail(ctx, block, det_code, &info_build, &lang_build);
            if (ret < 0) {
                if (clean_fence_code_detail != 0) {
                    md_free_attribute(ctx, &info_build);
                    md_free_attribute(ctx, &lang_build);
                }
                return ret;
            }
        }
    } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_TABLE))))) {
        const det_table: *MD_BLOCK_TABLE_DETAIL = @ptrCast(@alignCast(&det_bytes));
        det_table.*.col_count = @as(c_uint, block_data);
        det_table.*.head_row_count = 1;
        det_table.*.body_row_count = block.n_lines -% 2;
    }

    ret = abort: {
        if (is_in_tight_list == 0 or block_type != @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_P))))) {
            ret = ctx.*.parser.enter_block.?(@as(c_uint, block_type), @ptrCast(&det_bytes), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }
        }

        // Process block contents
        if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_HR))))) {
            // noop
        } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_CODE))))) {
            ret = md_process_code_block_contents(ctx, @intFromBool(block_data != 0), @ptrCast(@alignCast(@as([*c]const u8, @ptrCast(block)) + @sizeOf(MD_BLOCK))), block.n_lines);
            if (ret < 0) break :abort ret;
        } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_HTML))))) {
            ret = md_process_verbatim_block_contents(ctx, @as(c_uint, @bitCast(MD_TEXT_HTML)), @ptrCast(@alignCast(@as([*c]const u8, @ptrCast(block)) + @sizeOf(MD_BLOCK))), block.n_lines);
            if (ret < 0) break :abort ret;
        } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_TABLE))))) {
            ret = md_process_table_block_contents(ctx, @as(c_int, @intCast(block_data)), @ptrCast(@alignCast(@as([*c]const u8, @ptrCast(block)) + @sizeOf(MD_BLOCK))), block.n_lines);
            if (ret < 0) break :abort ret;
        } else {
            ret = md_process_normal_block_contents(ctx, @ptrCast(@alignCast(@as([*c]const u8, @ptrCast(block)) + @sizeOf(MD_BLOCK))), block.n_lines);
            if (ret < 0) break :abort ret;
        }

        if (is_in_tight_list == 0 or block_type != @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_P))))) {
            ret = ctx.*.parser.leave_block.?(@as(c_uint, block_type), @ptrCast(&det_bytes), ctx.*.userdata);
            if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }
        }
        break :abort ret;
    };

    if (clean_fence_code_detail != 0) {
        md_free_attribute(ctx, &info_build);
        md_free_attribute(ctx, &lang_build);
    }
    return ret;
}

pub fn md_process_all_blocks(arg_ctx: [*c]MD_CTX) callconv(.C) c_int {
    const ctx = arg_ctx;
    var byte_off: c_int = 0;
    var ret: c_int = 0;

    // Reuse containers for tracking loose/tight lists
    ctx.*.n_containers = 0;

    ret = abort: {
        while (byte_off < ctx.*.n_block_bytes) {
            const block: *MD_BLOCK = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes))) + @as(usize, @intCast(byte_off))));
            const block_type: u8 = block.getType();
            const block_flags: u8 = block.getFlags();
            const block_data: u16 = block.getData();

            // Build detail structs for container blocks
            var det_bytes: [32]u8 = @import("std").mem.zeroes([32]u8);

            if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_UL))))) {
                const det_ul: *MD_BLOCK_UL_DETAIL = @ptrCast(@alignCast(&det_bytes));
                det_ul.*.is_tight = if ((block_flags & MD_BLOCK_LOOSE_LIST) != 0) FALSE else TRUE;
                det_ul.*.mark = @truncate(@as(c_uint, block_data));
            } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_OL))))) {
                const det_ol: *MD_BLOCK_OL_DETAIL = @ptrCast(@alignCast(&det_bytes));
                det_ol.*.start = block.n_lines;
                det_ol.*.is_tight = if ((block_flags & MD_BLOCK_LOOSE_LIST) != 0) FALSE else TRUE;
                det_ol.*.mark_delimiter = @truncate(@as(c_uint, block_data));
            } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_LI))))) {
                const det_li: *MD_BLOCK_LI_DETAIL = @ptrCast(@alignCast(&det_bytes));
                det_li.*.is_task = @intFromBool(block_data != 0);
                det_li.*.task_mark = @truncate(@as(c_uint, block_data));
                det_li.*.task_mark_offset = block.n_lines;
            }

            if ((block_flags & MD_BLOCK_CONTAINER) != 0) {
                if ((block_flags & MD_BLOCK_CONTAINER_CLOSER) != 0) {
                    ret = ctx.*.parser.leave_block.?(@as(c_uint, block_type), @ptrCast(&det_bytes), ctx.*.userdata);
                    if (ret != 0) { md_log(ctx, "Aborted from leave_block() callback."); break :abort ret; }

                    if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_UL)))) or
                        block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_OL)))) or
                        block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_QUOTE)))))
                    {
                        ctx.*.n_containers -= 1;
                    }
                }

                if ((block_flags & MD_BLOCK_CONTAINER_OPENER) != 0) {
                    ret = ctx.*.parser.enter_block.?(@as(c_uint, block_type), @ptrCast(&det_bytes), ctx.*.userdata);
                    if (ret != 0) { md_log(ctx, "Aborted from enter_block() callback."); break :abort ret; }

                    if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_UL)))) or
                        block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_OL)))))
                    {
                        const containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));
                        containers[@as(usize, @intCast(ctx.*.n_containers))].is_loose = @as(u8, @intCast(block_flags & MD_BLOCK_LOOSE_LIST));
                        ctx.*.n_containers += 1;
                    } else if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_QUOTE))))) {
                        const containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));
                        containers[@as(usize, @intCast(ctx.*.n_containers))].is_loose = 1; // TRUE
                        ctx.*.n_containers += 1;
                    }
                }
            } else {
                ret = md_process_leaf_block(ctx, block);
                if (ret < 0) break :abort ret;

                if (block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_CODE)))) or
                    block_type == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_HTML)))))
                {
                    byte_off += @as(c_int, @intCast(block.n_lines * @as(MD_SIZE, @intCast(@sizeOf(MD_VERBATIMLINE)))));
                } else {
                    byte_off += @as(c_int, @intCast(block.n_lines * @as(MD_SIZE, @intCast(@sizeOf(MD_LINE)))));
                }
            }

            byte_off += @as(c_int, @intCast(@sizeOf(MD_BLOCK)));
        }

        ctx.*.n_block_bytes = 0;
        break :abort ret;
    };
    return ret;
}

pub fn md_push_block_bytes(arg_ctx: [*c]MD_CTX, arg_n_bytes: c_int) callconv(.C) ?*anyopaque {
    var ctx = arg_ctx;
    _ = &ctx;
    var n_bytes = arg_n_bytes;
    _ = &n_bytes;
    var ptr: ?*anyopaque = undefined;
    _ = &ptr;
    if ((ctx.*.n_block_bytes + n_bytes) > ctx.*.alloc_block_bytes) {
        var new_block_bytes: ?*anyopaque = undefined;
        _ = &new_block_bytes;
        ctx.*.alloc_block_bytes = if (ctx.*.alloc_block_bytes > @as(c_int, 0)) ctx.*.alloc_block_bytes + @divTrunc(ctx.*.alloc_block_bytes, @as(c_int, 2)) else @as(c_int, 512);
        new_block_bytes = realloc(ctx.*.block_bytes, @as(c_ulong, @bitCast(@as(c_long, ctx.*.alloc_block_bytes))));
        if (new_block_bytes == @as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("realloc() failed.", ctx.*.userdata);
                }
                if (!false) break;
            }
            return @as(?*anyopaque, @ptrFromInt(@as(c_int, 0)));
        }
        if (ctx.*.current_block != @as(?*MD_BLOCK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            var off_current_block: MD_OFFSET = @as(MD_OFFSET, @bitCast(@as(c_int, @truncate(@divExact(@as(c_long, @bitCast(@intFromPtr(@as([*c]u8, @ptrCast(@alignCast(ctx.*.current_block)))) -% @intFromPtr(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes)))))), @sizeOf(u8))))));
            _ = &off_current_block;
            ctx.*.current_block = @as(?*MD_BLOCK, @ptrCast(@alignCast(@as([*c]u8, @ptrCast(@alignCast(new_block_bytes))) + off_current_block)));
        }
        ctx.*.block_bytes = new_block_bytes;
    }
    ptr = @as(?*anyopaque, @ptrCast(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes))) + @as(usize, @bitCast(@as(isize, @intCast(ctx.*.n_block_bytes))))));
    ctx.*.n_block_bytes += n_bytes;
    return ptr;
}

pub fn md_start_new_block(arg_ctx: [*c]MD_CTX, arg_line: [*c]const MD_LINE_ANALYSIS) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var line = arg_line;
    _ = &line;
    var block: ?*MD_BLOCK = undefined;
    _ = &block;
    while (true) {
        if (!(ctx.*.current_block == @as(?*MD_BLOCK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) {
            unreachable;
        }
        if (!false) break;
    }
    block = @as(?*MD_BLOCK, @ptrCast(@alignCast(md_push_block_bytes(ctx, @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf(MD_BLOCK)))))))));
    if (block == @as(?*MD_BLOCK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) return -@as(c_int, 1);
    while (true) {
        switch (line.*.type) {
            @as(c_uint, @bitCast(@as(c_int, 1))) => {
                block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_HR))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 2))), @as(c_uint, @bitCast(@as(c_int, 3))) => {
                block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_H))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 6))), @as(c_uint, @bitCast(@as(c_int, 5))) => {
                block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_CODE))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 8))) => {
                block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_P))));
                break;
            },
            @as(c_uint, @bitCast(@as(c_int, 7))) => {
                block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_HTML))));
                break;
            },
            else => {
                while (true) {
                    unreachable;
// removed unreachable: if (!false) break;
                }
                break;
            },
        }
        break;
    }
    block.?.setFlags(0);
    block.?.setData(@truncate(line.*.data));
    block.?.n_lines = 0;
    ctx.*.current_block = block;
    return 0;
}

pub fn md_consume_link_reference_definitions(arg_ctx: [*c]MD_CTX) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var lines: [*c]MD_LINE = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(ctx.*.current_block.?)) + @sizeOf(MD_BLOCK)));
    _ = &lines;
    var n_lines: MD_SIZE = ctx.*.current_block.?.*.n_lines;
    _ = &n_lines;
    var n: MD_SIZE = 0;
    _ = &n;
    while (n < n_lines) {
        var n_link_ref_lines: c_int = undefined;
        _ = &n_link_ref_lines;
        n_link_ref_lines = md_is_link_reference_definition(ctx, lines + n, n_lines -% n);
        if (n_link_ref_lines == @as(c_int, 0)) break;
        if (n_link_ref_lines < @as(c_int, 0)) return -@as(c_int, 1);
        n +%= @as(MD_SIZE, @bitCast(n_link_ref_lines));
    }
    if (n > @as(MD_SIZE, @bitCast(@as(c_int, 0)))) {
        if (n == n_lines) {
            ctx.*.n_block_bytes -= @as(c_int, @bitCast(@as(c_uint, @truncate(@as(c_ulong, @bitCast(@as(c_ulong, n))) *% @sizeOf(MD_LINE)))));
            ctx.*.n_block_bytes -= @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf(MD_BLOCK)))));
            ctx.*.current_block = null;
        } else {
            _ = memmove(@as(?*anyopaque, @ptrCast(lines)), @as(?*const anyopaque, @ptrCast(lines + n)), @as(c_ulong, @bitCast(@as(c_ulong, n_lines -% n))) *% @sizeOf(MD_LINE));
            ctx.*.current_block.?.*.n_lines -%= n;
            ctx.*.n_block_bytes -= @as(c_int, @bitCast(@as(c_uint, @truncate(@as(c_ulong, @bitCast(@as(c_ulong, n))) *% @sizeOf(MD_LINE)))));
        }
    }
    return 0;
}

pub fn md_end_current_block(arg_ctx: [*c]MD_CTX) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = 0;

    if (ctx.*.current_block == null)
        return ret;

    abort: {
        // Check whether there is a reference definition.
        if (ctx.*.current_block.?.getType() == MD_BLOCK_P or
            (ctx.*.current_block.?.getType() == MD_BLOCK_H and (ctx.*.current_block.?.getFlags() & MD_BLOCK_SETEXT_HEADER) != 0))
        {
            const lines: [*c]MD_LINE = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(ctx.*.current_block.?)) + @sizeOf(MD_BLOCK)));
            if (lines[0].beg < ctx.*.size and ctx.*.text[lines[0].beg] == '[') {
                ret = md_consume_link_reference_definitions(ctx);
                if (ret < 0) break :abort;
                if (ctx.*.current_block == null)
                    return ret;
            }
        }

        if (ctx.*.current_block.?.getType() == MD_BLOCK_H and (ctx.*.current_block.?.getFlags() & MD_BLOCK_SETEXT_HEADER) != 0) {
            const n_lines = ctx.*.current_block.?.n_lines;
            if (n_lines > 1) {
                ctx.*.current_block.?.n_lines -= 1;
                ctx.*.n_block_bytes -= @sizeOf(MD_LINE);
            } else {
                ctx.*.current_block.?.setType(MD_BLOCK_P);
                return 0;
            }
        }

        ctx.*.current_block = null;
        break :abort;
    }

    return ret;
}

pub fn md_add_line_into_current_block(arg_ctx: [*c]MD_CTX, arg_analysis: [*c]const MD_LINE_ANALYSIS) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var analysis = arg_analysis;
    _ = &analysis;
    while (true) {
        if (!(ctx.*.current_block != @as(?*MD_BLOCK, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) {
            unreachable;
        }
        if (!false) break;
    }
    if ((@as(c_int, @intCast(ctx.*.current_block.?.getType())) == MD_BLOCK_CODE) or (@as(c_int, @intCast(ctx.*.current_block.?.getType())) == MD_BLOCK_HTML)) {
        var line: [*c]MD_VERBATIMLINE = undefined;
        _ = &line;
        line = @as([*c]MD_VERBATIMLINE, @ptrCast(@alignCast(md_push_block_bytes(ctx, @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf(MD_VERBATIMLINE)))))))));
        if (line == @as([*c]MD_VERBATIMLINE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) return -@as(c_int, 1);
        line.*.indent = analysis.*.indent;
        line.*.beg = analysis.*.beg;
        line.*.end = analysis.*.end;
    } else {
        var line: [*c]MD_LINE = undefined;
        _ = &line;
        line = @as([*c]MD_LINE, @ptrCast(@alignCast(md_push_block_bytes(ctx, @as(c_int, @bitCast(@as(c_uint, @truncate(@sizeOf(MD_LINE)))))))));
        if (line == @as([*c]MD_LINE, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) return -@as(c_int, 1);
        line.*.beg = analysis.*.beg;
        line.*.end = analysis.*.end;
    }
    ctx.*.current_block.?.*.n_lines +%= 1;
    return 0;
}

pub fn md_push_container_bytes(arg_ctx: [*c]MD_CTX, arg_type: MD_BLOCKTYPE, arg_start: c_uint, arg_data: c_uint, arg_flags: c_uint) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = 0;

    abort: {
        ret = md_end_current_block(ctx);
        if (ret < 0) break :abort;

        const block_ptr = md_push_block_bytes(ctx, @sizeOf(MD_BLOCK));
        if (block_ptr == null)
            return -1;
        const block: *MD_BLOCK = @ptrCast(@alignCast(block_ptr));
        block.setType(@truncate(arg_type));
        block.setFlags(@truncate(arg_flags));
        block.setData(@truncate(arg_data));
        block.n_lines = arg_start;
        break :abort;
    }

    return ret;
}

pub fn md_is_hr_line(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_killer: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_killer = arg_p_killer;
    _ = &p_killer;
    var off: MD_OFFSET = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    _ = &off;
    var n: c_int = 1;
    _ = &n;
    while ((off < ctx.*.size) and (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg])))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' '))) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')))) {
        if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg])))) {
            n += 1;
        }
        off +%= 1;
    }
    if (n < @as(c_int, 3)) {
        p_killer.* = off;
        return 0;
    }
    if ((off < ctx.*.size) and !((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n')))) {
        p_killer.* = off;
        return 0;
    }
    p_end.* = off;
    return 1;
}

pub fn md_is_atxheader_line(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_beg: [*c]MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_level: [*c]c_uint) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var p_beg = arg_p_beg;
    _ = &p_beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_level = arg_p_level;
    _ = &p_level;
    var n: c_int = undefined;
    _ = &n;
    var off: MD_OFFSET = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    _ = &off;
    while (((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '#'))) and ((off -% beg) < @as(MD_OFFSET, @bitCast(@as(c_int, 7))))) {
        off +%= 1;
    }
    n = @as(c_int, @bitCast(off -% beg));
    if (n > @as(c_int, 6)) return 0;
    p_level.* = @as(c_uint, @bitCast(n));
    if (((!((ctx.*.parser.flags & @as(c_uint, @bitCast(@as(c_int, 2)))) != 0) and (off < ctx.*.size)) and !((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')))) and !((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n')))) return 0;
    while ((off < ctx.*.size) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')))) {
        off +%= 1;
    }
    p_beg.* = off;
    p_end.* = off;
    return 1;
}

pub fn md_is_setext_underline(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_level: [*c]c_uint) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_level = arg_p_level;
    _ = &p_level;
    var off: MD_OFFSET = beg +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
    _ = &off;
    while ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))))) {
        off +%= 1;
    }
    while ((off < ctx.*.size) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')))) {
        off +%= 1;
    }
    if ((off < ctx.*.size) and !((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n')))) return 0;
    p_level.* = @as(c_uint, @bitCast(if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '=')) @as(c_int, 1) else @as(c_int, 2)));
    p_end.* = off;
    return 1;
}

pub fn md_is_table_underline(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_col_count: [*c]c_uint) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var p_col_count = arg_p_col_count;
    _ = &p_col_count;
    var off: MD_OFFSET = beg;
    _ = &off;
    var found_pipe: c_int = 0;
    _ = &found_pipe;
    var col_count: c_uint = 0;
    _ = &col_count;
    if ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '|'))) {
        found_pipe = 1;
        off +%= 1;
        while ((off < ctx.*.size) and (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c'))))) {
            off +%= 1;
        }
    }
    while (true) {
        var delimited: c_int = 0;
        _ = &delimited;
        if ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ':'))) {
            off +%= 1;
        }
        if ((off >= ctx.*.size) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '-'))) return 0;
        while ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '-'))) {
            off +%= 1;
        }
        if ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ':'))) {
            off +%= 1;
        }
        col_count +%= 1;
        if (col_count > @as(c_uint, @bitCast(@as(c_int, 128)))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("Suppressing table (column_count >128)", ctx.*.userdata);
                }
                if (!false) break;
            }
            return 0;
        }
        while ((off < ctx.*.size) and (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c'))))) {
            off +%= 1;
        }
        if ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '|'))) {
            delimited = 1;
            found_pipe = 1;
            off +%= 1;
            while ((off < ctx.*.size) and (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t'))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0b')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\x0c'))))) {
                off +%= 1;
            }
        }
        if ((off >= ctx.*.size) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n')))) break;
        if (!(delimited != 0)) return 0;
    }
    if (!(found_pipe != 0)) return 0;
    p_end.* = off;
    p_col_count.* = col_count;
    return 1;
}

pub fn md_is_opening_code_fence(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    while ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))))) {
        off +%= 1;
    }
    if ((off -% beg) < @as(MD_OFFSET, @bitCast(@as(c_int, 3)))) return 0;
    ctx.*.code_fence_length = off -% beg;
    while ((off < ctx.*.size) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' '))) {
        off +%= 1;
    }
    while ((off < ctx.*.size) and !((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\n')))) {
        if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[beg]))) == @as(c_int, '`')) and (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '`'))) return 0;
        off +%= 1;
    }
    p_end.* = off;
    return 1;
}

pub fn md_is_closing_code_fence(arg_ctx: [*c]MD_CTX, arg_ch: MD_CHAR, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    const ctx = arg_ctx;
    const ch = arg_ch;
    var off: MD_OFFSET = arg_beg;
    const p_end = arg_p_end;

    const ret: c_int = out: {
        // Closing fence must have at least the same length and use same char as opening one.
        while (off < ctx.*.size and ctx.*.text[off] == ch)
            off += 1;
        if (off -% arg_beg < ctx.*.code_fence_length)
            break :out FALSE;

        // Optionally, space(s) can follow
        while (off < ctx.*.size and ctx.*.text[off] == ' ')
            off += 1;

        // But nothing more is allowed on the line.
        if (off < ctx.*.size and !(ctx.*.text[off] == '\r' or ctx.*.text[off] == '\n'))
            break :out FALSE;

        break :out TRUE;
    };

    // Note we set *p_end even on failure
    p_end.* = off;
    return ret;
}

pub fn md_is_html_block_start_condition(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET) callconv(.C) c_int {
    const ctx = arg_ctx;
    const text = ctx.*.text;
    const size = ctx.*.size;
    var off: MD_OFFSET = arg_beg + 1;

    // Check for type 1: <script, <pre, or <style, <textarea
    for (&t1) |*tag| {
        if (tag.name == null) break;
        if (off + tag.len <= size) {
            if (md_ascii_case_eq(text + off, tag.name, tag.len) != 0) return 1;
        }
    }

    // Check for type 2: <!--
    if (off + 3 < size and text[off] == '!' and text[off + 1] == '-' and text[off + 2] == '-') return 2;

    // Check for type 3: <?
    if (off < size and text[off] == '?') return 3;

    // Check for type 4: <! + uppercase letter
    if (off < size and text[off] == '!') {
        if (off + 1 < size and text[off + 1] <= 127) return 4;
        if (off + 8 < size) {
            if (md_ascii_eq(text + off, "![CDATA[", 8) != 0) return 5;
        }
    }

    // Check for type 6: Many possible starting tags
    const isAlpha = struct {
        fn f(ch: u8) bool {
            return (ch >= 'A' and ch <= 'Z') or (ch >= 'a' and ch <= 'z');
        }
    }.f;

    if (off + 1 < size and (isAlpha(text[off]) or (text[off] == '/' and isAlpha(text[off + 1])))) {
        if (text[off] == '/') off += 1;

        const ch = text[off];
        const slot: usize = if (ch >= 'A' and ch <= 'Z') @as(usize, ch - 'A') else @as(usize, ch - 'a');
        const tags = map6[slot];

        for (tags) |tag| {
            if (tag.name == null) break;
            if (off + tag.len <= size) {
                if (md_ascii_case_eq(text + off, tag.name, tag.len) != 0) {
                    const tmp = off + tag.len;
                    if (tmp >= size) return 6;
                    const c = text[tmp];
                    if (c == ' ' or c == '\t' or c == '\r' or c == '\n' or c == '>') return 6;
                    if (tmp + 1 < size and c == '/' and text[tmp + 1] == '>') return 6;
                    break;
                }
            }
        }
    }

    // Check for type 7: any COMPLETE other opening or closing tag
    if (off + 1 < size) {
        var end: MD_OFFSET = undefined;
        if (md_is_html_tag(ctx, null, 0, arg_beg, size, &end) != 0) {
            // Only optional whitespace and new line may follow.
            while (end < size and (text[end] == ' ' or text[end] == '\t' or text[end] == '\x0b' or text[end] == '\x0c')) {
                end += 1;
            }
            if (end >= size or text[end] == '\r' or text[end] == '\n') return 7;
        }
    }

    return FALSE;
}

pub fn md_line_contains(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_what: [*c]const MD_CHAR, arg_what_len: MD_SIZE, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var beg = arg_beg;
    _ = &beg;
    var what = arg_what;
    _ = &what;
    var what_len = arg_what_len;
    _ = &what_len;
    var p_end = arg_p_end;
    _ = &p_end;
    var i: MD_OFFSET = undefined;
    _ = &i;
    {
        i = beg;
        while ((i +% what_len) < ctx.*.size) : (i +%= 1) {
            if ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[i]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[i]))) == @as(c_int, '\n'))) break;
            if (memcmp(@as(?*const anyopaque, @ptrCast(ctx.*.text + i)), @as(?*const anyopaque, @ptrCast(what)), @as(c_ulong, @bitCast(@as(c_ulong, what_len))) *% @sizeOf(MD_CHAR)) == @as(c_int, 0)) {
                p_end.* = i +% what_len;
                return 1;
            }
        }
    }
    p_end.* = i;
    return 0;
}

pub fn md_is_html_block_end_condition(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_int {
    const ctx = arg_ctx;
    const beg = arg_beg;
    const p_end = arg_p_end;
    const text = ctx.*.text;
    const size = ctx.*.size;

    switch (ctx.*.html_block_type) {
        1 => {
            var off: MD_OFFSET = beg;
            while (off + 1 < size and !(text[off] == '\r' or text[off] == '\n')) {
                if (text[off] == '<' and text[off + 1] == '/') {
                    for (&t1) |*tag| {
                        if (tag.name == null) break;
                        if (off + 2 + tag.len < size) {
                            if (md_ascii_case_eq(text + off + 2, tag.name, tag.len) != 0 and text[off + 2 + tag.len] == '>') {
                                p_end.* = off + 2 + tag.len + 1;
                                return TRUE;
                            }
                        }
                    }
                }
                off += 1;
            }
            p_end.* = off;
            return FALSE;
        },
        2 => return if (md_line_contains(ctx, beg, "-->", 3, p_end) != 0) @as(c_int, 2) else FALSE,
        3 => return if (md_line_contains(ctx, beg, "?>", 2, p_end) != 0) @as(c_int, 3) else FALSE,
        4 => return if (md_line_contains(ctx, beg, ">", 1, p_end) != 0) @as(c_int, 4) else FALSE,
        5 => return if (md_line_contains(ctx, beg, "]]>", 3, p_end) != 0) @as(c_int, 5) else FALSE,
        6, 7 => {
            if (beg >= size or text[beg] == '\r' or text[beg] == '\n') {
                p_end.* = beg;
                return ctx.*.html_block_type;
            }
            return FALSE;
        },
        else => unreachable,
    }
    return FALSE;
}

pub fn md_is_container_compatible(arg_pivot: ?*const MD_CONTAINER, arg_container: ?*const MD_CONTAINER) callconv(.C) c_int {
    const pivot = arg_pivot.?;
    const container = arg_container.?;
    if (@as(c_int, @bitCast(@as(c_uint, container.*.ch))) == @as(c_int, '>')) return 0;
    if (@as(c_int, @bitCast(@as(c_uint, container.*.ch))) != @as(c_int, @bitCast(@as(c_uint, pivot.*.ch)))) return 0;
    if (container.*.mark_indent > pivot.*.contents_indent) return 0;
    return 1;
}

pub fn md_push_container(arg_ctx: [*c]MD_CTX, arg_container: ?*const MD_CONTAINER) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var container = arg_container;
    _ = &container;
    if (ctx.*.n_containers >= ctx.*.alloc_containers) {
        var new_containers: ?*MD_CONTAINER = undefined;
        _ = &new_containers;
        ctx.*.alloc_containers = if (ctx.*.alloc_containers > @as(c_int, 0)) ctx.*.alloc_containers + @divTrunc(ctx.*.alloc_containers, @as(c_int, 2)) else @as(c_int, 16);
        new_containers = @as(?*MD_CONTAINER, @ptrCast(@alignCast(realloc(@as(?*anyopaque, @ptrCast(ctx.*.containers)), @as(c_ulong, @bitCast(@as(c_long, ctx.*.alloc_containers))) *% @sizeOf(MD_CONTAINER)))));
        if (new_containers == @as(?*MD_CONTAINER, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
            while (true) {
                if (ctx.*.parser.debug_log != @as(?*const fn ([*c]const u8, ?*anyopaque) callconv(.C) void, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0))))))) {
                    ctx.*.parser.debug_log.?("realloc() failed.", ctx.*.userdata);
                }
                if (!false) break;
            }
            return -@as(c_int, 1);
        }
        ctx.*.containers = new_containers;
    }
    _ = memcpy(@as(?*anyopaque, @ptrCast(&(blk: {
        const tmp = blk_1: {
            const ref = &ctx.*.n_containers;
            const tmp_2 = ref.*;
            ref.* += 1;
            break :blk_1 tmp_2;
        };
        if (tmp >= 0) break :blk @as([*c]MD_CONTAINER, @ptrCast(@alignCast(ctx.*.containers))) + @as(usize, @intCast(tmp)) else break :blk @as([*c]MD_CONTAINER, @ptrCast(@alignCast(ctx.*.containers))) - ~@as(usize, @bitCast(@as(isize, @intCast(tmp)) +% -1));
    }).*)), @as(?*const anyopaque, @ptrCast(container)), @sizeOf(MD_CONTAINER));
    return 0;
}

pub fn md_enter_child_containers(arg_ctx: [*c]MD_CTX, arg_n_children: c_int) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = 0;
    const containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));

    abort: {
        var i: c_int = ctx.*.n_containers - arg_n_children;
        while (i < ctx.*.n_containers) : (i += 1) {
            const ci: usize = @intCast(i);
            const c_ch = containers[ci].ch;
            var is_ordered_list: c_int = FALSE;

            if (c_ch == ')' or c_ch == '.') {
                is_ordered_list = TRUE;
                // fall through to list handling
            }
            if (c_ch == ')' or c_ch == '.' or c_ch == '-' or c_ch == '+' or c_ch == '*') {
                _ = md_end_current_block(ctx);
                containers[ci].block_byte_off = @bitCast(ctx.*.n_block_bytes);

                ret = md_push_container_bytes(ctx, if (is_ordered_list != 0) MD_BLOCK_OL else MD_BLOCK_UL, containers[ci].start, containers[ci].ch, MD_BLOCK_CONTAINER_OPENER);
                if (ret < 0) break :abort;
                ret = md_push_container_bytes(ctx, MD_BLOCK_LI, containers[ci].task_mark_off, if (containers[ci].is_task != 0) @as(c_uint, ctx.*.text[containers[ci].task_mark_off]) else 0, MD_BLOCK_CONTAINER_OPENER);
                if (ret < 0) break :abort;
            } else if (c_ch == '>') {
                ret = md_push_container_bytes(ctx, MD_BLOCK_QUOTE, 0, 0, MD_BLOCK_CONTAINER_OPENER);
                if (ret < 0) break :abort;
            }
        }
        break :abort;
    }

    return ret;
}

pub fn md_leave_child_containers(arg_ctx: [*c]MD_CTX, arg_n_keep: c_int) callconv(.C) c_int {
    const ctx = arg_ctx;
    var ret: c_int = 0;
    const containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));

    abort: {
        while (ctx.*.n_containers > arg_n_keep) {
            const ci: usize = @intCast(ctx.*.n_containers - 1);
            const c_ch = containers[ci].ch;
            var is_ordered_list: c_int = FALSE;

            if (c_ch == ')' or c_ch == '.') {
                is_ordered_list = TRUE;
            }
            if (c_ch == ')' or c_ch == '.' or c_ch == '-' or c_ch == '+' or c_ch == '*') {
                ret = md_push_container_bytes(ctx, MD_BLOCK_LI, containers[ci].task_mark_off, if (containers[ci].is_task != 0) @as(c_uint, ctx.*.text[containers[ci].task_mark_off]) else 0, MD_BLOCK_CONTAINER_CLOSER);
                if (ret < 0) break :abort;
                ret = md_push_container_bytes(ctx, if (is_ordered_list != 0) MD_BLOCK_OL else MD_BLOCK_UL, 0, containers[ci].ch, MD_BLOCK_CONTAINER_CLOSER);
                if (ret < 0) break :abort;
            } else if (c_ch == '>') {
                ret = md_push_container_bytes(ctx, MD_BLOCK_QUOTE, 0, 0, MD_BLOCK_CONTAINER_CLOSER);
                if (ret < 0) break :abort;
            }

            ctx.*.n_containers -= 1;
        }
        break :abort;
    }

    return ret;
}

pub fn md_is_container_mark(arg_ctx: [*c]MD_CTX, arg_indent: c_uint, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_p_container: ?*MD_CONTAINER) callconv(.C) c_int {
    var ctx = arg_ctx;
    _ = &ctx;
    var indent = arg_indent;
    _ = &indent;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    const p_container = arg_p_container.?;
    var off: MD_OFFSET = beg;
    _ = &off;
    var max_end: MD_OFFSET = undefined;
    _ = &max_end;
    if ((off >= ctx.*.size) or (indent >= ctx.*.code_indent_offset)) return 0;
    if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '>')) {
        off +%= 1;
        p_container.*.ch = '>';
        p_container.*.is_loose = 0;
        p_container.*.is_task = 0;
        p_container.*.mark_indent = indent;
        p_container.*.contents_indent = indent +% @as(c_uint, @bitCast(@as(c_int, 1)));
        p_end.* = off;
        return 1;
    }
    if (((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) != @as(c_int, '\x00')) and (strchr("-+*", @as(c_int, @bitCast(@as(c_uint, ctx.*.text[off])))) != @as([*c]u8, @ptrCast(@alignCast(@as(?*anyopaque, @ptrFromInt(@as(c_int, 0)))))))) and ((((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) >= ctx.*.size) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\t')))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\n'))))) {
        p_container.*.ch = ctx.*.text[off];
        p_container.*.is_loose = 0;
        p_container.*.is_task = 0;
        p_container.*.mark_indent = indent;
        p_container.*.contents_indent = indent +% @as(c_uint, @bitCast(@as(c_int, 1)));
        p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
        return 1;
    }
    max_end = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 9)));
    if (max_end > ctx.*.size) {
        max_end = ctx.*.size;
    }
    p_container.*.start = 0;
    while ((off < max_end) and ((@as(c_uint, @bitCast(@as(c_int, '0'))) <= @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) and (@as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off]))) <= @as(c_uint, @bitCast(@as(c_int, '9')))))) {
        p_container.*.start = ((p_container.*.start *% @as(c_uint, @bitCast(@as(c_int, 10)))) +% @as(c_uint, @bitCast(@as(c_uint, ctx.*.text[off])))) -% @as(c_uint, @bitCast(@as(c_int, '0')));
        off +%= 1;
    }
    if ((((off > beg) and (off < ctx.*.size)) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '.')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ')')))) and ((((off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))) >= ctx.*.size) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\t')))) or ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\r')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)))]))) == @as(c_int, '\n'))))) {
        p_container.*.ch = ctx.*.text[off];
        p_container.*.is_loose = 0;
        p_container.*.is_task = 0;
        p_container.*.mark_indent = indent;
        p_container.*.contents_indent = ((indent +% off) -% beg) +% @as(c_uint, @bitCast(@as(c_int, 1)));
        p_end.* = off +% @as(MD_OFFSET, @bitCast(@as(c_int, 1)));
        return 1;
    }
    return 0;
}

pub fn md_line_indentation(arg_ctx: [*c]MD_CTX, arg_total_indent: c_uint, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET) callconv(.C) c_uint {
    var ctx = arg_ctx;
    _ = &ctx;
    var total_indent = arg_total_indent;
    _ = &total_indent;
    var beg = arg_beg;
    _ = &beg;
    var p_end = arg_p_end;
    _ = &p_end;
    var off: MD_OFFSET = beg;
    _ = &off;
    var indent: c_uint = total_indent;
    _ = &indent;
    while ((off < ctx.*.size) and ((@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, ' ')) or (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')))) {
        if (@as(c_int, @bitCast(@as(c_uint, ctx.*.text[off]))) == @as(c_int, '\t')) {
            indent = (indent +% @as(c_uint, @bitCast(@as(c_int, 4)))) & @as(c_uint, @bitCast(~@as(c_int, 3)));
        } else {
            indent +%= 1;
        }
        off +%= 1;
    }
    p_end.* = off;
    return indent -% total_indent;
}

pub fn md_analyze_line(arg_ctx: [*c]MD_CTX, arg_beg: MD_OFFSET, arg_p_end: [*c]MD_OFFSET, arg_pivot_line: [*c]const MD_LINE_ANALYSIS, arg_line: [*c]MD_LINE_ANALYSIS) callconv(.C) c_int {
    const ctx = arg_ctx;
    const line = arg_line;
    var pivot_line: [*c]const MD_LINE_ANALYSIS = arg_pivot_line;
    // NOTE: must refresh after md_push_container() which may realloc
    var containers: [*c]MD_CONTAINER = @ptrCast(@alignCast(ctx.*.containers));
    var total_indent: c_uint = 0;
    var n_parents: c_int = 0;
    var n_brothers: c_int = 0;
    var n_children: c_int = 0;
    var container: MD_CONTAINER = @import("std").mem.zeroes(MD_CONTAINER);
    const prev_line_has_list_loosening_effect = ctx.*.last_line_has_list_loosening_effect;
    var off: MD_OFFSET = arg_beg;
    var hr_killer: MD_OFFSET = 0;
    var ret: c_int = 0;

    line.*.indent = md_line_indentation(ctx, total_indent, off, &off);
    total_indent +%= line.*.indent;
    line.*.beg = off;
    line.*.enforce_new_block = FALSE;

    // Determine how many of the current containers are our parents
    while (n_parents < ctx.*.n_containers) {
        const c = &containers[@as(usize, @intCast(n_parents))];

        if (c.*.ch == '>' and line.*.indent < ctx.*.code_indent_offset and
            off < ctx.*.size and ctx.*.text[off] == '>')
        {
            // Block quote mark
            off +%= 1;
            total_indent +%= 1;
            line.*.indent = md_line_indentation(ctx, total_indent, off, &off);
            total_indent +%= line.*.indent;

            // The optional 1st space after '>' is part of the block quote mark
            if (line.*.indent > 0)
                line.*.indent -= 1;

            line.*.beg = off;
        } else if (c.*.ch != '>' and line.*.indent >= c.*.contents_indent) {
            // List
            line.*.indent -%= c.*.contents_indent;
        } else {
            break;
        }

        n_parents += 1;
    }

    if (off >= ctx.*.size or (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r')) {
        // Blank line does not need any real indentation to be nested inside a list
        if (n_brothers + n_children == 0) {
            while (n_parents < ctx.*.n_containers and containers[@as(usize, @intCast(n_parents))].ch != '>')
                n_parents += 1;
        }
    }

    while (true) {
        // Check whether we are fenced code continuation
        if (pivot_line.*.type == MD_LINE_FENCEDCODE) {
            line.*.beg = off;

            // We are another MD_LINE_FENCEDCODE unless we are closing fence
            if (line.*.indent < ctx.*.code_indent_offset) {
                if (md_is_closing_code_fence(ctx, ctx.*.text[pivot_line.*.beg], off, &off) != 0) {
                    line.*.type = MD_LINE_BLANK;
                    ctx.*.last_line_has_list_loosening_effect = FALSE;
                    break;
                }
            }

            // Change indentation accordingly to the initial code fence
            if (n_parents == ctx.*.n_containers) {
                if (line.*.indent > pivot_line.*.indent)
                    line.*.indent -%= pivot_line.*.indent
                else
                    line.*.indent = 0;

                line.*.type = MD_LINE_FENCEDCODE;
                break;
            }
        }

        // Check whether we are HTML block continuation
        if (pivot_line.*.type == MD_LINE_HTML and ctx.*.html_block_type > 0) {
            if (n_parents < ctx.*.n_containers) {
                // HTML block is implicitly ended if the enclosing container block ends
                ctx.*.html_block_type = 0;
            } else {
                var html_block_type: c_int = undefined;

                html_block_type = md_is_html_block_end_condition(ctx, off, &off);
                if (html_block_type > 0) {
                    // Make sure this is the last line of the block
                    ctx.*.html_block_type = 0;

                    // Some end conditions serve as blank lines at the same time
                    if (html_block_type == 6 or html_block_type == 7) {
                        line.*.type = MD_LINE_BLANK;
                        line.*.indent = 0;
                        break;
                    }
                }

                line.*.type = MD_LINE_HTML;
                n_parents = ctx.*.n_containers;
                break;
            }
        }

        // Check for blank line
        if (off >= ctx.*.size or (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r')) {
            if (pivot_line.*.type == MD_LINE_INDENTEDCODE and n_parents == ctx.*.n_containers) {
                line.*.type = MD_LINE_INDENTEDCODE;
                if (line.*.indent > ctx.*.code_indent_offset)
                    line.*.indent -%= ctx.*.code_indent_offset
                else
                    line.*.indent = 0;
                ctx.*.last_line_has_list_loosening_effect = FALSE;
            } else {
                line.*.type = MD_LINE_BLANK;
                ctx.*.last_line_has_list_loosening_effect = @intFromBool(n_parents > 0 and
                    n_brothers + n_children == 0 and
                    containers[@as(usize, @intCast(n_parents - 1))].ch != '>');

                // Check for two blank lines at start of list item (issue #6)
                if (n_parents > 0 and containers[@as(usize, @intCast(n_parents - 1))].ch != '>' and
                    n_brothers + n_children == 0 and ctx.*.current_block == null and
                    ctx.*.n_block_bytes > @as(c_int, @intCast(@sizeOf(MD_BLOCK))))
                {
                    const top_block: *MD_BLOCK = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes))) + @as(usize, @intCast(ctx.*.n_block_bytes - @as(c_int, @intCast(@sizeOf(MD_BLOCK)))))));
                    if (top_block.getType() == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_LI)))))
                        ctx.*.last_list_item_starts_with_two_blank_lines = TRUE;
                }
            }
            break;
        } else {
            // 2nd half of two-blank-lines hack
            if (ctx.*.last_list_item_starts_with_two_blank_lines != 0) {
                if (n_parents > 0 and n_parents == ctx.*.n_containers and
                    containers[@as(usize, @intCast(n_parents - 1))].ch != '>' and
                    n_brothers + n_children == 0 and ctx.*.current_block == null and
                    ctx.*.n_block_bytes > @as(c_int, @intCast(@sizeOf(MD_BLOCK))))
                {
                    const top_block: *MD_BLOCK = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes))) + @as(usize, @intCast(ctx.*.n_block_bytes - @as(c_int, @intCast(@sizeOf(MD_BLOCK)))))));
                    if (top_block.getType() == @as(u8, @truncate(@as(c_uint, @bitCast(MD_BLOCK_LI))))) {
                        n_parents -= 1;

                        line.*.indent = total_indent;
                        if (n_parents > 0)
                            line.*.indent -%= MIN(line.*.indent, containers[@as(usize, @intCast(n_parents - 1))].contents_indent);
                    }
                }

                ctx.*.last_list_item_starts_with_two_blank_lines = FALSE;
            }
            ctx.*.last_line_has_list_loosening_effect = FALSE;
        }

        // Check whether we are Setext underline
        if (line.*.indent < ctx.*.code_indent_offset and pivot_line.*.type == MD_LINE_TEXT and
            off < ctx.*.size and (ctx.*.text[off] == '=' or ctx.*.text[off] == '-') and
            (n_parents == ctx.*.n_containers))
        {
            var level: c_uint = undefined;
            if (md_is_setext_underline(ctx, off, &off, &level) != 0) {
                line.*.type = MD_LINE_SETEXTUNDERLINE;
                line.*.data = level;
                break;
            }
        }

        // Check for thematic break line
        if (line.*.indent < ctx.*.code_indent_offset and
            off < ctx.*.size and off >= hr_killer and
            (ctx.*.text[off] == '-' or ctx.*.text[off] == '_' or ctx.*.text[off] == '*'))
        {
            if (md_is_hr_line(ctx, off, &off, &hr_killer) != 0) {
                line.*.type = MD_LINE_HR;
                break;
            }
        }

        // Check for "brother" container
        if (n_parents < ctx.*.n_containers and n_brothers + n_children == 0) {
            var tmp: MD_OFFSET = undefined;

            if (md_is_container_mark(ctx, line.*.indent, off, &tmp, &container) != 0 and
                md_is_container_compatible(&containers[@as(usize, @intCast(n_parents))], &container) != 0)
            {
                pivot_line = &md_dummy_blank_line;

                off = tmp;

                total_indent +%= container.contents_indent -% container.mark_indent;
                line.*.indent = md_line_indentation(ctx, total_indent, off, &off);
                total_indent +%= line.*.indent;
                line.*.beg = off;

                // Some of the following whitespace actually still belongs to the mark
                if (off >= ctx.*.size or (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r')) {
                    container.contents_indent +%= 1;
                } else if (line.*.indent <= ctx.*.code_indent_offset) {
                    container.contents_indent +%= line.*.indent;
                    line.*.indent = 0;
                } else {
                    container.contents_indent +%= 1;
                    line.*.indent -%= 1;
                }

                containers[@as(usize, @intCast(n_parents))].mark_indent = container.mark_indent;
                containers[@as(usize, @intCast(n_parents))].contents_indent = container.contents_indent;

                n_brothers += 1;
                continue;
            }
        }

        // Check for indented code
        if (line.*.indent >= ctx.*.code_indent_offset and (pivot_line.*.type != MD_LINE_TEXT)) {
            line.*.type = MD_LINE_INDENTEDCODE;
            line.*.indent -%= ctx.*.code_indent_offset;
            line.*.data = 0;
            break;
        }

        // Check for start of a new container block
        if (line.*.indent < ctx.*.code_indent_offset and
            md_is_container_mark(ctx, line.*.indent, off, &off, &container) != 0)
        {
            if (pivot_line.*.type == MD_LINE_TEXT and n_parents == ctx.*.n_containers and
                (off >= ctx.*.size or (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r')) and container.ch != '>')
            {
                // Noop. List mark followed by a blank line cannot interrupt a paragraph.
            } else if (pivot_line.*.type == MD_LINE_TEXT and n_parents == ctx.*.n_containers and
                (container.ch == '.' or container.ch == ')') and container.start != 1)
            {
                // Noop. Ordered list cannot interrupt a paragraph unless start index is 1.
            } else {
                total_indent +%= container.contents_indent -% container.mark_indent;
                line.*.indent = md_line_indentation(ctx, total_indent, off, &off);
                total_indent +%= line.*.indent;

                line.*.beg = off;
                line.*.data = @as(c_uint, container.ch);

                // Some of the following whitespace actually still belongs to the mark
                if (off >= ctx.*.size or (ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r')) {
                    container.contents_indent +%= 1;
                } else if (line.*.indent <= ctx.*.code_indent_offset) {
                    container.contents_indent +%= line.*.indent;
                    line.*.indent = 0;
                } else {
                    container.contents_indent +%= 1;
                    line.*.indent -%= 1;
                }

                if (n_brothers + n_children == 0)
                    pivot_line = &md_dummy_blank_line;

                if (n_children == 0) {
                    ret = md_leave_child_containers(ctx, n_parents + n_brothers);
                    if (ret < 0) {
                        return ret;
                    }
                }

                n_children += 1;
                ret = md_push_container(ctx, &container);
                if (ret < 0) {
                    return ret;
                }
                // Refresh after potential realloc in md_push_container
                containers = @ptrCast(@alignCast(ctx.*.containers));
                continue;
            }
        }

        // Check whether we are table continuation
        if (pivot_line.*.type == MD_LINE_TABLE and n_parents == ctx.*.n_containers) {
            line.*.type = MD_LINE_TABLE;
            break;
        }

        // Check for ATX header
        if (line.*.indent < ctx.*.code_indent_offset and
            off < ctx.*.size and ctx.*.text[off] == '#')
        {
            var level: c_uint = undefined;

            if (md_is_atxheader_line(ctx, off, &line.*.beg, &off, &level) != 0) {
                line.*.type = MD_LINE_ATXHEADER;
                line.*.data = level;
                break;
            }
        }

        // Check whether we are starting code fence
        if (line.*.indent < ctx.*.code_indent_offset and
            off < ctx.*.size and (ctx.*.text[off] == '`' or ctx.*.text[off] == '~'))
        {
            if (md_is_opening_code_fence(ctx, off, &off) != 0) {
                line.*.type = MD_LINE_FENCEDCODE;
                line.*.data = 1;
                line.*.enforce_new_block = TRUE;
                break;
            }
        }

        // Check for start of raw HTML block
        if (off < ctx.*.size and ctx.*.text[off] == '<' and
            (ctx.*.parser.flags & 0x0020) == 0) // MD_FLAG_NOHTMLBLOCKS
        {
            ctx.*.html_block_type = md_is_html_block_start_condition(ctx, off);

            // HTML block type 7 cannot interrupt paragraph
            if (ctx.*.html_block_type == 7 and pivot_line.*.type == MD_LINE_TEXT)
                ctx.*.html_block_type = 0;

            if (ctx.*.html_block_type > 0) {
                // The line itself also may immediately close the block
                if (md_is_html_block_end_condition(ctx, off, &off) == ctx.*.html_block_type) {
                    // Make sure this is the last line of the block
                    ctx.*.html_block_type = 0;
                }

                line.*.enforce_new_block = TRUE;
                line.*.type = MD_LINE_HTML;
                break;
            }
        }

        // Check for table underline
        if ((ctx.*.parser.flags & 0x0100) != 0 and pivot_line.*.type == MD_LINE_TEXT and // MD_FLAG_TABLES
            off < ctx.*.size and (ctx.*.text[off] == '|' or ctx.*.text[off] == '-' or ctx.*.text[off] == ':') and
            n_parents == ctx.*.n_containers)
        {
            var col_count: c_uint = undefined;

            if (ctx.*.current_block != null and ctx.*.current_block.?.n_lines == 1 and
                md_is_table_underline(ctx, off, &off, &col_count) != 0)
            {
                line.*.data = col_count;
                line.*.type = MD_LINE_TABLEUNDERLINE;
                break;
            }
        }

        // By default, we are normal text line
        line.*.type = MD_LINE_TEXT;
        if (pivot_line.*.type == MD_LINE_TEXT and n_brothers + n_children == 0) {
            // Lazy continuation
            n_parents = ctx.*.n_containers;
        }

        // Check for task mark
        if ((ctx.*.parser.flags & 0x0800) != 0 and n_brothers + n_children > 0 and // MD_FLAG_TASKLISTS
            blk: {
                const last_ch = containers[@as(usize, @intCast(ctx.*.n_containers - 1))].ch;
                break :blk (last_ch == '-' or last_ch == '+' or last_ch == '*' or last_ch == '.' or last_ch == ')');
            })
        {
            var tmp: MD_OFFSET = off;

            while (tmp < ctx.*.size and tmp < off +% 3 and (ctx.*.text[tmp] == ' ' or ctx.*.text[tmp] == '\t'))
                tmp +%= 1;
            if (tmp +% 2 < ctx.*.size and ctx.*.text[tmp] == '[' and
                (ctx.*.text[tmp +% 1] == 'x' or ctx.*.text[tmp +% 1] == 'X' or ctx.*.text[tmp +% 1] == ' ') and
                ctx.*.text[tmp +% 2] == ']' and
                (tmp +% 3 == ctx.*.size or ctx.*.text[tmp +% 3] == ' ' or ctx.*.text[tmp +% 3] == '\t' or
                    ctx.*.text[tmp +% 3] == '\n' or ctx.*.text[tmp +% 3] == '\r'))
            {
                const task_container: *MD_CONTAINER = if (n_children > 0) @ptrCast(&containers[@as(usize, @intCast(ctx.*.n_containers - 1))]) else &container;
                task_container.*.is_task = 1; // TRUE
                task_container.*.task_mark_off = tmp +% 1;
                off = tmp +% 3;
                while (off < ctx.*.size and (ctx.*.text[off] == ' ' or ctx.*.text[off] == '\t' or ctx.*.text[off] == '\n' or ctx.*.text[off] == '\r'))
                    off +%= 1;
                line.*.beg = off;
            }
        }

        break;
    }

    // Scan for end of the line (use loop unrolling optimization)
    {
        while (off +% 3 < ctx.*.size and
            ctx.*.text[off] != '\n' and ctx.*.text[off] != '\r' and
            ctx.*.text[off +% 1] != '\n' and ctx.*.text[off +% 1] != '\r' and
            ctx.*.text[off +% 2] != '\n' and ctx.*.text[off +% 2] != '\r' and
            ctx.*.text[off +% 3] != '\n' and ctx.*.text[off +% 3] != '\r')
        {
            off +%= 4;
        }
        while (off < ctx.*.size and ctx.*.text[off] != '\n' and ctx.*.text[off] != '\r')
            off +%= 1;
    }

    // Set end of the line
    line.*.end = off;

    // For ATX header, exclude the optional trailing mark
    if (line.*.type == MD_LINE_ATXHEADER) {
        var tmp: MD_OFFSET = line.*.end;
        while (tmp > line.*.beg and (ctx.*.text[tmp -% 1] == ' ' or ctx.*.text[tmp -% 1] == '\t'))
            tmp -%= 1;
        while (tmp > line.*.beg and ctx.*.text[tmp -% 1] == '#')
            tmp -%= 1;
        if (tmp == line.*.beg or ctx.*.text[tmp -% 1] == ' ' or ctx.*.text[tmp -% 1] == '\t' or (ctx.*.parser.flags & 0x0002) != 0) // MD_FLAG_PERMISSIVEATXHEADERS
            line.*.end = tmp;
    }

    // Trim trailing spaces
    if (line.*.type != MD_LINE_INDENTEDCODE and line.*.type != MD_LINE_FENCEDCODE and line.*.type != MD_LINE_HTML) {
        while (line.*.end > line.*.beg and (ctx.*.text[line.*.end -% 1] == ' ' or ctx.*.text[line.*.end -% 1] == '\t'))
            line.*.end -%= 1;
    }

    // Eat also the new line
    if (off < ctx.*.size and ctx.*.text[off] == '\r')
        off +%= 1;
    if (off < ctx.*.size and ctx.*.text[off] == '\n')
        off +%= 1;

    arg_p_end.* = off;

    // If we belong to a list after seeing a blank line, the list is loose
    if (prev_line_has_list_loosening_effect != 0 and line.*.type != MD_LINE_BLANK and n_parents + n_brothers > 0) {
        const c = &containers[@as(usize, @intCast(n_parents + n_brothers - 1))];
        if (c.*.ch != '>') {
            const block: *MD_BLOCK = @ptrCast(@alignCast(@as([*c]u8, @ptrCast(@alignCast(ctx.*.block_bytes))) + @as(usize, c.*.block_byte_off)));
            const old_tfd = block.type_flags_data;
            block.type_flags_data = old_tfd | (@as(u32, MD_BLOCK_LOOSE_LIST) << 8);
        }
    }

    ret = abort: {
        // Leave any containers we are not part of anymore
        if (n_children == 0 and n_parents + n_brothers < ctx.*.n_containers) {
            ret = md_leave_child_containers(ctx, n_parents + n_brothers);
            if (ret < 0) break :abort ret;
        }

        // Enter any container we found a mark for
        if (n_brothers > 0) {
            ret = md_push_container_bytes(ctx, @as(c_uint, @bitCast(MD_BLOCK_LI)), containers[@as(usize, @intCast(n_parents))].task_mark_off, if (containers[@as(usize, @intCast(n_parents))].is_task != 0) @as(c_uint, ctx.*.text[containers[@as(usize, @intCast(n_parents))].task_mark_off]) else 0, MD_BLOCK_CONTAINER_CLOSER);
            if (ret < 0) break :abort ret;
            ret = md_push_container_bytes(ctx, @as(c_uint, @bitCast(MD_BLOCK_LI)), container.task_mark_off, if (container.is_task != 0) @as(c_uint, ctx.*.text[container.task_mark_off]) else 0, MD_BLOCK_CONTAINER_OPENER);
            if (ret < 0) break :abort ret;
            containers[@as(usize, @intCast(n_parents))].is_task = container.is_task;
            containers[@as(usize, @intCast(n_parents))].task_mark_off = container.task_mark_off;
        }

        if (n_children > 0) {
            ret = md_enter_child_containers(ctx, n_children);
            if (ret < 0) break :abort ret;
        }

        break :abort ret;
    };
    return ret;
}

pub fn md_process_line(arg_ctx: [*c]MD_CTX, arg_p_pivot_line: [*c][*c]const MD_LINE_ANALYSIS, arg_line: [*c]MD_LINE_ANALYSIS) callconv(.C) c_int {
    const ctx = arg_ctx;
    const line = arg_line;
    const pivot_line: [*c]const MD_LINE_ANALYSIS = arg_p_pivot_line[0];
    var ret: c_int = 0;

    // Blank line ends current leaf block
    if (line.*.type == MD_LINE_BLANK) {
        ret = md_end_current_block(ctx);
        if (ret < 0) return ret;
        arg_p_pivot_line[0] = &md_dummy_blank_line;
        return 0;
    }

    if (line.*.enforce_new_block != 0) {
        ret = md_end_current_block(ctx);
        if (ret < 0) return ret;
    }

    // Some line types form block on their own
    if (line.*.type == MD_LINE_HR or line.*.type == MD_LINE_ATXHEADER) {
        ret = abort: {
            ret = md_end_current_block(ctx);
            if (ret < 0) break :abort ret;
            ret = md_start_new_block(ctx, line);
            if (ret < 0) break :abort ret;
            ret = md_add_line_into_current_block(ctx, line);
            if (ret < 0) break :abort ret;
            ret = md_end_current_block(ctx);
            if (ret < 0) break :abort ret;
            break :abort ret;
        };
        if (ret == 0) arg_p_pivot_line[0] = &md_dummy_blank_line;
        return ret;
    }

    // MD_LINE_SETEXTUNDERLINE changes meaning of the current block and ends it
    if (line.*.type == MD_LINE_SETEXTUNDERLINE) {
        ret = abort: {
            ctx.*.current_block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_H))));
            ctx.*.current_block.?.setData(@truncate(line.*.data));
            ctx.*.current_block.?.type_flags_data |= (@as(u32, MD_BLOCK_SETEXT_HEADER) << 8);
            ret = md_add_line_into_current_block(ctx, line);
            if (ret < 0) break :abort ret;
            ret = md_end_current_block(ctx);
            if (ret < 0) break :abort ret;
            if (ctx.*.current_block == null) {
                arg_p_pivot_line[0] = &md_dummy_blank_line;
            } else {
                // This happens if we have consumed all the body as link ref. defs.
                line.*.type = MD_LINE_TEXT;
                arg_p_pivot_line[0] = line;
            }
            break :abort 0;
        };
        return ret;
    }

    // MD_LINE_TABLEUNDERLINE changes meaning of the current block
    if (line.*.type == MD_LINE_TABLEUNDERLINE) {
        ctx.*.current_block.?.setType(@truncate(@as(c_uint, @bitCast(MD_BLOCK_TABLE))));
        ctx.*.current_block.?.setData(@truncate(line.*.data));
        @as(*MD_LINE_ANALYSIS, @ptrCast(@constCast(pivot_line))).type = MD_LINE_TABLE;
        ret = md_add_line_into_current_block(ctx, line);
        if (ret < 0) return ret;
        return 0;
    }

    ret = abort: {
        // The current block also ends if the line has different type
        if (line.*.type != pivot_line.*.type) {
            ret = md_end_current_block(ctx);
            if (ret < 0) break :abort ret;
        }

        // The current line may start a new block
        if (ctx.*.current_block == null) {
            ret = md_start_new_block(ctx, line);
            if (ret < 0) break :abort ret;
            arg_p_pivot_line[0] = line;
        }

        // In all other cases the line is just a continuation of the current block
        ret = md_add_line_into_current_block(ctx, line);
        if (ret < 0) break :abort ret;

        break :abort ret;
    };
    return ret;
}

pub fn md_process_doc(arg_ctx: [*c]MD_CTX) callconv(.C) c_int {
    const ctx = arg_ctx;
    var pivot_line: [*c]const MD_LINE_ANALYSIS = &md_dummy_blank_line;
    var line_buf: [2]MD_LINE_ANALYSIS = .{ .{}, .{} };
    var line: [*c]MD_LINE_ANALYSIS = &line_buf[0];
    var off: MD_OFFSET = 0;
    var ret: c_int = 0;

    abort: {
        // MD_ENTER_BLOCK(MD_BLOCK_DOC, NULL)
        ret = ctx.*.parser.enter_block.?(@as(c_uint, @bitCast(MD_BLOCK_DOC)), null, ctx.*.userdata);
        if (ret != 0) {
            if (ctx.*.parser.debug_log) |log_fn| {
                log_fn("Aborted from enter_block() callback.", ctx.*.userdata);
            }
            break :abort;
        }

        while (off < ctx.*.size) {
            if (line == pivot_line)
                line = if (line == &line_buf[0]) &line_buf[1] else &line_buf[0];

            ret = md_analyze_line(ctx, off, &off, pivot_line, line);
            if (ret < 0) break :abort;
            ret = md_process_line(ctx, &pivot_line, line);
            if (ret < 0) break :abort;
        }

        _ = md_end_current_block(ctx);

        ret = md_build_ref_def_hashtable(ctx);
        if (ret < 0) break :abort;

        // Process all blocks.
        ret = md_leave_child_containers(ctx, 0);
        if (ret < 0) break :abort;
        ret = md_process_all_blocks(ctx);
        if (ret < 0) break :abort;

        // MD_LEAVE_BLOCK(MD_BLOCK_DOC, NULL)
        ret = ctx.*.parser.leave_block.?(@as(c_uint, @bitCast(MD_BLOCK_DOC)), null, ctx.*.userdata);
        if (ret != 0) {
            if (ctx.*.parser.debug_log) |log_fn| {
                log_fn("Aborted from leave_block() callback.", ctx.*.userdata);
            }
            break :abort;
        }
    }

    return ret;
}


