//! Adds two entries to the registry from a separate crate, the way a
//! control crate adds `TypeInfo` entries through `#[derive(XamlType)]`.

use linkme::distributed_slice;
use spike_registry::{ENTRIES, Entry};

#[distributed_slice(ENTRIES)]
static BUTTON: Entry = Entry { name: "Button", origin: "entries-a" };

#[distributed_slice(ENTRIES)]
static TEXTBLOCK: Entry = Entry { name: "TextBlock", origin: "entries-a" };
