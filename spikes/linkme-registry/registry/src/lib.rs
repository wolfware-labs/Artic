//! Defines the distributed slice that the other spike crates add to.

use linkme::distributed_slice;

/// Stand-in for a registry descriptor such as `TypeInfo`.
#[derive(Debug)]
pub struct Entry {
    pub name: &'static str,
    pub origin: &'static str,
}

#[distributed_slice]
pub static ENTRIES: [Entry];

#[distributed_slice(ENTRIES)]
static ROOT: Entry = Entry { name: "Root", origin: "registry" };

/// Number of entries every configuration should see.
pub const EXPECTED: usize = 5;

/// One line in the format the run scripts parse: `entries=<n> names=<a,b,...>`.
pub fn report() -> String {
    let mut names: Vec<String> = ENTRIES.iter().map(|e| format!("{}::{}", e.origin, e.name)).collect();
    names.sort();
    format!("entries={} names={}", ENTRIES.len(), names.join(","))
}
