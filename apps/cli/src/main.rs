//! `fuselane`: the command-line interface.
//!
//! Commands arrive in Phase 2 (`get`, `ls`, `pause`, `resume`, `rm`, `nets`).
//! For now it only reports its version, which the packaged-app smoke test uses.

use clap::Parser;

/// Fuse every connection into one fast lane.
#[derive(Debug, Parser)]
#[command(name = "fuselane", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn version_matches_the_workspace() {
        assert_eq!(
            Cli::command().get_version(),
            Some(env!("CARGO_PKG_VERSION"))
        );
    }
}
