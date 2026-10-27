//! Platform boundary. The UI and shortcut code do not call Core Audio directly.
use serde::Serialize;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Microphone {
    pub id: u32,
    pub name: String,
    pub muted: bool,
}

#[cfg(target_os = "macos")]
mod macos {
    use super::Microphone;
    use std::{
        ffi::{c_char, c_void},
        mem::size_of,
        ptr,
    };

    #[repr(C)]
    struct Address {
        selector: u32,
        scope: u32,
        element: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    unsafe extern "C" {
        fn AudioObjectGetPropertyData(
            id: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: *mut u32,
            data: *mut c_void,
        ) -> i32;
        fn AudioObjectSetPropertyData(
            id: u32,
            address: *const Address,
            qualifier_size: u32,
            qualifier: *const c_void,
            size: u32,
            data: *const c_void,
        ) -> i32;
        fn AudioObjectHasProperty(id: u32, address: *const Address) -> u8;
        fn AudioObjectIsPropertySettable(
            id: u32,
            address: *const Address,
            writable: *mut u8,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFStringGetCString(
            value: *const c_void,
            buffer: *mut c_char,
            size: isize,
            encoding: u32,
        ) -> u8;
        fn CFRelease(value: *const c_void);
    }

    const fn four(value: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*value)
    }
    fn address(selector: &[u8; 4], scope: &[u8; 4]) -> Address {
        Address {
            selector: four(selector),
            scope: four(scope),
            element: 0,
        }
    }
    fn read<T: Copy + Default>(id: u32, address: &Address) -> Result<T, String> {
        let mut value = T::default();
        let mut size = size_of::<T>() as u32;
        // SAFETY: `value` is a properly aligned buffer of the requested size.
        let status = unsafe {
            AudioObjectGetPropertyData(
                id,
                address,
                0,
                ptr::null(),
                &mut size,
                &mut value as *mut _ as *mut c_void,
            )
        };
        if status != 0 {
            return Err(format!("Cannot read the microphone (Core Audio {status})."));
        }
        if size != size_of::<T>() as u32 {
            return Err("The microphone returned an unexpected property size.".into());
        }
        Ok(value)
    }
    fn device_name(id: u32) -> String {
        let Ok(value) = read::<usize>(id, &address(b"lnam", b"glob")) else {
            return "Microphone".into();
        };
        if value == 0 {
            return "Microphone".into();
        }
        let mut bytes = [0u8; 4096];
        // SAFETY: Core Audio returns an owned CFString. The output buffer is valid.
        let success = unsafe {
            CFStringGetCString(
                value as *const c_void,
                bytes.as_mut_ptr().cast(),
                bytes.len() as isize,
                0x08000100,
            )
        };
        unsafe {
            CFRelease(value as *const c_void);
        }
        if success == 0 {
            return "Microphone".into();
        }
        let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        String::from_utf8_lossy(&bytes[..end]).into_owned()
    }
    pub fn current() -> Result<Microphone, String> {
        let id = read::<u32>(1, &address(b"dIn ", b"glob"))?;
        if id == 0 {
            return Err("No system input microphone is available.".into());
        }
        let name = device_name(id);
        let property = address(b"mute", b"inpt");
        let mut writable = 0;
        // Only a writable device-wide input mute is accepted. No volume fallback.
        let present = unsafe { AudioObjectHasProperty(id, &property) } != 0;
        let status = unsafe { AudioObjectIsPropertySettable(id, &property, &mut writable) };
        if !present || status != 0 || writable == 0 {
            return Err(format!("{name} does not support device-wide software mute. Choose another input in macOS Sound settings."));
        }
        let muted = read::<u32>(id, &property)? != 0;
        Ok(Microphone { id, name, muted })
    }
    pub fn set_mute(id: u32, muted: bool) -> Result<(), String> {
        let value = u32::from(muted);
        let property = address(b"mute", b"inpt");
        // SAFETY: Core Audio receives a pointer to a live UInt32 value.
        let status = unsafe {
            AudioObjectSetPropertyData(
                id,
                &property,
                0,
                ptr::null(),
                4,
                &value as *const _ as *const c_void,
            )
        };
        if status != 0 {
            return Err(format!(
                "Cannot change microphone mute (Core Audio {status})."
            ));
        }
        let actual = read::<u32>(id, &property)? != 0;
        if actual != muted {
            return Err("The microphone did not confirm the requested mute state.".into());
        }
        Ok(())
    }
}

#[cfg(target_os = "macos")]
pub use macos::{current, set_mute};

#[cfg(not(target_os = "macos"))]
pub fn current() -> Result<Microphone, String> {
    Err("This version supports macOS. The Windows audio backend is not implemented.".into())
}
#[cfg(not(target_os = "macos"))]
pub fn set_mute(_: u32, _: bool) -> Result<(), String> {
    current().map(|_| ())
}

pub fn test_round_trip() -> Result<(), String> {
    let original = current()?;
    println!(
        "Microphone: {} (id {}), original muted={}",
        original.name, original.id, original.muted
    );
    // Restore the original value even if the readback assertion fails.
    struct Restore(Microphone);
    impl Drop for Restore {
        fn drop(&mut self) {
            if let Err(error) = set_mute(self.0.id, self.0.muted) {
                eprintln!("RESTORE FAILED: {error}");
            }
        }
    }
    let restore = Restore(original.clone());
    set_mute(original.id, !original.muted)?;
    println!("Changed and confirmed muted={}", !original.muted);
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let actual = current()?;
        if actual.id != original.id {
            return Err("The system input changed during the test. The original microphone state will be restored.".into());
        }
        if actual.muted == original.muted {
            return Err("The microphone did not keep its new mute state. Another app or the device changed it. Close other microphone control apps and repeat the test.".into());
        }
    }
    set_mute(original.id, original.muted)?;
    println!("Restored and confirmed muted={}", original.muted);
    std::mem::forget(restore);
    Ok(())
}
