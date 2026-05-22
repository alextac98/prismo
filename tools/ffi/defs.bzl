def _prismo_diplomat_bindings_impl(ctx):
    stem = ctx.attr.stem
    c_decl = ctx.actions.declare_file(stem + ".d.h")
    c_header = ctx.actions.declare_file(stem + ".h")
    cpp_decl = ctx.actions.declare_file(stem + ".d.hpp")
    cpp_header = ctx.actions.declare_file(stem + ".hpp")
    c_runtime = ctx.actions.declare_file("diplomat_runtime.h")
    cpp_runtime = ctx.actions.declare_file("diplomat_runtime.hpp")
    outputs = [c_decl, c_header, cpp_decl, cpp_header, c_runtime, cpp_runtime]

    ctx.actions.run(
        executable = ctx.executable._codegen_tool,
        inputs = [ctx.file.entry],
        outputs = outputs,
        arguments = [
            ctx.file.entry.path,
            stem,
            c_decl.path,
            c_header.path,
            cpp_decl.path,
            cpp_header.path,
            c_runtime.path,
            cpp_runtime.path,
        ],
        mnemonic = "PrismoDiplomatBindings",
        progress_message = "Generating Diplomat bindings for %s" % ctx.label,
    )

    return [DefaultInfo(files = depset(outputs))]

prismo_diplomat_bindings = rule(
    implementation = _prismo_diplomat_bindings_impl,
    attrs = {
        "entry": attr.label(
            mandatory = True,
            allow_single_file = True,
            doc = "Rust entry source containing the Diplomat bridge.",
        ),
        "stem": attr.string(
            mandatory = True,
            doc = "Base filename of the generated binding headers.",
        ),
        "_codegen_tool": attr.label(
            default = Label("//tools/ffi:diplomat_codegen_tool"),
            executable = True,
            cfg = "exec",
        ),
    },
    doc = "Generates the concrete Diplomat C and C++ binding headers for a Rust bridge file.",
)
