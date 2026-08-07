# DFI18n - Translation Mod for Dwarf Fortress

DFI18n is a translation mod for Dwarf Fortress.

**Please note:** This project is in early development. The current release is a **beta version** that may crash the game, corrupt save files, or cause loss of your game progress!

This mod relies on data files provided by community translators to function.

## Runtime Localization Pipeline

DFI18n captures ordinary and colored text fragments, reconstructs them into visual text blocks, and then derives sentence-level translation requests. This allows wrapped, multi-line, and dynamically assembled text to be translated and redrawn over its complete source rectangle.

The runtime attempts local full-text matches first, followed by sentence-level matching and fragment fallback. Character preference summaries receive specialized handling: `likes`, `prefers to consume`, and `absolutely detests` sections are split into independently translatable prefixes and list items. Unmatched preference parts remain in English so the complete text block can still be rendered without losing content.

## Translation Diagnostics

Recognized and unmatched runtime text is written under `Dwarf Fortress/dfi18n-data/logs`:

*   `sentences.csv` contains reconstructed complete sentences.
*   `split-sentences.csv` contains sentence-level requests produced from larger visual blocks.
*   `unmatched.csv` contains text that did not match local translation data.

`df_matcher.exe` can test the same local matching and preference-specialization logic outside the game. Place it in the root of a complete `dfi18n-data` directory and run:

```text
df_matcher.exe "A complete sentence to test."
```

It prints the translated result when a match is found. Specialized preference blocks may contain both translated and original parts when only some components match.

## Optional Cloud Translation

Cloud translation is disabled by default, and the client does not contain a hard-coded service address. Configure it from the DFHack console:

```text
dfi18n cloud set translate.example.com
dfi18n cloud enable
```

The `set` command also accepts a complete HTTP or HTTPS endpoint. A bare host is normalized to `https://<host>/v1/translate` and persisted to `cloud-translation.toml`. Disable cloud requests for the current session with:

```text
dfi18n cloud disable
```

The optional self-hosted service is documented in [translation-server/README.md](translation-server/README.md). API credentials and prompts remain on the server; the game client only submits eligible text.

## Supported Versions

This mod has been tested with [Dwarf Fortress 53.08 from Steam](https://store.steampowered.com/app/975370/Dwarf_Fortress/) (on both Windows and Linux). It may also work with the Itch or Classic versions, but this is not guaranteed.

## Installation Steps

1.  Back up your Dwarf Fortress save files.
2.  Install [DFHack](https://store.steampowered.com/app/2346660/DFHack__Dwarf_Fortress_Modding_Engine/) from Steam.
3.  Subscribe to the [DFI18n](https://steamcommunity.com/sharedfiles/filedetails/?id=3613958631) mod on the Steam Workshop.
4.  Subscribe to mods providing translation data files on the Steam Workshop.
    *   **Simplified Chinese:** [DFI18n Data - Simplified Chinese](https://steamcommunity.com/sharedfiles/filedetails/?id=3635900931)
5.  If you previously installed the old Lite version of `dfint-rust-cjk`, remove `dfhooks_dfint_cjk.dll` and `libdfhooks_dfint_cjk.so` from your game directory.
6.  Launch the game normally.

**Troubleshooting:** If the translation mod fails to work, try enabling "Portable Mode" in the game settings (especially if your Steam library is not in the default location).

## Known Issues

*   The mod may impact game performance, resulting in lower FPS.
*   The DFHack overlay may sometimes render incorrectly.
*   Tabs are not rendered with the correct width.
*   Some texts remain untranslated (if not yet covered by community translations).
*   Abbreviated text (e.g., `Cow` -> `Cw`) or text ending with ellipsis (`...`) will not translate correctly.
*   Some markup-heavy screens may still fall back to their original rendering.
*   Hot-reloading translation data files may cause rendering issues.
*   Windows backtrace frames may not show accurate memory addresses.

## Acknowledgements

*   This project draws heavy inspiration from [df-steam-hook-rs](https://github.com/dfint/df-steam-hook-rs).
*   Memory search techniques are based on ideas from [dfint/search_offsets](https://github.com/dfint/search_offsets) by [insolor](https://github.com/insolor). Search patterns were generated using the IDA [FindFunc](https://github.com/FelixBer/FindFunc) plugin (thanks to [shevernitskiy](https://github.com/shevernitskiy) for suggesting this method).
*   Thanks to [DFHack](https://github.com/DFHack) for providing the Lua API, structure definitions, and scripts to load types into Ghidra which helps reverse engineering.
*   Translation data files provided by community translators:
    *   **Simplified Chinese:** The [Dwarf Fortress Chinese Wiki](https://dfzh.huijiwiki.com/) translation team (special thanks to Bilibili user [WAN1694](https://space.bilibili.com/32828123/) for translation and testing assistance!).

## License

See [LICENSE](LICENSE) for details.
