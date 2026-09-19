use clickless_core::LogicalKey;

pub fn vk_to_logical(vk: u32) -> Option<LogicalKey> {
    match vk {
        0x14 => Some(LogicalKey::CapsLock),
        0x48 => Some(LogicalKey::H),
        0x4A => Some(LogicalKey::J),
        0x4B => Some(LogicalKey::K),
        0x4C => Some(LogicalKey::L),
        0x55 => Some(LogicalKey::U),
        0x49 => Some(LogicalKey::I),
        0x4F => Some(LogicalKey::O),
        0x46 => Some(LogicalKey::F),
        0x44 => Some(LogicalKey::D),
        0x57 => Some(LogicalKey::W),
        0x53 => Some(LogicalKey::S),
        0x4D => Some(LogicalKey::M),
        0xBC => Some(LogicalKey::Comma),
        0xBE => Some(LogicalKey::Dot),
        0x20 => Some(LogicalKey::Space),
        0x1B => Some(LogicalKey::Esc),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clickless_core::LogicalKey;

    #[test]
    fn t01_capslock_translates() {
        assert_eq!(vk_to_logical(0x14), Some(LogicalKey::CapsLock));
    }

    #[test]
    fn t02_hjkl_translates() {
        assert_eq!(vk_to_logical(0x48), Some(LogicalKey::H));
        assert_eq!(vk_to_logical(0x4A), Some(LogicalKey::J));
        assert_eq!(vk_to_logical(0x4B), Some(LogicalKey::K));
        assert_eq!(vk_to_logical(0x4C), Some(LogicalKey::L));
    }

    #[test]
    fn t03_uo_translates() {
        assert_eq!(vk_to_logical(0x55), Some(LogicalKey::U));
        assert_eq!(vk_to_logical(0x4F), Some(LogicalKey::O));
    }

    #[test]
    fn t04_fd_translates() {
        assert_eq!(vk_to_logical(0x46), Some(LogicalKey::F));
        assert_eq!(vk_to_logical(0x44), Some(LogicalKey::D));
    }

    #[test]
    fn t05_ws_translates() {
        assert_eq!(vk_to_logical(0x57), Some(LogicalKey::W));
        assert_eq!(vk_to_logical(0x53), Some(LogicalKey::S));
    }

    #[test]
    fn t06_esc_translates() {
        assert_eq!(vk_to_logical(0x1B), Some(LogicalKey::Esc));
    }

    #[test]
    fn t07_unmapped_returns_none() {
        assert_eq!(vk_to_logical(0x0D), None); // VK_RETURN
        assert_eq!(vk_to_logical(0x41), None); // VK_A
        assert_eq!(vk_to_logical(0x09), None); // VK_TAB
    }

    #[test]
    fn t08_modifier_vks_are_unmapped() {
        assert_eq!(vk_to_logical(0x10), None); // VK_SHIFT
        assert_eq!(vk_to_logical(0x11), None); // VK_CONTROL
        assert_eq!(vk_to_logical(0x12), None); // VK_MENU (Alt)
        assert_eq!(vk_to_logical(0x5B), None); // VK_LWIN
    }
}
