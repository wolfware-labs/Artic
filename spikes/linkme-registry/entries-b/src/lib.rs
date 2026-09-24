//! Adds two entries to the registry from a separate crate, the way a
//! control crate adds `TypeInfo` entries through `#[derive(XamlType)]`.

use linkme::distributed_slice;
use spike_registry::{ENTRIES, Entry};

#[distributed_slice(ENTRIES)]
static GRID: Entry = Entry { name: "Grid", origin: "entries-b" };

#[distributed_slice(ENTRIES)]
static STACKPANEL: Entry = Entry { name: "StackPanel", origin: "entries-b" };
