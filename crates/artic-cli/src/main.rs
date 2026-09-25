//! `cargo artic`: project tooling for artic apps.

const USAGE: &str = "\
Usage: cargo artic <command>

Commands:
    check     Report XAML diagnostics without building
    new       Create a new artic app
    schema    Generate an XSD for editor completion
    inspect   Attach to a running debug app
    ios       Generate the iOS shell project
    android   Generate the Android shell project
    lsp       Start the language server

No commands are implemented yet.
";

fn main() {
  // Cargo runs subcommands as `cargo-artic artic <args>`, so skip the
  // subcommand name when it is present.
  let mut args = std::env::args().skip(1).peekable();
  if args.peek().map(String::as_str) == Some("artic") {
    args.next();
  }
  print!("{USAGE}");
}
