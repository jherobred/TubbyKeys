//! Physical key layout. Maps set-1 scan codes to a row and a horizontal
//! position on a standard ANSI keyboard so the engine can pick row sounds and
//! pan each key to where it sits under your hands.

/// Sound class of a key. Packs can ship dedicated samples for these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyClass {
    Generic,
    Space,
    Enter,
    Backspace,
}

/// Where a key sits. `row` 0 is the number/function row, 4 the space bar row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyPos {
    pub class: KeyClass,
    pub row: u8,
    /// Horizontal centre across the whole board, 0.0 (far left) to 1.0 (far
    /// right). Drives the visualizer's mini keyboard.
    pub x: f32,
    /// Stereo position, -1.0 (left) to 1.0 (right). Centred where the hands
    /// split rather than on the whole board, which would put nearly every
    /// letter and the space bar on the left.
    pub pan: f32,
}

/// Width of the main block plus navigation cluster and numpad, in key units.
const BOARD_WIDTH: f32 = 22.5;
/// Between G and H, where the left and right hands meet, in key units.
const HAND_SPLIT: f32 = 6.75;

/// Scan code with the extended (E0) prefix folded into bit 8.
pub fn scan_id(scan_code: u32, extended: bool) -> u16 {
    ((scan_code & 0xFF) as u16) | if extended { 0x100 } else { 0 }
}

/// Resolve a key. Unknown keys still make a sound, centred on the main block.
pub fn lookup(id: u16) -> KeyPos {
    let (row, units, class) = match id {
        // Function row sits with the number row: both use the top-row sound.
        0x001 => (0, 0.5, KeyClass::Generic), // Esc
        0x03B..=0x044 => (0, 2.5 + (id - 0x03B) as f32 * 1.05, KeyClass::Generic), // F1-F10
        0x057 => (0, 13.0, KeyClass::Generic), // F11
        0x058 => (0, 14.0, KeyClass::Generic), // F12
        0x029 => (0, 0.5, KeyClass::Generic), // `
        0x002..=0x00D => (0, 1.5 + (id - 0x002) as f32, KeyClass::Generic), // 1 - =
        0x00E => (0, 14.0, KeyClass::Backspace),
        0x00F => (1, 0.75, KeyClass::Generic), // Tab
        0x010..=0x01B => (1, 2.0 + (id - 0x010) as f32, KeyClass::Generic), // Q - ]
        0x02B => (1, 14.25, KeyClass::Generic), // \
        0x03A => (2, 0.9, KeyClass::Generic),  // Caps Lock
        0x01E..=0x028 => (2, 2.25 + (id - 0x01E) as f32, KeyClass::Generic), // A - '
        0x01C => (2, 13.9, KeyClass::Enter),
        0x02A => (3, 1.1, KeyClass::Generic), // Left Shift
        0x02C..=0x035 => (3, 2.75 + (id - 0x02C) as f32, KeyClass::Generic), // Z - /
        0x036 => (3, 13.6, KeyClass::Generic), // Right Shift
        0x01D => (4, 0.6, KeyClass::Generic), // Left Ctrl
        0x15B => (4, 1.9, KeyClass::Generic), // Left Win
        0x038 => (4, 3.1, KeyClass::Generic), // Left Alt
        0x039 => (4, 7.0, KeyClass::Space),
        0x138 => (4, 10.9, KeyClass::Generic), // Right Alt
        0x15C => (4, 12.1, KeyClass::Generic), // Right Win
        0x15D => (4, 13.3, KeyClass::Generic), // Menu
        0x11D => (4, 14.4, KeyClass::Generic), // Right Ctrl
        // Navigation cluster.
        0x137 => (0, 15.5, KeyClass::Generic), // Print Screen
        0x046 => (0, 16.5, KeyClass::Generic), // Scroll Lock
        0x045 | 0x145 => (0, 17.5, KeyClass::Generic), // Pause / Num Lock
        0x152 => (1, 15.5, KeyClass::Generic), // Insert
        0x147 => (1, 16.5, KeyClass::Generic), // Home
        0x149 => (1, 17.5, KeyClass::Generic), // Page Up
        0x153 => (2, 15.5, KeyClass::Generic), // Delete
        0x14F => (2, 16.5, KeyClass::Generic), // End
        0x151 => (2, 17.5, KeyClass::Generic), // Page Down
        0x148 => (3, 16.5, KeyClass::Generic), // Up
        0x14B => (4, 15.5, KeyClass::Generic), // Left
        0x150 => (4, 16.5, KeyClass::Generic), // Down
        0x14D => (4, 17.5, KeyClass::Generic), // Right
        // Numpad.
        0x135 => (0, 19.5, KeyClass::Generic), // /
        0x037 => (0, 20.5, KeyClass::Generic), // *
        0x04A => (0, 21.5, KeyClass::Generic), // -
        0x047..=0x049 => (1, 18.5 + (id - 0x047) as f32, KeyClass::Generic), // 7 8 9
        0x04E => (1, 21.5, KeyClass::Generic), // +
        0x04B..=0x04D => (2, 18.5 + (id - 0x04B) as f32, KeyClass::Generic), // 4 5 6
        0x04F..=0x051 => (3, 18.5 + (id - 0x04F) as f32, KeyClass::Generic), // 1 2 3
        0x11C => (3, 21.5, KeyClass::Enter),   // Numpad Enter
        0x052 => (4, 19.0, KeyClass::Generic), // 0
        0x053 => (4, 20.5, KeyClass::Generic), // .
        _ => (2, 7.5, KeyClass::Generic),
    };
    KeyPos {
        class,
        row,
        x: (units / BOARD_WIDTH).clamp(0.0, 1.0),
        pan: ((units - HAND_SPLIT) / HAND_SPLIT).clamp(-1.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_keys_have_their_own_class() {
        assert_eq!(lookup(0x039).class, KeyClass::Space);
        assert_eq!(lookup(0x01C).class, KeyClass::Enter);
        assert_eq!(lookup(0x11C).class, KeyClass::Enter);
        assert_eq!(lookup(0x00E).class, KeyClass::Backspace);
        assert_eq!(lookup(0x01E).class, KeyClass::Generic);
    }

    #[test]
    fn rows_follow_the_physical_layout() {
        assert_eq!(lookup(0x002).row, 0); // 1
        assert_eq!(lookup(0x010).row, 1); // Q
        assert_eq!(lookup(0x01E).row, 2); // A
        assert_eq!(lookup(0x02C).row, 3); // Z
        assert_eq!(lookup(0x039).row, 4); // Space
    }

    #[test]
    fn left_keys_sit_left_of_right_keys() {
        let q = lookup(0x010).x;
        let p = lookup(0x019).x;
        let numpad_plus = lookup(0x04E).x;
        assert!(q < p && p < numpad_plus);
        assert!((0.0..=1.0).contains(&numpad_plus));
    }

    #[test]
    fn everyday_typing_sounds_balanced() {
        // Letters plus the space bar, weighted equally, average near the middle.
        let keys: Vec<u16> = (0x10..=0x19)
            .chain(0x1E..=0x26)
            .chain(0x2C..=0x32)
            .chain([0x39])
            .collect();
        let mean = keys.iter().map(|&k| lookup(k).pan).sum::<f32>() / keys.len() as f32;
        assert!(mean.abs() < 0.1, "typing leans {mean}");
        // Home-row pairs mirror each other: F/J, D/K, S/L.
        for (left, right) in [(0x21, 0x24), (0x20, 0x25), (0x1F, 0x26)] {
            let (l, r) = (lookup(left).pan, lookup(right).pan);
            assert!(l < 0.0 && r > 0.0 && (l + r).abs() < 0.01, "{l} vs {r}");
        }
        assert!(lookup(0x039).pan.abs() < 0.05); // space bar sits in the middle
    }

    #[test]
    fn extended_bit_distinguishes_keys() {
        assert_eq!(scan_id(0x1C, true), 0x11C);
        assert_eq!(scan_id(0x1C, false), 0x01C);
        assert_ne!(lookup(0x11D).x, lookup(0x01D).x); // right vs left Ctrl
    }
}
