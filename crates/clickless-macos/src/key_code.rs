use clickless_core::LogicalKey;

pub fn cg_to_logical(code: u16) -> Option<LogicalKey> {
    match code {
        0x39 => Some(LogicalKey::CapsLock),
        0x38 => Some(LogicalKey::ShiftLeft),
        0x3B => Some(LogicalKey::ControlLeft),
        0x04 => Some(LogicalKey::H),
        0x26 => Some(LogicalKey::J),
        0x28 => Some(LogicalKey::K),
        0x25 => Some(LogicalKey::L),
        0x20 => Some(LogicalKey::U),
        0x22 => Some(LogicalKey::I),
        0x1F => Some(LogicalKey::O),
        0x03 => Some(LogicalKey::F),
        0x02 => Some(LogicalKey::D),
        0x0D => Some(LogicalKey::W),
        0x01 => Some(LogicalKey::S),
        0x2E => Some(LogicalKey::M),
        0x2B => Some(LogicalKey::Comma),
        0x2F => Some(LogicalKey::Dot),
        0x31 => Some(LogicalKey::Space),
        0x35 => Some(LogicalKey::Esc),
        0x00 => Some(LogicalKey::A),
        0x0B => Some(LogicalKey::B),
        0x08 => Some(LogicalKey::C),
        0x0E => Some(LogicalKey::E),
        0x05 => Some(LogicalKey::G),
        0x2D => Some(LogicalKey::N),
        0x23 => Some(LogicalKey::P),
        0x0C => Some(LogicalKey::Q),
        0x0F => Some(LogicalKey::R),
        0x11 => Some(LogicalKey::T),
        0x09 => Some(LogicalKey::V),
        0x07 => Some(LogicalKey::X),
        0x10 => Some(LogicalKey::Y),
        0x06 => Some(LogicalKey::Z),
        0x29 => Some(LogicalKey::Semicolon),
        0x2C => Some(LogicalKey::Slash),
        0x33 => Some(LogicalKey::Backspace),
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
        assert_eq!(cg_to_logical(0x00), Some(LogicalKey::A)); // kVK_ANSI_A
        assert_eq!(cg_to_logical(0x30), None); // kVK_Tab
    }

    #[test]
    fn t08_left_shift_and_control_translate() {
        assert_eq!(cg_to_logical(0x38), Some(LogicalKey::ShiftLeft));
        assert_eq!(cg_to_logical(0x3B), Some(LogicalKey::ControlLeft));
        assert_eq!(cg_to_logical(0x3A), None); // kVK_Option
        assert_eq!(cg_to_logical(0x37), None); // kVK_Command
    }
}
