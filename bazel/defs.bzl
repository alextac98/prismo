PrismoPluginInfo = provider(
    doc = "Metadata and packaged files for a Prismo plugin bundle.",
    fields = {
        "executable": "Executable file to place in the plugin directory.",
        "executable_name": "Filename to use for the packaged plugin executable.",
        "manifest": "Plugin manifest file to place in the plugin directory.",
        "plugin_dir_name": "Directory name to use inside the bundle.",
    },
)
def _prismo_plugin_impl(ctx):
    executable = ctx.executable.executable
    executable_name = executable.basename
    manifest = ctx.file.manifest

    return [
        DefaultInfo(files = depset([manifest, executable])),
        PrismoPluginInfo(
            executable = executable,
            executable_name = executable_name,
            manifest = manifest,
            plugin_dir_name = ctx.label.name,
        ),
    ]


prismo_plugin = rule(
    implementation = _prismo_plugin_impl,
    attrs = {
        "executable": attr.label(
            mandatory = True,
            allow_single_file = True,
            executable = True,
            cfg = "target",
            doc = "Executable target to package as a Prismo plugin.",
        ),
        "manifest": attr.label(
            mandatory = True,
            allow_single_file = True,
            doc = "Checked-in plugin manifest to package unchanged.",
        ),
    },
    doc = "Packages an executable into the standard Prismo plugin directory layout.",
)


def _prismo_bundle_impl(ctx):
    bundle_dir = ctx.actions.declare_directory(ctx.label.name)
    launcher = ctx.actions.declare_file(ctx.label.name + "_run")
    prismo_executable = ctx.executable.prismo
    prismo_name = prismo_executable.basename
    plugin_args = []
    inputs = [prismo_executable]

    for plugin in ctx.attr.plugins:
        info = plugin[PrismoPluginInfo]
        plugin_args.extend([
            info.plugin_dir_name,
            info.manifest.path,
            info.executable.path,
            info.executable_name,
        ])
        inputs.extend([info.manifest, info.executable])

    ctx.actions.run(
        executable = ctx.executable._assemble_bundle_tool,
        inputs = inputs,
        outputs = [bundle_dir],
        arguments = [bundle_dir.path, prismo_executable.path, prismo_name] + plugin_args,
        mnemonic = "PrismoBundle",
    )

    ctx.actions.expand_template(
        template = ctx.file._launcher_template,
        output = launcher,
        substitutions = {
            "{WORKSPACE}": ctx.workspace_name,
            "{BUNDLE}": bundle_dir.short_path,
            "{PRISMO_NAME}": prismo_name,
        },
        is_executable = True,
    )

    runfiles = ctx.runfiles(
        files = [bundle_dir],
        transitive_files = depset(ctx.files._runfiles_lib),
    )

    return [
        DefaultInfo(
            files = depset([bundle_dir]),
            executable = launcher,
            runfiles = runfiles,
        ),
    ]


prismo_bundle = rule(
    implementation = _prismo_bundle_impl,
    executable = True,
    attrs = {
        "prismo": attr.label(
            mandatory = True,
            allow_single_file = True,
            executable = True,
            cfg = "target",
            doc = "Prismo executable target to place at the root of the bundle.",
        ),
        "plugins": attr.label_list(
            providers = [PrismoPluginInfo],
            doc = "Packaged Prismo plugin targets to include in the bundle.",
        ),
        "_assemble_bundle_tool": attr.label(
            default = Label("//bazel:assemble_bundle"),
            executable = True,
            cfg = "exec",
        ),
        "_runfiles_lib": attr.label(
            default = Label("@bazel_tools//tools/bash/runfiles"),
            allow_files = True,
        ),
        "_launcher_template": attr.label(
            default = Label("//bazel:prismo-run.sh.tera"),
            allow_single_file = True,
        ),
    },
    doc = "Builds a runnable Prismo bundle containing the app and plugin directories.",
)
