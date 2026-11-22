pub mod linux;
pub mod macos;
pub mod output_enigo;
#[cfg(windows)]
pub mod windows;

pub use output_enigo::EnigoAdapter;

#[cfg(windows)]
pub use windows::*;

#[cfg(windows)]
pub fn is_capslock_pressed() -> bool {
    unsafe {
        windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(
            windows_sys::Win32::UI::Input::KeyboardAndMouse::VK_CAPITAL as i32,
        ) < 0
    }
}
