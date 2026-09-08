// md4c-html: Markdown to HTML renderer
// Zig port of md4c-html.c from the md4c project
// (http://github.com/mity/md4c)
//
// Copyright (c) 2016-2024 Martin Mitáš (original C code)
// Zig port maintains identical behavior.

const std = @import("std");
const entity = @import("entity.zig");

// Import C types from md4c.h
const c = @cImport({
    @cInclude("md4c.h");
});

const MD_CHAR = c.MD_CHAR;
const MD_SIZE = c.MD_SIZE;
const MD_OFFSET = c.MD_OFFSET;

// Block types
const MD_BLOCK_DOC = c.MD_BLOCK_DOC;
const MD_BLOCK_QUOTE = c.MD_BLOCK_QUOTE;
const MD_BLOCK_UL = c.MD_BLOCK_UL;
const MD_BLOCK_OL = c.MD_BLOCK_OL;
const MD_BLOCK_LI = c.MD_BLOCK_LI;
const MD_BLOCK_HR = c.MD_BLOCK_HR;
const MD_BLOCK_H = c.MD_BLOCK_H;
const MD_BLOCK_CODE = c.MD_BLOCK_CODE;
const MD_BLOCK_HTML = c.MD_BLOCK_HTML;
const MD_BLOCK_P = c.MD_BLOCK_P;
const MD_BLOCK_TABLE = c.MD_BLOCK_TABLE;
const MD_BLOCK_THEAD = c.MD_BLOCK_THEAD;
const MD_BLOCK_TBODY = c.MD_BLOCK_TBODY;
const MD_BLOCK_TR = c.MD_BLOCK_TR;
const MD_BLOCK_TH = c.MD_BLOCK_TH;
const MD_BLOCK_TD = c.MD_BLOCK_TD;

// Span types
const MD_SPAN_EM = c.MD_SPAN_EM;
const MD_SPAN_STRONG = c.MD_SPAN_STRONG;
const MD_SPAN_A = c.MD_SPAN_A;
const MD_SPAN_IMG = c.MD_SPAN_IMG;
const MD_SPAN_CODE = c.MD_SPAN_CODE;
const MD_SPAN_DEL = c.MD_SPAN_DEL;
const MD_SPAN_LATEXMATH = c.MD_SPAN_LATEXMATH;
const MD_SPAN_LATEXMATH_DISPLAY = c.MD_SPAN_LATEXMATH_DISPLAY;
const MD_SPAN_WIKILINK = c.MD_SPAN_WIKILINK;
const MD_SPAN_U = c.MD_SPAN_U;

// Text types
const MD_TEXT_NORMAL = c.MD_TEXT_NORMAL;
const MD_TEXT_NULLCHAR = c.MD_TEXT_NULLCHAR;
const MD_TEXT_BR = c.MD_TEXT_BR;
const MD_TEXT_SOFTBR = c.MD_TEXT_SOFTBR;
const MD_TEXT_ENTITY = c.MD_TEXT_ENTITY;
const MD_TEXT_CODE = c.MD_TEXT_CODE;
const MD_TEXT_HTML = c.MD_TEXT_HTML;
const MD_TEXT_LATEXMATH = c.MD_TEXT_LATEXMATH;

// Alignment
const MD_ALIGN_LEFT = c.MD_ALIGN_LEFT;
const MD_ALIGN_CENTER = c.MD_ALIGN_CENTER;
const MD_ALIGN_RIGHT = c.MD_ALIGN_RIGHT;

// Renderer flags
const MD_HTML_FLAG_DEBUG: c_uint = 0x0001;
const MD_HTML_FLAG_VERBATIM_ENTITIES: c_uint = 0x0002;
const MD_HTML_FLAG_SKIP_UTF8_BOM: c_uint = 0x0004;
const MD_HTML_FLAG_XHTML: c_uint = 0x0008;

const NEED_HTML_ESC_FLAG: u8 = 0x1;
const NEED_URL_ESC_FLAG: u8 = 0x2;

const ProcessOutputFn = *const fn ([*c]const MD_CHAR, MD_SIZE, ?*anyopaque) callconv(.C) void;

const MD_HTML = struct {
    process_output: ProcessOutputFn,
    userdata: ?*anyopaque,
    flags: c_uint,
    image_nesting_level: c_int,
    escape_map: [256]u8,
};

fn renderVerbatim(r: *MD_HTML, text: [*]const u8, size: MD_SIZE) void {
    r.process_output(@ptrCast(text), size, r.userdata);
}

fn renderVerbatimStr(r: *MD_HTML, comptime text: []const u8) void {
    renderVerbatim(r, text.ptr, @intCast(text.len));
}

fn renderHtmlEscaped(r: *MD_HTML, data: [*]const u8, size: MD_SIZE) void {
    var beg: MD_OFFSET = 0;
    var off: MD_OFFSET = 0;
    const sz: u32 = size;

    while (true) {
        // Optimization: loop unrolling
        while (off + 3 < sz and
            (r.escape_map[data[off + 0]] & NEED_HTML_ESC_FLAG) == 0 and
            (r.escape_map[data[off + 1]] & NEED_HTML_ESC_FLAG) == 0 and
            (r.escape_map[data[off + 2]] & NEED_HTML_ESC_FLAG) == 0 and
            (r.escape_map[data[off + 3]] & NEED_HTML_ESC_FLAG) == 0)
        {
            off += 4;
        }
        while (off < sz and (r.escape_map[data[off]] & NEED_HTML_ESC_FLAG) == 0) {
            off += 1;
        }

        if (off > beg)
            renderVerbatim(r, data + beg, off - beg);

        if (off < sz) {
            switch (data[off]) {
                '&' => renderVerbatimStr(r, "&amp;"),
                '<' => renderVerbatimStr(r, "&lt;"),
                '>' => renderVerbatimStr(r, "&gt;"),
                '"' => renderVerbatimStr(r, "&quot;"),
                else => {},
            }
            off += 1;
        } else {
            break;
        }
        beg = off;
    }
}

fn renderUrlEscaped(r: *MD_HTML, data: [*]const u8, size: MD_SIZE) void {
    const hex_chars = "0123456789ABCDEF";
    var beg: MD_OFFSET = 0;
    var off: MD_OFFSET = 0;
    const sz: u32 = size;

    while (true) {
        while (off < sz and (r.escape_map[data[off]] & NEED_URL_ESC_FLAG) == 0) {
            off += 1;
        }
        if (off > beg)
            renderVerbatim(r, data + beg, off - beg);

        if (off < sz) {
            switch (data[off]) {
                '&' => renderVerbatimStr(r, "&amp;"),
                else => {
                    var hex: [3]u8 = undefined;
                    hex[0] = '%';
                    hex[1] = hex_chars[(@as(u32, data[off]) >> 4) & 0xf];
                    hex[2] = hex_chars[(@as(u32, data[off]) >> 0) & 0xf];
                    renderVerbatim(r, &hex, 3);
                },
            }
            off += 1;
        } else {
            break;
        }

        beg = off;
    }
}

fn hexVal(ch: u8) u32 {
    if ('0' <= ch and ch <= '9')
        return ch - '0';
    if ('A' <= ch and ch <= 'Z')
        return ch - 'A' + 10;
    return ch - 'a' + 10;
}

const AppendFn = *const fn (*MD_HTML, [*]const u8, MD_SIZE) void;

fn renderUtf8Codepoint(r: *MD_HTML, codepoint: u32, fn_append: AppendFn) void {
    const utf8_replacement_char = [3]u8{ 0xef, 0xbf, 0xbd };

    var utf8: [4]u8 = undefined;
    var n: usize = undefined;

    if (codepoint <= 0x7f) {
        n = 1;
        utf8[0] = @intCast(codepoint);
    } else if (codepoint <= 0x7ff) {
        n = 2;
        utf8[0] = @intCast(0xc0 | ((codepoint >> 6) & 0x1f));
        utf8[1] = @intCast(0x80 + ((codepoint >> 0) & 0x3f));
    } else if (codepoint <= 0xffff) {
        n = 3;
        utf8[0] = @intCast(0xe0 | ((codepoint >> 12) & 0xf));
        utf8[1] = @intCast(0x80 + ((codepoint >> 6) & 0x3f));
        utf8[2] = @intCast(0x80 + ((codepoint >> 0) & 0x3f));
    } else {
        n = 4;
        utf8[0] = @intCast(0xf0 | ((codepoint >> 18) & 0x7));
        utf8[1] = @intCast(0x80 + ((codepoint >> 12) & 0x3f));
        utf8[2] = @intCast(0x80 + ((codepoint >> 6) & 0x3f));
        utf8[3] = @intCast(0x80 + ((codepoint >> 0) & 0x3f));
    }

    if (0 < codepoint and codepoint <= 0x10ffff) {
        fn_append(r, &utf8, @intCast(n));
    } else {
        fn_append(r, &utf8_replacement_char, 3);
    }
}

fn renderEntity(r: *MD_HTML, text: [*]const u8, size: MD_SIZE, fn_append: AppendFn) void {
    if (r.flags & MD_HTML_FLAG_VERBATIM_ENTITIES != 0) {
        renderVerbatim(r, text, size);
        return;
    }

    // We assume UTF-8 output is what is desired.
    if (size > 3 and text[1] == '#') {
        var codepoint: u32 = 0;

        if (text[2] == 'x' or text[2] == 'X') {
            // Hexadecimal entity (e.g. "&#x1234abcd;")
            var i: u32 = 3;
            while (i < size - 1) : (i += 1) {
                codepoint = 16 * codepoint + hexVal(text[i]);
            }
        } else {
            // Decimal entity (e.g. "&#1234;")
            var i: u32 = 2;
            while (i < size - 1) : (i += 1) {
                codepoint = 10 * codepoint + @as(u32, text[i] - '0');
            }
        }

        renderUtf8Codepoint(r, codepoint, fn_append);
        return;
    } else {
        // Named entity (e.g. "&nbsp;")
        const ent = entity.entity_lookup(@ptrCast(text), size);
        if (ent != null) {
            const e = ent.?;
            renderUtf8Codepoint(r, e.codepoints[0], fn_append);
            if (e.codepoints[1] != 0)
                renderUtf8Codepoint(r, e.codepoints[1], fn_append);
            return;
        }
    }

    fn_append(r, text, size);
}

fn renderAttribute(r: *MD_HTML, attr: *const c.MD_ATTRIBUTE, fn_append: AppendFn) void {
    var i: usize = 0;
    while (attr.substr_offsets[i] < attr.size) : (i += 1) {
        const typ = attr.substr_types[i];
        const off = attr.substr_offsets[i];
        const size = attr.substr_offsets[i + 1] - off;
        const text_ptr: [*]const u8 = @ptrCast(attr.text);
        const text = text_ptr + off;

        switch (typ) {
            MD_TEXT_NULLCHAR => renderUtf8Codepoint(r, 0x0000, renderVerbatim),
            MD_TEXT_ENTITY => renderEntity(r, text, size, fn_append),
            else => fn_append(r, text, size),
        }
    }
}

fn renderOpenOlBlock(r: *MD_HTML, det: *const c.MD_BLOCK_OL_DETAIL) void {
    if (det.start == 1) {
        renderVerbatimStr(r, "<ol>\n");
        return;
    }

    var buf: [64]u8 = undefined;
    const written = std.fmt.bufPrint(&buf, "<ol start=\"{d}\">\n", .{det.start}) catch {
        renderVerbatimStr(r, "<ol>\n");
        return;
    };
    renderVerbatim(r, written.ptr, @intCast(written.len));
}

fn renderOpenLiBlock(r: *MD_HTML, det: *const c.MD_BLOCK_LI_DETAIL) void {
    if (det.is_task != 0) {
        renderVerbatimStr(r, "<li class=\"task-list-item\">" ++
            "<input type=\"checkbox\" class=\"task-list-item-checkbox\" disabled");
        if (det.task_mark == 'x' or det.task_mark == 'X')
            renderVerbatimStr(r, " checked");
        renderVerbatimStr(r, ">");
    } else {
        renderVerbatimStr(r, "<li>");
    }
}

fn renderOpenCodeBlock(r: *MD_HTML, det: *const c.MD_BLOCK_CODE_DETAIL) void {
    renderVerbatimStr(r, "<pre><code");

    // If known, output the HTML 5 attribute class="language-LANGNAME".
    if (det.lang.text != null) {
        renderVerbatimStr(r, " class=\"language-");
        renderAttribute(r, &det.lang, renderHtmlEscaped);
        renderVerbatimStr(r, "\"");
    }

    renderVerbatimStr(r, ">");
}

fn renderOpenTdBlock(r: *MD_HTML, cell_type: []const u8, det: *const c.MD_BLOCK_TD_DETAIL) void {
    renderVerbatimStr(r, "<");
    renderVerbatim(r, cell_type.ptr, @intCast(cell_type.len));

    switch (det.@"align") {
        MD_ALIGN_LEFT => renderVerbatimStr(r, " align=\"left\">"),
        MD_ALIGN_CENTER => renderVerbatimStr(r, " align=\"center\">"),
        MD_ALIGN_RIGHT => renderVerbatimStr(r, " align=\"right\">"),
        else => renderVerbatimStr(r, ">"),
    }
}

fn renderOpenASpan(r: *MD_HTML, det: *const c.MD_SPAN_A_DETAIL) void {
    renderVerbatimStr(r, "<a href=\"");
    renderAttribute(r, &det.href, renderUrlEscaped);

    if (det.title.text != null) {
        renderVerbatimStr(r, "\" title=\"");
        renderAttribute(r, &det.title, renderHtmlEscaped);
    }

    renderVerbatimStr(r, "\">");
}

fn renderOpenImgSpan(r: *MD_HTML, det: *const c.MD_SPAN_IMG_DETAIL) void {
    renderVerbatimStr(r, "<img src=\"");
    renderAttribute(r, &det.src, renderUrlEscaped);

    renderVerbatimStr(r, "\" alt=\"");
}

fn renderCloseImgSpan(r: *MD_HTML, det: *const c.MD_SPAN_IMG_DETAIL) void {
    if (det.title.text != null) {
        renderVerbatimStr(r, "\" title=\"");
        renderAttribute(r, &det.title, renderHtmlEscaped);
    }

    if (r.flags & MD_HTML_FLAG_XHTML != 0) {
        renderVerbatimStr(r, "\" />");
    } else {
        renderVerbatimStr(r, "\">");
    }
}

fn renderOpenWikilinkSpan(r: *MD_HTML, det: *const c.struct_MD_SPAN_WIKILINK) void {
    renderVerbatimStr(r, "<x-wikilink data-target=\"");
    renderAttribute(r, &det.target, renderHtmlEscaped);

    renderVerbatimStr(r, "\">");
}

// Callback implementations
fn enterBlockCallback(block_type: c_uint, detail: ?*anyopaque, userdata: ?*anyopaque) callconv(.C) c_int {
    const head = [6][]const u8{ "<h1>", "<h2>", "<h3>", "<h4>", "<h5>", "<h6>" };
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));

    switch (block_type) {
        MD_BLOCK_DOC => {},
        MD_BLOCK_QUOTE => renderVerbatimStr(r, "<blockquote>\n"),
        MD_BLOCK_UL => renderVerbatimStr(r, "<ul>\n"),
        MD_BLOCK_OL => {
            const det: *const c.MD_BLOCK_OL_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenOlBlock(r, det);
        },
        MD_BLOCK_LI => {
            const det: *const c.MD_BLOCK_LI_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenLiBlock(r, det);
        },
        MD_BLOCK_HR => {
            if (r.flags & MD_HTML_FLAG_XHTML != 0)
                renderVerbatimStr(r, "<hr />\n")
            else
                renderVerbatimStr(r, "<hr>\n");
        },
        MD_BLOCK_H => {
            const det: *const c.MD_BLOCK_H_DETAIL = @ptrCast(@alignCast(detail));
            const h = head[det.level - 1];
            renderVerbatim(r, h.ptr, @intCast(h.len));
        },
        MD_BLOCK_CODE => {
            const det: *const c.MD_BLOCK_CODE_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenCodeBlock(r, det);
        },
        MD_BLOCK_HTML => {},
        MD_BLOCK_P => renderVerbatimStr(r, "<p>"),
        MD_BLOCK_TABLE => renderVerbatimStr(r, "<table>\n"),
        MD_BLOCK_THEAD => renderVerbatimStr(r, "<thead>\n"),
        MD_BLOCK_TBODY => renderVerbatimStr(r, "<tbody>\n"),
        MD_BLOCK_TR => renderVerbatimStr(r, "<tr>\n"),
        MD_BLOCK_TH => {
            const det: *const c.MD_BLOCK_TD_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenTdBlock(r, "th", det);
        },
        MD_BLOCK_TD => {
            const det: *const c.MD_BLOCK_TD_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenTdBlock(r, "td", det);
        },
        else => {},
    }

    return 0;
}

fn leaveBlockCallback(block_type: c_uint, detail: ?*anyopaque, userdata: ?*anyopaque) callconv(.C) c_int {
    const head = [6][]const u8{ "</h1>\n", "</h2>\n", "</h3>\n", "</h4>\n", "</h5>\n", "</h6>\n" };
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));

    switch (block_type) {
        MD_BLOCK_DOC => {},
        MD_BLOCK_QUOTE => renderVerbatimStr(r, "</blockquote>\n"),
        MD_BLOCK_UL => renderVerbatimStr(r, "</ul>\n"),
        MD_BLOCK_OL => renderVerbatimStr(r, "</ol>\n"),
        MD_BLOCK_LI => renderVerbatimStr(r, "</li>\n"),
        MD_BLOCK_HR => {},
        MD_BLOCK_H => {
            const det: *const c.MD_BLOCK_H_DETAIL = @ptrCast(@alignCast(detail));
            const h = head[det.level - 1];
            renderVerbatim(r, h.ptr, @intCast(h.len));
        },
        MD_BLOCK_CODE => renderVerbatimStr(r, "</code></pre>\n"),
        MD_BLOCK_HTML => {},
        MD_BLOCK_P => renderVerbatimStr(r, "</p>\n"),
        MD_BLOCK_TABLE => renderVerbatimStr(r, "</table>\n"),
        MD_BLOCK_THEAD => renderVerbatimStr(r, "</thead>\n"),
        MD_BLOCK_TBODY => renderVerbatimStr(r, "</tbody>\n"),
        MD_BLOCK_TR => renderVerbatimStr(r, "</tr>\n"),
        MD_BLOCK_TH => renderVerbatimStr(r, "</th>\n"),
        MD_BLOCK_TD => renderVerbatimStr(r, "</td>\n"),
        else => {},
    }

    return 0;
}

fn enterSpanCallback(span_type: c_uint, detail: ?*anyopaque, userdata: ?*anyopaque) callconv(.C) c_int {
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));
    const inside_img = (r.image_nesting_level > 0);

    if (span_type == MD_SPAN_IMG)
        r.image_nesting_level += 1;
    if (inside_img)
        return 0;

    switch (span_type) {
        MD_SPAN_EM => renderVerbatimStr(r, "<em>"),
        MD_SPAN_STRONG => renderVerbatimStr(r, "<strong>"),
        MD_SPAN_U => renderVerbatimStr(r, "<u>"),
        MD_SPAN_A => {
            const det: *const c.MD_SPAN_A_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenASpan(r, det);
        },
        MD_SPAN_IMG => {
            const det: *const c.MD_SPAN_IMG_DETAIL = @ptrCast(@alignCast(detail));
            renderOpenImgSpan(r, det);
        },
        MD_SPAN_CODE => renderVerbatimStr(r, "<code>"),
        MD_SPAN_DEL => renderVerbatimStr(r, "<del>"),
        MD_SPAN_LATEXMATH => renderVerbatimStr(r, "<x-equation>"),
        MD_SPAN_LATEXMATH_DISPLAY => renderVerbatimStr(r, "<x-equation type=\"display\">"),
        MD_SPAN_WIKILINK => {
            const det: *const c.struct_MD_SPAN_WIKILINK = @ptrCast(@alignCast(detail));
            renderOpenWikilinkSpan(r, det);
        },
        else => {},
    }

    return 0;
}

fn leaveSpanCallback(span_type: c_uint, detail: ?*anyopaque, userdata: ?*anyopaque) callconv(.C) c_int {
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));

    if (span_type == MD_SPAN_IMG)
        r.image_nesting_level -= 1;
    if (r.image_nesting_level > 0)
        return 0;

    switch (span_type) {
        MD_SPAN_EM => renderVerbatimStr(r, "</em>"),
        MD_SPAN_STRONG => renderVerbatimStr(r, "</strong>"),
        MD_SPAN_U => renderVerbatimStr(r, "</u>"),
        MD_SPAN_A => renderVerbatimStr(r, "</a>"),
        MD_SPAN_IMG => {
            const det: *const c.MD_SPAN_IMG_DETAIL = @ptrCast(@alignCast(detail));
            renderCloseImgSpan(r, det);
        },
        MD_SPAN_CODE => renderVerbatimStr(r, "</code>"),
        MD_SPAN_DEL => renderVerbatimStr(r, "</del>"),
        MD_SPAN_LATEXMATH, MD_SPAN_LATEXMATH_DISPLAY => renderVerbatimStr(r, "</x-equation>"),
        MD_SPAN_WIKILINK => renderVerbatimStr(r, "</x-wikilink>"),
        else => {},
    }

    return 0;
}

fn textCallback(text_type: c_uint, text: [*c]const MD_CHAR, size: MD_SIZE, userdata: ?*anyopaque) callconv(.C) c_int {
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));
    const text_ptr: [*]const u8 = @ptrCast(text);

    switch (text_type) {
        MD_TEXT_NULLCHAR => renderUtf8Codepoint(r, 0x0000, renderVerbatim),
        MD_TEXT_BR => {
            if (r.image_nesting_level == 0) {
                if (r.flags & MD_HTML_FLAG_XHTML != 0)
                    renderVerbatimStr(r, "<br />\n")
                else
                    renderVerbatimStr(r, "<br>\n");
            } else {
                renderVerbatimStr(r, " ");
            }
        },
        MD_TEXT_SOFTBR => {
            if (r.image_nesting_level == 0)
                renderVerbatimStr(r, "\n")
            else
                renderVerbatimStr(r, " ");
        },
        MD_TEXT_HTML => renderVerbatim(r, text_ptr, size),
        MD_TEXT_ENTITY => renderEntity(r, text_ptr, size, renderHtmlEscaped),
        else => renderHtmlEscaped(r, text_ptr, size),
    }

    return 0;
}

fn debugLogCallback(msg: [*c]const u8, userdata: ?*anyopaque) callconv(.C) void {
    const r: *MD_HTML = @ptrCast(@alignCast(userdata));
    if (r.flags & MD_HTML_FLAG_DEBUG != 0) {
        const stderr = std.io.getStdErr().writer();
        const m: [*]const u8 = @ptrCast(msg);
        // Find null terminator
        var len: usize = 0;
        while (m[len] != 0) : (len += 1) {}
        stderr.print("MD4C: {s}\n", .{m[0..len]}) catch {};
    }
}

fn isAlnum(ch: u8) bool {
    return (ch >= 'a' and ch <= 'z') or (ch >= 'A' and ch <= 'Z') or (ch >= '0' and ch <= '9');
}

fn charInStr(ch: u8, str: []const u8) bool {
    for (str) |s| {
        if (ch == s) return true;
    }
    return false;
}

// Exported C ABI function
pub export fn md_html(
    input: [*c]const MD_CHAR,
    input_size: MD_SIZE,
    process_output: ProcessOutputFn,
    userdata: ?*anyopaque,
    parser_flags: c_uint,
    renderer_flags: c_uint,
) c_int {
    var render = MD_HTML{
        .process_output = process_output,
        .userdata = userdata,
        .flags = renderer_flags,
        .image_nesting_level = 0,
        .escape_map = [_]u8{0} ** 256,
    };

    var parser = c.MD_PARSER{
        .abi_version = 0,
        .flags = parser_flags,
        .enter_block = enterBlockCallback,
        .leave_block = leaveBlockCallback,
        .enter_span = enterSpanCallback,
        .leave_span = leaveSpanCallback,
        .text = textCallback,
        .debug_log = debugLogCallback,
        .syntax = null,
    };

    // Build map of characters which need escaping.
    for (0..256) |i| {
        const ch: u8 = @intCast(i);

        if (charInStr(ch, "\"&<>"))
            render.escape_map[i] |= NEED_HTML_ESC_FLAG;

        if (!isAlnum(ch) and !charInStr(ch, "~-_.+!*(),%#@?=;:/,+$"))
            render.escape_map[i] |= NEED_URL_ESC_FLAG;
    }

    // Consider skipping UTF-8 byte order mark (BOM).
    var actual_input: [*c]const u8 = input;
    var actual_size: MD_SIZE = input_size;
    if (renderer_flags & MD_HTML_FLAG_SKIP_UTF8_BOM != 0) {
        const bom = [3]u8{ 0xef, 0xbb, 0xbf };
        if (actual_size >= 3) {
            const inp: [*]const u8 = @ptrCast(actual_input);
            if (inp[0] == bom[0] and inp[1] == bom[1] and inp[2] == bom[2]) {
                actual_input = @ptrCast(inp + 3);
                actual_size -= 3;
            }
        }
    }

    return c.md_parse(actual_input, actual_size, &parser, @ptrCast(&render));
}
