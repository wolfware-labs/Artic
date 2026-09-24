//! The entry crates are only listed in Cargo.toml. Nothing names them, so
//! rustc may leave them out of the link entirely.

fn main() {
    println!("{}", spike_registry::report());
}
