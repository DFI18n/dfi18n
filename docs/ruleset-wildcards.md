# Ruleset wildcards

The rule-based translator supports two bounded, translatable wildcard replacers:

```toml
"{%word:item} Barrel" = "{%word:item}桶"
"{%any:plant} seeds bag" = "{%any:plant}种子袋"
```

- `%word` captures one non-empty whitespace-delimited word.
- `%any` captures non-empty text up to the next literal anchor, or all remaining text when terminal.
- A suffix after the colon names a capture. Names are required when a rule uses the same wildcard type more than once.
- Captures are translated through the root ruleset after the complete outer rule matches. Missing inner translations preserve the captured source text.
- Capture translation never invokes simple CSV translation, cloud translation, or AI translation.

For safety, wildcard captures do not cross line breaks, are limited to 4096 bytes, produce at most 64 candidates per rule step, and recurse through at most four capture layers. Adjacent wildcards and a non-terminal `%any` without an immediate literal boundary are rejected while loading the TOML file.

Exact rules remain preferred over wildcard rules. Among wildcard rules, a longer literal anchor is preferred; with equal anchors, `%word` is preferred over `%any`.
