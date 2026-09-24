//! C entry points for the iOS static library and the Android shared library.

extern crate spike_entries_a;
extern crate spike_entries_b;

/// Prints the registry report to stdout and returns the entry count.
#[unsafe(no_mangle)]
pub extern "C" fn spike_report() -> usize {
    println!("{}", spike_registry::report());
    spike_registry::ENTRIES.len()
}
