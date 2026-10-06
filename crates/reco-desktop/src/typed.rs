//! The character a key types on the keyboard's layout, so the preview's
//! character keys (R, F, + = - _ [ ]) follow the layout, as the Slint app
//! matched the typed text (owner's choice, Module 8). Makepad's key events
//! carry only the key's place; on Windows and Linux its text events carry
//! the character, but on macOS they come only while a text field has the
//! input method, so there the layout itself is asked (`UCKeyTranslate`).

use makepad_widgets::{KeyCode, KeyModifiers};

/// The character `key` types with `modifiers` on the current layout
/// (macOS); `None` elsewhere, where text events carry it.
pub fn typed(key: KeyCode, modifiers: &KeyModifiers) -> Option<char> {
    #[cfg(target_os = "macos")]
    {
        mac::typed(key, modifiers)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (key, modifiers);
        None
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::c_void;

    use super::*;

    type CfRef = *const c_void;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn TISCopyCurrentKeyboardLayoutInputSource() -> CfRef;
        #[cfg(test)]
        fn TISCreateInputSourceList(properties: CfRef, include_all_installed: u8) -> CfRef;
        fn TISGetInputSourceProperty(source: CfRef, key: CfRef) -> CfRef;
        static kTISPropertyUnicodeKeyLayoutData: CfRef;
        #[cfg(test)]
        static kTISPropertyInputSourceID: CfRef;
        fn LMGetKbdType() -> u8;
        #[allow(clippy::too_many_arguments)]
        fn UCKeyTranslate(
            layout: *const c_void,
            virtual_key: u16,
            action: u16,
            modifier_state: u32,
            keyboard_type: u32,
            options: u32,
            dead_key_state: *mut u32,
            max_length: usize,
            actual_length: *mut usize,
            chars: *mut u16,
        ) -> i32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: CfRef);
        fn CFDataGetBytePtr(data: CfRef) -> *const u8;
        #[cfg(test)]
        fn CFStringCreateWithBytes(
            alloc: CfRef,
            bytes: *const u8,
            length: isize,
            encoding: u32,
            external: u8,
        ) -> CfRef;
        #[cfg(test)]
        fn CFDictionaryCreate(
            alloc: CfRef,
            keys: *const CfRef,
            values: *const CfRef,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> CfRef;
        #[cfg(test)]
        fn CFArrayGetCount(array: CfRef) -> isize;
        #[cfg(test)]
        fn CFArrayGetValueAtIndex(array: CfRef, index: isize) -> CfRef;
        #[cfg(test)]
        static kCFTypeDictionaryKeyCallBacks: c_void;
        #[cfg(test)]
        static kCFTypeDictionaryValueCallBacks: c_void;
    }

    #[cfg(test)]
    const UTF8: u32 = 0x0800_0100;
    const KEY_DOWN: u16 = 0;
    const NO_DEAD_KEYS: u32 = 1;
    /// Carbon's modifier bits, as `UCKeyTranslate` takes them (`>> 8`).
    const SHIFT: u32 = 1 << 1;
    const OPTION: u32 = 1 << 3;

    /// macOS's virtual key code for a key that types (Makepad's table,
    /// apple_util.rs, the other way round).
    pub(super) fn virtual_key(key: KeyCode) -> Option<u16> {
        use KeyCode::*;
        Some(match key {
            KeyA => 0x00,
            KeyS => 0x01,
            KeyD => 0x02,
            KeyF => 0x03,
            KeyH => 0x04,
            KeyG => 0x05,
            KeyZ => 0x06,
            KeyX => 0x07,
            KeyC => 0x08,
            KeyV => 0x09,
            KeyB => 0x0b,
            KeyQ => 0x0c,
            KeyW => 0x0d,
            KeyE => 0x0e,
            KeyR => 0x0f,
            KeyY => 0x10,
            KeyT => 0x11,
            Key1 => 0x12,
            Key2 => 0x13,
            Key3 => 0x14,
            Key4 => 0x15,
            Key6 => 0x16,
            Key5 => 0x17,
            Equals => 0x18,
            Key9 => 0x19,
            Key7 => 0x1a,
            Minus => 0x1b,
            Key8 => 0x1c,
            Key0 => 0x1d,
            RBracket => 0x1e,
            KeyO => 0x1f,
            KeyU => 0x20,
            LBracket => 0x21,
            KeyI => 0x22,
            KeyP => 0x23,
            KeyL => 0x25,
            KeyJ => 0x26,
            Quote => 0x27,
            KeyK => 0x28,
            Semicolon => 0x29,
            Backslash => 0x2a,
            Comma => 0x2b,
            Slash => 0x2c,
            KeyN => 0x2d,
            KeyM => 0x2e,
            Period => 0x2f,
            Backtick => 0x32,
            NumpadDecimal => 0x41,
            NumpadMultiply => 0x43,
            NumpadAdd => 0x45,
            NumpadDivide => 0x4b,
            NumpadSubtract => 0x4e,
            NumpadEquals => 0x51,
            Numpad0 => 0x52,
            Numpad1 => 0x53,
            Numpad2 => 0x54,
            Numpad3 => 0x55,
            Numpad4 => 0x56,
            Numpad5 => 0x57,
            Numpad6 => 0x58,
            Numpad7 => 0x59,
            Numpad8 => 0x5b,
            Numpad9 => 0x5c,
            _ => return None,
        })
    }

    /// The character `key` types on the current layout.
    pub(super) fn typed(key: KeyCode, modifiers: &KeyModifiers) -> Option<char> {
        // SAFETY: an owned input source, released below; its layout data
        // lives as long as it does.
        unsafe {
            let source = TISCopyCurrentKeyboardLayoutInputSource();
            if source.is_null() {
                return None;
            }
            let found = translate(source, key, modifiers);
            CFRelease(source);
            found
        }
    }

    /// The character `key` types on the layout named `id`
    /// ("com.apple.keylayout.German"); `None` if it isn't installed.
    #[cfg(test)]
    pub(super) fn typed_on(id: &str, key: KeyCode, modifiers: &KeyModifiers) -> Option<char> {
        // SAFETY: every object created here is released here; the input
        // source is read while the list that holds it lives.
        unsafe {
            let name =
                CFStringCreateWithBytes(std::ptr::null(), id.as_ptr(), id.len() as isize, UTF8, 0);
            let keys = [kTISPropertyInputSourceID];
            let values = [name];
            let filter = CFDictionaryCreate(
                std::ptr::null(),
                keys.as_ptr(),
                values.as_ptr(),
                1,
                &kCFTypeDictionaryKeyCallBacks,
                &kCFTypeDictionaryValueCallBacks,
            );
            let list = TISCreateInputSourceList(filter, 1);
            let found = if !list.is_null() && CFArrayGetCount(list) > 0 {
                translate(CFArrayGetValueAtIndex(list, 0), key, modifiers)
            } else {
                None
            };
            if !list.is_null() {
                CFRelease(list);
            }
            CFRelease(filter);
            CFRelease(name);
            found
        }
    }

    /// `key` through `source`'s layout.
    ///
    /// # Safety
    /// `source` is a live input source.
    unsafe fn translate(source: CfRef, key: KeyCode, modifiers: &KeyModifiers) -> Option<char> {
        let code = virtual_key(key)?;
        let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
        if data.is_null() {
            return None;
        }
        let layout = CFDataGetBytePtr(data) as *const c_void;
        let state =
            if modifiers.shift { SHIFT } else { 0 } | if modifiers.alt { OPTION } else { 0 };
        let mut dead = 0u32;
        let mut length = 0usize;
        let mut chars = [0u16; 4];
        let status = UCKeyTranslate(
            layout,
            code,
            KEY_DOWN,
            state,
            u32::from(LMGetKbdType()),
            NO_DEAD_KEYS,
            &mut dead,
            chars.len(),
            &mut length,
            chars.as_mut_ptr(),
        );
        if status != 0 || length == 0 {
            return None;
        }
        char::decode_utf16(chars[..length.min(chars.len())].iter().copied())
            .next()?
            .ok()
            .filter(|c| !c.is_control())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn with(shift: bool, alt: bool) -> KeyModifiers {
            KeyModifiers {
                shift,
                alt,
                ..Default::default()
            }
        }

        #[test]
        fn keys_have_their_virtual_codes() {
            assert_eq!(virtual_key(KeyCode::KeyR), Some(0x0f));
            assert_eq!(virtual_key(KeyCode::RBracket), Some(0x1e));
            assert_eq!(virtual_key(KeyCode::Equals), Some(0x18));
            assert_eq!(virtual_key(KeyCode::NumpadAdd), Some(0x45));
            assert_eq!(virtual_key(KeyCode::Escape), None, "it types nothing");
        }

        /// macOS reads keyboard layouts only on the main thread (elsewhere
        /// it aborts), and the test harness runs tests on threads of its
        /// own: this runs the layout tests again, one at a time, which
        /// puts them on the main thread.
        #[test]
        fn the_layouts_are_read_on_the_main_thread() {
            let exe = std::env::current_exe().expect("this test binary");
            let out = std::process::Command::new(exe)
                .args([
                    "--ignored",
                    "--test-threads=1",
                    "typed::mac::tests::main_thread_",
                ])
                .output()
                .expect("runs");
            let text = String::from_utf8_lossy(&out.stdout);
            assert!(
                out.status.success() && text.contains("2 passed"),
                "{text}{}",
                String::from_utf8_lossy(&out.stderr)
            );
        }

        #[test]
        #[ignore = "run on the main thread by the_layouts_are_read_on_the_main_thread"]
        fn main_thread_a_us_keyboard_types_what_its_keys_say() {
            const US: &str = "com.apple.keylayout.US";
            let none = with(false, false);
            assert_eq!(typed_on(US, KeyCode::Equals, &none), Some('='));
            assert_eq!(typed_on(US, KeyCode::Equals, &with(true, false)), Some('+'));
            assert_eq!(typed_on(US, KeyCode::Minus, &with(true, false)), Some('_'));
            assert_eq!(typed_on(US, KeyCode::LBracket, &none), Some('['));
            assert_eq!(typed_on(US, KeyCode::KeyR, &none), Some('r'));
            assert_eq!(typed_on(US, KeyCode::KeyF, &with(true, false)), Some('F'));
            assert_eq!(typed_on(US, KeyCode::NumpadAdd, &none), Some('+'));
        }

        #[test]
        #[ignore = "run on the main thread by the_layouts_are_read_on_the_main_thread"]
        fn main_thread_a_german_keyboard_types_plus_where_us_has_a_bracket() {
            const DE: &str = "com.apple.keylayout.German";
            let none = with(false, false);
            assert_eq!(typed_on(DE, KeyCode::RBracket, &none), Some('+'));
            assert_eq!(typed_on(DE, KeyCode::Slash, &none), Some('-'));
            assert_eq!(typed_on(DE, KeyCode::Key5, &with(false, true)), Some('['));
            assert_eq!(typed_on(DE, KeyCode::Key6, &with(false, true)), Some(']'));
            assert_eq!(typed_on(DE, KeyCode::KeyR, &none), Some('r'));
        }
    }
}
