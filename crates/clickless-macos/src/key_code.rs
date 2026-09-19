use clickless_core::LogicalKey;

pub fn cg_to_logical(code: u16) -> Option<LogicalKey> {
    match code {
        0x39 => Some(LogicalKey::CapsLock),
        0x04 => Some(LogicalKey::H),
        0x26 => Some(LogicalKey::J),
        0x28 => Some(LogicalKey::K),
        0x25 => Some(LogicalKey::L),
        0x20 => Some(LogicalKey::U),
        0x1F => Some(LogicalKey::O),
        0x03 => Some(LogicalKey::F),
        0x02 => Some(LogicalKey::D),
        0x0D => Some(LogicalKey::W),
        0x01 => Some(LogicalKey::S),
        0x35 => Some(LogicalKey::Esc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::LogicalKey;

    #[test]
    fn t01_capslock_translates() {
        assert_eq!(cg_to_logical(0x39), Some(LogicalKey::CapsLock));
    }

    #[test]
    fn t02_hjkl_translates() {
        assert_eq!(cg_to_logical(0x04), Some(LogicalKey::H));
        assert_eq!(cg_to_logical(0x26), Some(LogicalKey::J));
        assert_eq!(cg_to_logical(0x28), Some(LogicalKey::K));
        assert_eq!(cg_to_logical(0x25), Some(LogicalKey::L));
    }

    #[test]
    fn t03_uo_translates() {
        assert_eq!(cg_to_logical(0x20), Some(LogicalKey::U));
        assert_eq!(cg_to_logical(0x1F), Some(LogicalKey::O));
    }

    #[test]
    fn t04_fd_translates() {
        assert_eq!(cg_to_logical(0x03), Some(LogicalKey::F));
        assert_eq!(cg_to_logical(0x02), Some(LogicalKey::D));
    }

    #[test]
    fn t05_ws_translates() {
        assert_eq!(cg_to_logical(0x0D), Some(LogicalKey::W));
        assert_eq!(cg_to_logical(0x01), Some(LogicalKey::S));
    }

    #[test]
    fn t06_esc_translates() {
        assert_eq!(cg_to_logical(0x35), Some(LogicalKey::Esc));
    }

    #[test]
    fn t07_unmapped_returns_none() {
        assert_eq!(cg_to_logical(0x24), None); // kVK_Return
        assert_eq!(cg_to_logical(0x31), None); // kVK_Space
        assert_eq!(cg_to_logical(0x00), None); // kVK_ANSI_A
        assert_eq!(cg_to_logical(0x30), None); // kVK_Tab
    }

    #[test]
    fn t08_modifier_keys_are_unmapped() {
        assert_eq!(cg_to_logical(0x38), None); // kVK_Shift
        assert_eq!(cg_to_logical(0x3B), None); // kVK_Control
        assert_eq!(cg_to_logical(0x3A), None); // kVK_Option
        assert_eq!(cg_to_logical(0x37), None); // kVK_Command
    }
}
