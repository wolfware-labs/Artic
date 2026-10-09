//! Test fixture: derives one XAML type, `Beta`, and nothing else.

use artic_macros::XamlType;

#[derive(Debug, XamlType)]
#[xaml(namespace = "https://fixtures.artic.test/b")]
struct Beta;
