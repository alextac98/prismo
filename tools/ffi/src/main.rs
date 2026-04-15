use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use diplomat_tool::DocsUrlGenerator;

fn main() -> std::io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    match args.len() {
        4 => generate_into_dirs(&args),
        9 => generate_into_files(&args),
        _ => {
            eprintln!(
                "usage: {} <entry-lib.rs> <c-out-dir> <cpp-out-dir>",
                args.first()
                    .map(String::as_str)
                    .unwrap_or("diplomat_codegen")
            );
            eprintln!(
                "   or: {} <entry-lib.rs> <stem> <c-decl-out> <c-header-out> <cpp-decl-out> <cpp-header-out> <c-runtime-out> <cpp-runtime-out>",
                args.first()
                    .map(String::as_str)
                    .unwrap_or("diplomat_codegen")
            );
            std::process::exit(1);
        }
    }
}

fn generate_into_dirs(args: &[String]) -> std::io::Result<()> {
    let entry = PathBuf::from(&args[1]);
    let c_out = PathBuf::from(&args[2]);
    let cpp_out = PathBuf::from(&args[3]);
    let docs = DocsUrlGenerator::with_base_urls(None, Default::default());

    diplomat_tool::r#gen(&entry, "c", &c_out, &docs, None, true)?;
    diplomat_tool::r#gen(&entry, "cpp", &cpp_out, &docs, None, true)?;

    Ok(())
}

fn generate_into_files(args: &[String]) -> std::io::Result<()> {
    let entry = PathBuf::from(&args[1]);
    let stem = &args[2];
    let c_decl = PathBuf::from(&args[3]);
    let c_header = PathBuf::from(&args[4]);
    let cpp_decl = PathBuf::from(&args[5]);
    let cpp_header = PathBuf::from(&args[6]);
    let c_runtime = PathBuf::from(&args[7]);
    let cpp_runtime = PathBuf::from(&args[8]);
    let temp_root = unique_temp_dir();
    let c_out = temp_root.join("c");
    let cpp_out = temp_root.join("cpp");

    fs::create_dir_all(&c_out)?;
    fs::create_dir_all(&cpp_out)?;

    let result = (|| {
        let docs = DocsUrlGenerator::with_base_urls(None, Default::default());
        diplomat_tool::r#gen(&entry, "c", &c_out, &docs, None, true)?;
        diplomat_tool::r#gen(&entry, "cpp", &cpp_out, &docs, None, true)?;

        write_generated_file(c_out.join(format!("{stem}.d.h")), c_decl)?;
        write_generated_file(c_out.join(format!("{stem}.h")), c_header)?;
        write_generated_file(cpp_out.join(format!("{stem}.d.hpp")), cpp_decl)?;
        write_generated_file(cpp_out.join(format!("{stem}.hpp")), cpp_header)?;
        write_generated_file(c_out.join("diplomat_runtime.h"), c_runtime)?;
        write_generated_file(cpp_out.join("diplomat_runtime.hpp"), cpp_runtime)?;

        Ok(())
    })();

    let _ = fs::remove_dir_all(&temp_root);
    result
}

fn write_generated_file(from: PathBuf, to: PathBuf) -> std::io::Result<()> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(to, fs::read(from)?)?;
    Ok(())
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    env::temp_dir().join(format!("prismo-diplomat-{}-{}", std::process::id(), nanos))
}
