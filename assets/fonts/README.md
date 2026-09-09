# Embedded UI font

`NotoSansCJKsc-UI.otf` is a glyph subset of **Noto Sans CJK SC Regular**, downloaded from the official notofonts repository:

- Source: <https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf>
- License: SIL Open Font License 1.1 (`OFL.txt`)
- Subset SHA-256: `7e2110b2897075a2f77904ecdfffd0517b74c13cb5c6d55b2b6b3ddc024df0d6`

The subset contains all non-ASCII glyphs currently used by the Rust UI and project documentation. It is embedded as the first proportional-font fallback so built-in Chinese labels render on Windows even when no Chinese language pack is installed. Installed system fonts remain fallback choices for user-entered glyphs outside the subset.

The subset was produced with fonttools `pyftsubset`, preserving layout features, cmap tables, names, and the recommended/notdef glyphs. Modified font files remain under OFL-1.1.
