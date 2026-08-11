const std = @import("std");
const c = @cImport({
    @cInclude("md4c.h");
    @cInclude("md4c-html.h");
});

const MD_SIZE = c.MD_SIZE;
const MD_CHAR = c.MD_CHAR;

// Parser flags
const MD_FLAG_COLLAPSEWHITESPACE = 0x0001;
const MD_FLAG_PERMISSIVEATXHEADERS = 0x0002;
const MD_FLAG_PERMISSIVEURLAUTOLINKS = 0x0004;
const MD_FLAG_PERMISSIVEEMAILAUTOLINKS = 0x0008;
const MD_FLAG_NOINDENTEDCODEBLOCKS = 0x0010;
const MD_FLAG_NOHTMLBLOCKS = 0x0020;
const MD_FLAG_NOHTMLSPANS = 0x0040;
const MD_FLAG_TABLES = 0x0100;
const MD_FLAG_STRIKETHROUGH = 0x0200;
const MD_FLAG_PERMISSIVEWWWAUTOLINKS = 0x0400;
const MD_FLAG_TASKLISTS = 0x0800;
const MD_FLAG_LATEXMATHSPANS = 0x1000;
const MD_FLAG_WIKILINKS = 0x2000;
const MD_FLAG_UNDERLINE = 0x4000;
const MD_FLAG_HARD_SOFT_BREAKS = 0x8000;

const MD_FLAG_PERMISSIVEAUTOLINKS = MD_FLAG_PERMISSIVEEMAILAUTOLINKS | MD_FLAG_PERMISSIVEURLAUTOLINKS | MD_FLAG_PERMISSIVEWWWAUTOLINKS;
const MD_FLAG_NOHTML = MD_FLAG_NOHTMLBLOCKS | MD_FLAG_NOHTMLSPANS;

// Renderer flags
const MD_HTML_FLAG_DEBUG = 0x0001;
const MD_HTML_FLAG_VERBATIM_ENTITIES = 0x0002;
const MD_HTML_FLAG_SKIP_UTF8_BOM = 0x0004;
const MD_HTML_FLAG_XHTML = 0x0008;

// Dialect presets
const MD_DIALECT_GITHUB = MD_FLAG_PERMISSIVEAUTOLINKS | MD_FLAG_TABLES | MD_FLAG_STRIKETHROUGH | MD_FLAG_TASKLISTS;

const OutputBuffer = struct {
    data: []u8,
    size: usize,
    capacity: usize,
    allocator: std.mem.Allocator,

    fn init(allocator: std.mem.Allocator, initial_capacity: usize) !OutputBuffer {
        const data = try allocator.alloc(u8, initial_capacity);
        return OutputBuffer{
            .data = data,
            .size = 0,
            .capacity = initial_capacity,
            .allocator = allocator,
        };
    }

    fn deinit(self: *OutputBuffer) void {
        self.allocator.free(self.data);
    }

    fn append(self: *OutputBuffer, new_data: [*c]const u8, len: usize) !void {
        if (self.size + len > self.capacity) {
            const new_cap = self.size + self.size / 2 + len;
            const new_buf = try self.allocator.alloc(u8, new_cap);
            @memcpy(new_buf[0..self.size], self.data[0..self.size]);
            self.allocator.free(self.data);
            self.data = new_buf;
            self.capacity = new_cap;
        }
        const src: [*]const u8 = @ptrCast(new_data);
        @memcpy(self.data[self.size .. self.size + len], src[0..len]);
        self.size += len;
    }
};

fn processOutput(text: [*c]const MD_CHAR, size: MD_SIZE, userdata: ?*anyopaque) callconv(.C) void {
    const buf: *OutputBuffer = @ptrCast(@alignCast(userdata));
    buf.append(text, size) catch {
        // Cannot propagate error from C callback; buffer append failed
    };
}

const Flag = struct {
    name: []const u8,
    flag: u32,
    is_parser: bool, // true = parser flag, false = renderer flag
};

const cli_flags = [_]Flag{
    .{ .name = "fcollapse-whitespace", .flag = MD_FLAG_COLLAPSEWHITESPACE, .is_parser = true },
    .{ .name = "flatex-math", .flag = MD_FLAG_LATEXMATHSPANS, .is_parser = true },
    .{ .name = "fpermissive-atx-headers", .flag = MD_FLAG_PERMISSIVEATXHEADERS, .is_parser = true },
    .{ .name = "fpermissive-autolinks", .flag = MD_FLAG_PERMISSIVEAUTOLINKS, .is_parser = true },
    .{ .name = "fhard-soft-breaks", .flag = MD_FLAG_HARD_SOFT_BREAKS, .is_parser = true },
    .{ .name = "fpermissive-email-autolinks", .flag = MD_FLAG_PERMISSIVEEMAILAUTOLINKS, .is_parser = true },
    .{ .name = "fpermissive-url-autolinks", .flag = MD_FLAG_PERMISSIVEURLAUTOLINKS, .is_parser = true },
    .{ .name = "fpermissive-www-autolinks", .flag = MD_FLAG_PERMISSIVEWWWAUTOLINKS, .is_parser = true },
    .{ .name = "fstrikethrough", .flag = MD_FLAG_STRIKETHROUGH, .is_parser = true },
    .{ .name = "ftables", .flag = MD_FLAG_TABLES, .is_parser = true },
    .{ .name = "ftasklists", .flag = MD_FLAG_TASKLISTS, .is_parser = true },
    .{ .name = "funderline", .flag = MD_FLAG_UNDERLINE, .is_parser = true },
    .{ .name = "fverbatim-entities", .flag = MD_HTML_FLAG_VERBATIM_ENTITIES, .is_parser = false },
    .{ .name = "fwiki-links", .flag = MD_FLAG_WIKILINKS, .is_parser = true },
    .{ .name = "fno-html-blocks", .flag = MD_FLAG_NOHTMLBLOCKS, .is_parser = true },
    .{ .name = "fno-html-spans", .flag = MD_FLAG_NOHTMLSPANS, .is_parser = true },
    .{ .name = "fno-html", .flag = MD_FLAG_NOHTML, .is_parser = true },
    .{ .name = "fno-indented-code", .flag = MD_FLAG_NOINDENTEDCODEBLOCKS, .is_parser = true },
};

pub fn main() !u8 {
    const allocator = std.heap.c_allocator;
    const stderr = std.io.getStdErr().writer();

    var parser_flags: c_uint = 0;
    var renderer_flags: c_uint = MD_HTML_FLAG_DEBUG | MD_HTML_FLAG_SKIP_UTF8_BOM;
    var want_fullhtml = false;
    var want_xhtml = false;
    var want_stat = false;
    var input_path: ?[]const u8 = null;
    var output_path: ?[]const u8 = null;
    var html_title: ?[]const u8 = null;
    var css_path: ?[]const u8 = null;

    // Parse command line arguments
    var args = std.process.args();
    _ = args.next(); // skip program name

    while (args.next()) |arg| {
        if (std.mem.eql(u8, arg, "--")) {
            // After --, everything is a non-option
            while (args.next()) |rest| {
                input_path = rest;
            }
            break;
        }

        if (arg.len > 2 and std.mem.startsWith(u8, arg, "--")) {
            const long_opt = arg[2..];

            // Check for options with = arguments
            if (std.mem.indexOf(u8, long_opt, "=")) |eq_pos| {
                const opt_name = long_opt[0..eq_pos];
                const opt_value = long_opt[eq_pos + 1 ..];
                if (std.mem.eql(u8, opt_name, "output")) {
                    output_path = opt_value;
                    continue;
                } else if (std.mem.eql(u8, opt_name, "html-title")) {
                    html_title = opt_value;
                    continue;
                } else if (std.mem.eql(u8, opt_name, "html-css")) {
                    css_path = opt_value;
                    continue;
                }
            }

            if (std.mem.eql(u8, long_opt, "output")) {
                output_path = args.next();
                continue;
            } else if (std.mem.eql(u8, long_opt, "full-html")) {
                want_fullhtml = true;
                continue;
            } else if (std.mem.eql(u8, long_opt, "xhtml")) {
                want_xhtml = true;
                renderer_flags |= MD_HTML_FLAG_XHTML;
                continue;
            } else if (std.mem.eql(u8, long_opt, "stat")) {
                want_stat = true;
                continue;
            } else if (std.mem.eql(u8, long_opt, "help")) {
                printUsage();
                return 0;
            } else if (std.mem.eql(u8, long_opt, "version")) {
                const stdout = std.io.getStdOut().writer();
                try stdout.print("0.5.2\n", .{});
                return 0;
            } else if (std.mem.eql(u8, long_opt, "commonmark")) {
                // CommonMark is default (flags = 0)
                continue;
            } else if (std.mem.eql(u8, long_opt, "github")) {
                parser_flags |= MD_DIALECT_GITHUB;
                continue;
            } else if (std.mem.eql(u8, long_opt, "html-title")) {
                html_title = args.next();
                continue;
            } else if (std.mem.eql(u8, long_opt, "html-css")) {
                css_path = args.next();
                continue;
            } else {
                // Check extension flags
                var found = false;
                for (cli_flags) |f| {
                    if (std.mem.eql(u8, long_opt, f.name)) {
                        if (f.is_parser) {
                            parser_flags |= @as(c_uint, f.flag);
                        } else {
                            renderer_flags |= @as(c_uint, f.flag);
                        }
                        found = true;
                        break;
                    }
                }
                if (!found) {
                    try stderr.print("Unknown option: --{s}\n", .{long_opt});
                    return 1;
                }
                continue;
            }
        }

        if (arg.len > 1 and arg[0] == '-' and arg[1] != '-') {
            // Short options
            var i: usize = 1;
            while (i < arg.len) : (i += 1) {
                switch (arg[i]) {
                    'o' => {
                        if (i + 1 < arg.len) {
                            output_path = arg[i + 1 ..];
                            break;
                        } else {
                            output_path = args.next();
                        }
                    },
                    'f' => want_fullhtml = true,
                    'x' => {
                        want_xhtml = true;
                        renderer_flags |= MD_HTML_FLAG_XHTML;
                    },
                    's' => want_stat = true,
                    'h' => {
                        printUsage();
                        return 0;
                    },
                    'v' => {
                        const stdout = std.io.getStdOut().writer();
                        try stdout.print("0.5.2\n", .{});
                        return 0;
                    },
                    else => {
                        try stderr.print("Unknown option: -{c}\n", .{arg[i]});
                        return 1;
                    },
                }
            }
            continue;
        }

        // Non-option argument (input file)
        if (std.mem.eql(u8, arg, "-")) {
            input_path = null; // stdin
        } else {
            input_path = arg;
        }
    }

    // Read input
    var input_data: []u8 = undefined;
    if (input_path) |path| {
        if (!std.mem.eql(u8, path, "-")) {
            const file = std.fs.cwd().openFile(path, .{}) catch {
                try stderr.print("Cannot open {s}.\n", .{path});
                return 1;
            };
            defer file.close();
            input_data = file.readToEndAlloc(allocator, std.math.maxInt(usize)) catch {
                try stderr.print("Error reading {s}.\n", .{path});
                return 1;
            };
        } else {
            input_data = std.io.getStdIn().readToEndAlloc(allocator, std.math.maxInt(usize)) catch {
                try stderr.print("Error reading stdin.\n", .{});
                return 1;
            };
        }
    } else {
        input_data = std.io.getStdIn().readToEndAlloc(allocator, std.math.maxInt(usize)) catch {
            try stderr.print("Error reading stdin.\n", .{});
            return 1;
        };
    }
    defer allocator.free(input_data);

    // Prepare output buffer
    var buf_out = try OutputBuffer.init(allocator, input_data.len + input_data.len / 8 + 64);
    defer buf_out.deinit();

    // Parse and render
    const t0 = std.time.nanoTimestamp();

    const ret = c.md_html(
        @ptrCast(input_data.ptr),
        @intCast(input_data.len),
        processOutput,
        @ptrCast(&buf_out),
        parser_flags,
        renderer_flags,
    );

    const t1 = std.time.nanoTimestamp();

    if (ret != 0) {
        try stderr.print("Parsing failed.\n", .{});
        return 1;
    }

    // Write output
    var out_file: std.fs.File = undefined;
    if (output_path) |path| {
        if (!std.mem.eql(u8, path, "-")) {
            out_file = std.fs.cwd().createFile(path, .{}) catch {
                try stderr.print("Cannot open {s}.\n", .{path});
                return 1;
            };
        } else {
            out_file = std.io.getStdOut();
        }
    } else {
        out_file = std.io.getStdOut();
    }

    const out_writer = out_file.writer();

    if (want_fullhtml) {
        if (want_xhtml) {
            try out_writer.writeAll("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            try out_writer.writeAll("<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.1//EN\" \"http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd\">\n");
            try out_writer.writeAll("<html xmlns=\"http://www.w3.org/1999/xhtml\">\n");
        } else {
            try out_writer.writeAll("<!DOCTYPE html>\n");
            try out_writer.writeAll("<html>\n");
        }
        try out_writer.writeAll("<head>\n");
        if (html_title) |title| {
            try out_writer.print("<title>{s}</title>\n", .{title});
        } else {
            try out_writer.writeAll("<title></title>\n");
        }
        if (want_xhtml) {
            try out_writer.writeAll("<meta name=\"generator\" content=\"md2html\" />\n");
            try out_writer.writeAll("<meta charset=\"UTF-8\" />\n");
        } else {
            try out_writer.writeAll("<meta name=\"generator\" content=\"md2html\">\n");
            try out_writer.writeAll("<meta charset=\"UTF-8\">\n");
        }
        if (css_path) |css| {
            if (want_xhtml) {
                try out_writer.print("<link rel=\"stylesheet\" href=\"{s}\" />\n", .{css});
            } else {
                try out_writer.print("<link rel=\"stylesheet\" href=\"{s}\">\n", .{css});
            }
        }
        try out_writer.writeAll("</head>\n");
        try out_writer.writeAll("<body>\n");
    }

    try out_writer.writeAll(buf_out.data[0..buf_out.size]);

    if (want_fullhtml) {
        try out_writer.writeAll("</body>\n");
        try out_writer.writeAll("</html>\n");
    }

    if (output_path != null) {
        out_file.close();
    }

    if (want_stat) {
        const elapsed_ns = t1 - t0;
        const elapsed_ms: f64 = @as(f64, @floatFromInt(elapsed_ns)) / 1_000_000.0;
        if (elapsed_ms < 1000.0) {
            try stderr.print("Time spent on parsing: {d:7.2} ms.\n", .{elapsed_ms});
        } else {
            try stderr.print("Time spent on parsing: {d:6.3} s.\n", .{elapsed_ms / 1000.0});
        }
    }

    return 0;
}

fn printUsage() void {
    const stdout = std.io.getStdOut().writer();
    stdout.writeAll(
        "Usage: md2html [OPTION]... [FILE]\n" ++
            "Convert input FILE (or standard input) in Markdown format to HTML.\n" ++
            "\n" ++
            "General options:\n" ++
            "  -o  --output=FILE    Output file (default is standard output)\n" ++
            "  -f, --full-html      Generate full HTML document, including header\n" ++
            "  -x, --xhtml          Generate XHTML instead of HTML\n" ++
            "  -s, --stat           Measure time of input parsing\n" ++
            "  -h, --help           Display this help and exit\n" ++
            "  -v, --version        Display version and exit\n" ++
            "\n" ++
            "Markdown dialect options:\n" ++
            "(note these are equivalent to some combinations of the flags below)\n" ++
            "      --commonmark     CommonMark (this is default)\n" ++
            "      --github         Github Flavored Markdown\n" ++
            "\n" ++
            "Markdown extension options:\n" ++
            "      --fcollapse-whitespace\n" ++
            "                       Collapse non-trivial whitespace\n" ++
            "      --flatex-math    Enable LaTeX style mathematics spans\n" ++
            "      --fpermissive-atx-headers\n" ++
            "                       Allow ATX headers without delimiting space\n" ++
            "      --fpermissive-url-autolinks\n" ++
            "                       Allow URL autolinks without '<', '>'\n" ++
            "      --fpermissive-www-autolinks\n" ++
            "                       Allow WWW autolinks without any scheme (e.g. 'www.example.com')\n" ++
            "      --fpermissive-email-autolinks  \n" ++
            "                       Allow e-mail autolinks without '<', '>' and 'mailto:'\n" ++
            "      --fpermissive-autolinks\n" ++
            "                       Same as --fpermissive-url-autolinks --fpermissive-www-autolinks\n" ++
            "                       --fpermissive-email-autolinks\n" ++
            "      --fhard-soft-breaks\n" ++
            "                       Force all soft breaks to act as hard breaks\n" ++
            "      --fstrikethrough Enable strike-through spans\n" ++
            "      --ftables        Enable tables\n" ++
            "      --ftasklists     Enable task lists\n" ++
            "      --funderline     Enable underline spans\n" ++
            "      --fwiki-links    Enable wiki links\n" ++
            "\n" ++
            "Markdown suppression options:\n" ++
            "      --fno-html-blocks\n" ++
            "                       Disable raw HTML blocks\n" ++
            "      --fno-html-spans\n" ++
            "                       Disable raw HTML spans\n" ++
            "      --fno-html       Same as --fno-html-blocks --fno-html-spans\n" ++
            "      --fno-indented-code\n" ++
            "                       Disable indented code blocks\n" ++
            "\n" ++
            "HTML generator options:\n" ++
            "      --fverbatim-entities\n" ++
            "                       Do not translate entities\n" ++
            "      --html-title=TITLE Sets the title of the document\n" ++
            "      --html-css=URL   In full HTML or XHTML mode add a css link\n" ++
            "\n",
    ) catch {};
}
