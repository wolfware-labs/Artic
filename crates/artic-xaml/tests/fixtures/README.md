# Parser fixture corpus

Small `.xaml` files the parser is tested against. The runner, `tests/fixtures.rs`, parses every file
and compares the result with a reviewed [insta](https://insta.rs) snapshot in `tests/snapshots/`.

## Layout

```text
valid/<construct>/<case>.xaml     must parse; the snapshot is the infoset
invalid/<construct>/<case>.xaml   must fail; the snapshot is the rendered errors
```

`<construct>` groups cases by the spec §4 row they cover: `attached`, `markup-extension`,
`namespaces`, `unsupported`, and so on. A file under `valid/` that doesn't parse, or a file under
`invalid/` that does, fails the run with its errors or with "should have errors".

`valid/` means "parses", not "resolves against the registry": a valid fixture can still name a type
no crate declares. Back-end runs over this corpus (#61) add their own marker for that when they
need one.

## Adding a fixture

1. Add `valid/<construct>/<case>.xaml` or `invalid/<construct>/<case>.xaml`. Keep it small and
   about one thing; a comment at the top can say what it shows.
2. Run `cargo insta test -p artic-xaml --test fixtures --review`, or `cargo test` followed by
   `cargo insta review`, and check the new snapshot.
3. Commit the fixture and its snapshot together.

The runner needs no change. A fixture that's removed or renamed leaves its old snapshot behind:
delete that snapshot too. Files keep LF line endings on every platform (`.gitattributes`), since
spans depend on the exact bytes.

## Reading a snapshot

A `valid/` snapshot prints one line per node or member, indented by depth, with spans as
`line:col..line:col`:

```text
Object Button [https://artic.rs/ui] 1:2..1:8      the type as written, [its namespace], its name
  Width 1:9..1:14                                 a property member, at its name
    Value "100" 1:16..1:19
  attached Grid.Row [https://artic.rs/ui] 2:3..2:11
  directive Name 3:3..3:9
  content 4:1..6:1
    Value "" raw "\n  " 4:1..5:3                   raw is shown when whitespace rules changed it
```

An `invalid/` snapshot is exactly what a user sees: the rendered diagnostics.
