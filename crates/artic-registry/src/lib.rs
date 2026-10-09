//! Type registry that maps XAML names to artic types and properties.
//!
//! Every crate that defines XAML types adds a [`TypeInfo`] to the [`XAML_TYPES`] slice, and
//! [`Registry::from_link_time`] indexes all of them for lookup. There is no registration call: the
//! linker gathers the entries.

mod descriptors;
mod registry;

pub use descriptors::{ArgInfo, AttachedPropertyInfo, EventInfo, MarkupExtensionInfo, PropertyInfo, TypeInfo};
pub use registry::{Duplicate, Registry, RegistryError};

#[doc(hidden)]
pub use linkme;
use linkme::distributed_slice;

/// Every [`TypeInfo`] registered by the crates linked into the binary.
///
/// The `XamlType` derive adds one entry per type. An entry can also be added by hand; outside this
/// crate, point the attribute at the `linkme` this crate re-exports so the defining crate doesn't
/// need its own `linkme` dependency:
///
/// ```
/// use artic_core::Object;
/// use artic_core::tree::{NodeId, TreeArena};
/// use artic_registry::{Registry, TypeInfo};
///
/// #[derive(Debug)]
/// struct Gauge;
/// impl Object for Gauge {}
///
/// static GAUGE: TypeInfo = TypeInfo {
///   name: "Gauge",
///   namespace: "using:my_app::views",
///   rust_path: "my_app::views::Gauge",
///   new: |arena: &mut TreeArena| -> NodeId { arena.alloc(Box::new(Gauge)) },
///   content: None,
///   is_collection: false,
///   properties: &[],
///   events: &[],
///   attached_properties: &[],
///   markup_extension: None,
/// };
///
/// #[artic_registry::linkme::distributed_slice(artic_registry::XAML_TYPES)]
/// #[linkme(crate = artic_registry::linkme)]
/// static GAUGE_ENTRY: &TypeInfo = &GAUGE;
///
/// let registry = Registry::from_link_time().expect("no duplicates");
/// assert!(registry.type_by_rust_path("my_app::views::Gauge").is_some());
/// ```
///
/// # Linking rules
///
/// The linker only gathers entries from crates that are part of the link, and two cases leave a
/// crate out without any error:
///
/// 1. **A crate that nothing references isn't linked.** rustc drops a dependency the code never
///    names, before the linker runs, so its entries disappear even though it is in `Cargo.toml`.
///    Naming it once keeps it: `use my_widgets as _;`.
/// 2. **An iOS static library needs `-force_load`.** `ld` only loads the archive members that
///    resolve a symbol it needs, and nothing refers to the members holding entries. Pass
///    `-force_load` with the archive, and pass the archive only once.
///
/// Entries survive fat and thin LTO, `codegen-units = 1`, `--gc-sections` and `-dead_strip`, and an
/// Android `cdylib` needs nothing extra.
#[distributed_slice]
pub static XAML_TYPES: [&'static TypeInfo];
