//! Character → X11 keysym mapping for clipboard-to-keystrokes typing.
//!
//! RFB KeyEvent carries the keysym directly, so the server applies its own
//! shift/layout logic — we never need to synthesize modifier keys here.

/// Returns None for control characters we can't meaningfully type.
pub fn char_to_keysym(c: char) -> Option<u32> {
    match c {
        '\n' | '\r' => Some(0xFF0D), // Return
        '\t' => Some(0xFF09),        // Tab
        c if (c as u32) < 0x20 => None,
        // Latin-1 range: keysym == codepoint
        c if (c as u32) <= 0xFF => Some(c as u32),
        // Everything else: X11 Unicode keysym rule
        c => Some(0x0100_0000 + c as u32),
    }
}
