# TheDraw Font (TDF) Format

The TheDraw font format is a compact binary container created for DOS-era ANSI art tooling. A single file may hold one *or many* fonts (a “bundle”). RetroFont parses it into Unicode while preserving color, outline semantics, and structural metadata.

## High-Level Goals

- Store pre-rendered glyph artwork (multi-line) for printable characters.
- Represent different font styles: Block (solid), Outline (stroke with fill/hole markers), Color (per-cell palette attributes).
- Allow bundling multiple fonts in one file for distribution.
- Keep the structure simple and fast to parse using offsets.

---

## File Layout (Bundle)

```text
+0      : 1 byte  = ID length (always 0x13 for legacy bundles: 19 bytes + 1 control)
+1..+18 : "TheDraw FONTS file" (18 ASCII bytes)
+19     : 0x1A (CTRL-Z DOS EOF marker)
+20..   : Repeated font records ...
End     : 0x00 (single zero byte terminator after last font)
```

If only a single font is present the same structure applies—still starts with the bundle header.

---

## Font Record Structure

| Order | Field | Size | Description |
|-------|-------|------|-------------|
| 1 | Indicator | 4 bytes | Little-endian constant `0xFF00AA55` marking a font start. |
| 2 | Name length | 1 byte | Original name length (≤ 12); stored area always 12 bytes padded with zeros. |
| 3 | Name text | 12 bytes | ASCII name, null padded. If shorter, rest is zeros. |
| 4 | Reserved | 4 bytes | Unused/magic (historically flags); usually zeros. |
| 5 | Font type | 1 byte | `0 = Outline`, `1 = Block`, `2 = Color`. Other values unsupported. |
| 6 | Spacing | 1 byte | Horizontal advance hint per glyph. |
| 7 | Glyph block length | 2 bytes | Little-endian size of the upcoming glyph block data. |
| 8 | Lookup table | 94 × 2 bytes | Offsets (u16) into glyph block for printable ASCII `!` .. `~` (space is special). `0xFFFF` = missing glyph. |
| 9 | Glyph block | variable | Concatenated glyph records referenced by the table. |

After processing a record, parsing continues at the next byte—until the bundle terminator `0x00`.

---

## Character Coverage

- The lookup table indexes the printable range `! (0x21)` through `~ (0x7E)` → 94 entries.
- Space `' '` is not directly in the table; many fonts omit it. RetroFont fabricates spacing when space is missing (using the font’s spacing field or fallback width).
- Internally RetroFont expands glyph storage to a 256-slot array for convenience, though unused slots remain `None`.

---

## Glyph Record Encoding

Each glyph referenced via the lookup offset is stored as:

```text
Byte 0  : width
Byte 1  : height
Byte 2+ : part stream ... terminated by 0x00
```

The “part stream” is a linear sequence of tagged bytes (and sometimes attribute bytes) representing visible cells or control markers.

### Control / Special Bytes

| Byte | Meaning (Context) | Mapped GlyphPart |
|------|-------------------|------------------|
| `0x00` | Terminator | (Ends glyph stream) |
| `0x0D` | Carriage return (line break) | `NewLine` |
| `0xFF` | Hard blank (non-collapsible space) | `HardBlank` |
| `b'&'` | End marker (legacy padding; often ignored) | `EndMarker` |
| `b'@'` | Outline fill marker | `FillMarker` |
| `b'O'` | Outline hole marker | `OutlineHole` |
| `b'A'..b'R'` | Outline placeholder style slots | `OutlinePlaceholder(letter)` |

All other bytes are interpreted as **character codes** (CP437) whose Unicode equivalents are looked up.

### Color Glyphs (Type = 2)

Each displayed character is stored as:

```text
[char_code][attribute_byte]
```

The second byte is a classic DOS text‑mode attribute (as used in VGA 80×25 text pages):

Attribute byte bit layout:

```text
7 6 5 4 3 2 1 0
B b b b f f f f
```

- Bits 0–3 (`f f f f`): Foreground color (0–15). Bit 3 is the “bright” (intensity) bit.
- Bits 4–6 (`b b b`): Background color (0–7). (Only 3 bits; background cannot use bright unless blink is disabled in hardware—historically.)
- Bit 7 (`B`): Blink flag (1 = blinking text). Many modern terminals repurpose this as “bright background” when blink is not supported or is disabled.

So the canonical DOS interpretation is: low nibble = foreground, high nibble = background + optional blink.

RetroFont converts these to `GlyphPart::Colored { ch, fg, bg, blink }`.

### Block Glyphs (Type = 1)

Each byte is either:

- `0xFF` → HardBlank
- Otherwise → CP437 mapped to Unicode → `GlyphPart::Char(c)`

### Outline Glyphs (Type = 0)

Outline fonts compress decorative box/stroke styles using placeholder bytes:

- `A`–`R` map to indexed stroke characters (RetroFont expands them to Unicode box drawing chars at render time).
- `@` and `O` represent fill/hole tokens (useful in edit mode visibility).
- Raw spaces `' '` are preserved.
- Other bytes pass through CP437 → Unicode.

This separation allows post-processing (style transforms, variant themes) without altering raw data.

---

## CP437 → Unicode Mapping

The original format uses IBM Code Page 437. RetroFont includes:

- `CP437_TO_UNICODE[256]` array: direct conversion.
- `UNICODE_TO_CP437` lazy `HashMap<char, u8>` for serialization (skips `'\0'` to avoid unintended mapping).
- Serialization falls back to `'?'` if a character cannot be mapped back.

---

## Spacing Behavior

The **spacing** byte gives a recommended inter-glyph horizontal advance. Use cases:

- If the space character is missing, emit `spacing` columns of blanks.
- Editors or renderers may override this with kerning or smushing (not part of TDF; FIGlet handles smushing separately).

RetroFont's renderer:

1. Checks for a defined `' '` glyph.
2. Falls back to spacing value (`>= 1`) if absent.

---

## Bundle Termination

A single zero byte (`0x00`) after the final font record signals end-of-bundle. Parsers should stop, ignoring trailing garbage if any (RetroFont errors on truncation before this point).

---

## Error Conditions (Typical)

| Condition | Cause |
|-----------|-------|
| “file too short” | Missing header or early EOF. |
| “id length mismatch” | First length byte not equal to `THE_DRAW_FONT_ID.len() + 1`. |
| “id mismatch” | Header bytes don’t match `TheDraw FONTS file`. |
| “font indicator mismatch” | 4-byte marker not `0xFF00AA55`. |
| “unsupported type” | Type byte outside 0–2. |
| “glyph … outside block” | Offset points beyond declared block size. |
| “truncated …” | Any structure ends before expected length. |

Font records partially present at EOF are rejected (defensive parsing).

---

## Example (Annotated Hex Snippet)

```text
13                                        ; id length (0x13 = 19 + 1)
54 68 65 44 72 61 77 20 46 4F 4E 54 53 20 ; "TheDraw FONTS "
66 69 6C 65                               ; "file"
1A                                        ; CTRL-Z
55 AA 00 FF                               ; indicator (little-endian 0xFF00AA55)
08                                        ; original name length
46 75 6E 74 6F 70 69 61 00 00 00 00       ; "Funtopia" + padding
00 00 00 00                               ; reserved
01                                        ; font type (Block)
03                                        ; spacing
E4 00                                     ; glyph block size (0x00E4)
... lookup table (94 * 2 bytes) ...
... glyph block ...
00                                        ; bundle terminator
```

---

## Differences vs FIGlet

| Aspect | TDF | FIGlet |
|--------|-----|--------|
| Multiple fonts per file | Yes (bundle) | No (one font per .flf) |
| Encoding | Binary offsets | Plain text lines |
| Color support | Yes (attr byte per cell) | Typically monochrome or ANSI embed |
| Outline abstraction | Placeholder + runtime mapping | Some fonts emulate outlines manually |
| Space handling | Often missing, spacing fallback | Always part of ASCII range |
| Termination | 0x00 after last font | End of file |

---

## Rendering Notes

1. **Line Breaks**: Carriage return byte (`0x0D`) denotes end of *glyph* line, independent of explicit `width`.
2. **Hard Blank**: Non-collapsible cell—treated as visible space; distinct from normal blank for merging/composition.
3. **Outline Expansion**: Placeholder letters are converted to Unicode box drawing characters via a static table (`OUTLINE_CHAR_SET_UNICODE`).
4. **Color Layer**: Foreground/background indices can map to ANSI 16-color escapes in CLI output; higher-color expansions are renderer-specific.

---

## Serialization (Writing Fonts)

When writing:

- Reconstruct lookup table: offset or `0xFFFF`.
- Pack glyphs sequentially, translating Unicode → CP437.
- Insert terminators.
- Compute block length and write header.
- For bundles, repeat font records, then final `0x00`.

---

## Practical Parsing Steps (RetroFont)

1. Verify header & control marker.
2. Loop:
   - Read indicator, basic metadata.
   - Read block size, table, slice glyph block.
   - For each table offset: decode glyph or skip.
3. Stop at `0x00`.

Error out early on any inconsistency rather than guess.

---

## Edge Cases

- **Empty Font**: All lookup entries `0xFFFF`; allowed but yields zero defined chars.
- **Oversized Name**: Truncated to 12 bytes (parser enforces length).
- **Invalid Offsets**: Offsets ≥ block size → error (prevent OOB reads).
- **Missing Terminator**: Stream ends before glyph part terminator; parser stops glyph early (best-effort) but may raise error depending on policy.

---

## Unicode Strategy

Original CP437 specifics (line drawing, Greek letters) are preserved through direct mapping. This enables modern monospaced terminals and editors to render accurate box/line geometry without pseudo-ASCII approximations.

---

## Summary

TheDraw’s TDF format balances compact binary representation with enough metadata to support:

- Multi-font distribution
- Stylized outlines
- Colored glyph cells
- Fast offset-based glyph access

RetroFont internally normalizes all glyph content to a semantic `GlyphPart` stream and Unicode characters, enabling consistent cross-format rendering and transformation.
