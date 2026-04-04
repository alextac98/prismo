use std::env;
use std::path::PathBuf;

use diplomat_tool::DocsUrlGenerator;

fn main() -> std::io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() != 4 {
        eprintln!(
            "usage: {} <entry-lib.rs> <c-out-dir> <cpp-out-dir>",
            args.first()
                .map(String::as_str)
                .unwrap_or("diplomat_codegen")
        );
        std::process::exit(1);
    }

    let entry = PathBuf::from(&args[1]);
    let c_out = PathBuf::from(&args[2]);
    let cpp_out = PathBuf::from(&args[3]);
    let docs = DocsUrlGenerator::with_base_urls(None, Default::default());

    diplomat_tool::r#gen(&entry, "c", &c_out, &docs, None, true)?;
    diplomat_tool::r#gen(&entry, "cpp", &cpp_out, &docs, None, true)?;

    Ok(())
}
