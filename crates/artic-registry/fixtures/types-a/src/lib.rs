//! Test fixture: derives one XAML type, `Alpha`, and nothing else.

use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://fixtures.artic.test/a")]
struct Alpha;
