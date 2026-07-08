//! Friendly key-name → X11 keysym mapping for the Agent Bridge `key_combo`
//! tool. Agents send names like `["ctrl","alt","delete"]`; we resolve each to
//! a keysym and press/release them as a chord.

/// Resolve a single key name (case-insensitive) or a one-character literal to
/// an X11 keysym. Returns None for unknown names.
pub fn name_to_keysym(name: &str) -> Option<u32> {
    let n = name.trim().to_ascii_lowercase();
    let sym = match n.as_str() {
        "ctrl" | "control" | "control_l" => 0xFFE3,
        "control_r" | "rctrl" => 0xFFE4,
        "shift" | "shift_l" => 0xFFE1,
        "shift_r" | "rshift" => 0xFFE2,
        "alt" | "alt_l" | "option" => 0xFFE9,
        "alt_r" | "altgr" => 0xFFEA,
        "super" | "win" | "cmd" | "meta" | "super_l" => 0xFFEB,
        "super_r" => 0xFFEC,
        "enter" | "return" => 0xFF0D,
        "tab" => 0xFF09,
        "esc" | "escape" => 0xFF1B,
        "backspace" | "bksp" => 0xFF08,
        "del" | "delete" => 0xFFFF,
        "insert" | "ins" => 0xFF63,
        "space" => 0x0020,
        "home" => 0xFF50,
        "end" => 0xFF57,
        "pageup" | "pgup" => 0xFF55,
        "pagedown" | "pgdn" => 0xFF56,
        "up" => 0xFF52,
        "down" => 0xFF54,
        "left" => 0xFF51,
        "right" => 0xFF53,
        // function keys F1..F12 -> 0xFFBE..
        _ if n.starts_with('f')
            && n[1..]
                .parse::<u32>()
                .map(|k| (1..=12).contains(&k))
                .unwrap_or(false) =>
        {
            0xFFBD + n[1..].parse::<u32>().unwrap()
        }
        // single printable character: Latin-1 keysym == codepoint
        _ if n.chars().count() == 1 => {
            let c = name.chars().next().unwrap();
            return crate::rfb::char_to_keysym(c);
        }
        _ => return None,
    };
    Some(sym)
}
