# Preserve Generated Names

## Goal

Keep Dwarf Fortress generated proper names in their original English spelling while continuing to translate profession and position suffixes attached to those names.

## Scope

- Preserve names matched by the `dwarf_language` ruleset.
- Keep the existing `name` grammar so strings such as `Name, Profession` and `Name, Position` remain matchable.
- Continue translating professions, positions, interface text, creature species, relationships, and fixed visual text.
- Leave `english_name` unchanged because it already returns English text unchanged.

## Design

The rules engine only matches literals and references; it has no arbitrary original-text capture token. Removing `dwarf_language` from `name.toml` would therefore prevent the whole name expression from matching and would also stop translation of attached profession and position suffixes.

Instead, preserve the `dwarf_language` namespace and its name grammar, but make every lexical mapping in these files an identity mapping:

- `dwarf_language/none.toml`
- `dwarf_language/adj.toml`
- `dwarf_language/veb.toml`

For example, `"Abal" = "埃巴尔"` becomes `"Abal" = "Abal"`. The existing `dwarfname.toml` composition remains unchanged, so generated names are still recognized without being transliterated.

Apply the same mechanical rewrite to every installed copy of the Chinese ruleset.

## Validation

- Parse every modified TOML file successfully.
- Confirm every rule in the three lexical files has identical source and destination text.
- Confirm all installed ruleset copies have identical hashes.
- Use the translation tool to verify:
  - a generated name remains English;
  - a generated name followed by a known profession keeps the name in English and translates the profession;
  - ordinary non-name rules still translate normally.

## Rollback

Restore the original three lexical files from the unmodified mod data bundle if name transliteration is desired again.
