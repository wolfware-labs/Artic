//! The entry crates are named with `extern crate`, which forces rustc to link
//! them even though no item from them is used.

extern crate spike_entries_a;
extern crate spike_entries_b;

fn main() {
    println!("{}", spike_registry::report());
}
