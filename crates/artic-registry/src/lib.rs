//! Type registry that maps XAML names to artic types and properties.

mod descriptors;
mod registry;

pub use descriptors::{ArgInfo, AttachedPropertyInfo, EventInfo, MarkupExtensionInfo, PropertyInfo, TypeInfo};
pub use registry::{Duplicate, Registry, RegistryError};
