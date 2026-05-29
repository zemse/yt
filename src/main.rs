//! `yt` — a CLI for YouTube transcripts, downloads, and metadata.
//!
//! This is an early v0.0.1 scaffold and is not yet functional. See PLAN.md
//! in the repository for the implementation roadmap.

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let arg = std::env::args().nth(1);
    match arg.as_deref() {
        Some("--version") | Some("-V") => println!("yt {VERSION}"),
        _ => {
            println!("yt {VERSION} — YouTube CLI (early scaffold, not yet functional)");
            println!("Roadmap: https://github.com/zemse/yt/blob/main/PLAN.md");
        }
    }
}
