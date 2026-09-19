use clickless_core::LogicalKey;

pub fn evdev_to_logical(code: u16) -> Option<LogicalKey> {
    match code {
        58 => Some(LogicalKey::CapsLock),
        35 => Some(LogicalKey::H),
        36 => Some(LogicalKey::J),
        37 => Some(LogicalKey::K),
        38 => Some(LogicalKey::L),
        22 => Some(LogicalKey::U),
        23 => Some(LogicalKey::I),
        24 => Some(LogicalKey::O),
        33 => Some(LogicalKey::F),
        32 => Some(LogicalKey::D),
        17 => Some(LogicalKey::W),
        31 => Some(LogicalKey::S),
        50 => Some(LogicalKey::M),
        51 => Some(LogicalKey::Comma),
        52 => Some(LogicalKey::Dot),
        57 => Some(LogicalKey::Space),
        1 => Some(LogicalKey::Esc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::LogicalKey;

    #[test]
    fn t01_capslock_translates() {
        assert_eq!(evdev_to_logical(58), Some(LogicalKey::CapsLock));
    }

    #[test]
    fn t02_hjkl_translates() {
        assert_eq!(evdev_to_logical(35), Some(LogicalKey::H));
        assert_eq!(evdev_to_logical(36), Some(LogicalKey::J));
        assert_eq!(evdev_to_logical(37), Some(LogicalKey::K));
        assert_eq!(evdev_to_logical(38), Some(LogicalKey::L));
    }

    #[test]
    fn t03_uo_translates() {
        assert_eq!(evdev_to_logical(22), Some(LogicalKey::U));
        assert_eq!(evdev_to_logical(24), Some(LogicalKey::O));
    }

    #[test]
    fn t04_fd_translates() {
        assert_eq!(evdev_to_logical(33), Some(LogicalKey::F));
        assert_eq!(evdev_to_logical(32), Some(LogicalKey::D));
    }

    #[test]
    fn t05_ws_translates() {
        assert_eq!(evdev_to_logical(17), Some(LogicalKey::W));
        assert_eq!(evdev_to_logical(31), Some(LogicalKey::S));
    }

    #[test]
    fn t06_esc_translates() {
        assert_eq!(evdev_to_logical(1), Some(LogicalKey::Esc));
    }

    #[test]
    fn t07_unmapped_returns_none() {
        assert_eq!(evdev_to_logical(28), None); // KEY_ENTER
        assert_eq!(evdev_to_logical(30), None); // KEY_A
        assert_eq!(evdev_to_logical(15), None); // KEY_TAB
    }

    #[test]
    fn t08_modifier_keys_are_unmapped() {
        assert_eq!(evdev_to_logical(42), None); // KEY_LEFTSHIFT
        assert_eq!(evdev_to_logical(29), None); // KEY_LEFTCTRL
        assert_eq!(evdev_to_logical(56), None); // KEY_LEFTALT
        assert_eq!(evdev_to_logical(125), None); // KEY_LEFTMETA
    }
}
