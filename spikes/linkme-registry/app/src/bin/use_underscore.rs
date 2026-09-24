//! The entry crates are named with `use name as _;`, which also makes rustc
//! link them without importing anything.

use spike_entries_a as _;
use spike_entries_b as _;

fn main() {
    println!("{}", spike_registry::report());
}
