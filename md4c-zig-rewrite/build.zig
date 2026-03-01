const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    // Build the Zig entity module as a static library with C ABI exports
    const entity_mod = b.addModule("entity", .{
        .root_source_file = b.path("src/entity.zig"),
    });

    // Build the Zig md4c-html module as a static library with C ABI exports
    const md4c_html_lib = b.addStaticLibrary(.{
        .name = "md4c-html-zig",
        .root_source_file = b.path("src/md4c_html.zig"),
        .target = target,
        .optimize = optimize,
    });
    md4c_html_lib.addIncludePath(b.path("csrc"));
    md4c_html_lib.root_module.addImport("entity", entity_mod);
    md4c_html_lib.linkLibC();

    // Build md4c core parser as a Zig static library
    const md4c_lib = b.addStaticLibrary(.{
        .name = "md4c",
        .root_source_file = b.path("src/md4c.zig"),
        .target = target,
        .optimize = optimize,
    });
    md4c_lib.addIncludePath(b.path("csrc"));
    md4c_lib.linkLibC();

    // Build md2html executable from Zig source
    const md2html = b.addExecutable(.{
        .name = "md2html",
        .root_source_file = b.path("src/main.zig"),
        .target = target,
        .optimize = optimize,
    });
    md2html.addIncludePath(b.path("csrc"));
    md2html.linkLibrary(md4c_lib);
    md2html.linkLibrary(md4c_html_lib);
    md2html.linkLibC();

    b.installArtifact(md2html);

    const run_cmd = b.addRunArtifact(md2html);
    run_cmd.step.dependOn(b.getInstallStep());
    if (b.args) |args| {
        run_cmd.addArgs(args);
    }

    const run_step = b.step("run", "Run md2html");
    run_step.dependOn(&run_cmd.step);
}
