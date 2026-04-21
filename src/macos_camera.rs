//! Minimal CoreMediaIO bindings used to detect whether any camera on the system
//! is currently in use, without requiring admin privileges.
//!
//! This replaces the previous approach of shelling out to `log stream`, which
//! requires the user to be an admin (see issue #119).
//!
//! The CoreMediaIO property `kCMIODevicePropertyDeviceIsRunningSomewhere`
//! reports, for each camera device, whether *any* process on the system is
//! currently capturing from it. Reading this property is a normal user-space
//! operation and does not require elevated privileges.

#![cfg(target_os = "macos")]

use std::ffi::c_void;
use std::mem;
use std::ptr;

// `CMIOObjectID` is a `UInt32` in CoreMediaIO.
type CmioObjectId = u32;

// Four-character codes from `<CoreMediaIO/CMIOHardware.h>`.
//
// The four-char-code helpers from Apple's headers are simply
// `(a << 24) | (b << 16) | (c << 8) | d` over ASCII bytes.
const fn fourcc(s: &[u8; 4]) -> u32 {
    ((s[0] as u32) << 24) | ((s[1] as u32) << 16) | ((s[2] as u32) << 8) | (s[3] as u32)
}

const K_CMIO_OBJECT_SYSTEM_OBJECT: CmioObjectId = 1;
const K_CMIO_HARDWARE_PROPERTY_DEVICES: u32 = fourcc(b"dev#");
const K_CMIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
const K_CMIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = fourcc(b"glob");
// `kCMIOObjectPropertyElementMain` (formerly `Master`) is defined as 0.
const K_CMIO_OBJECT_PROPERTY_ELEMENT_MAIN: u32 = 0;

#[repr(C)]
struct CmioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreMediaIO", kind = "framework")]
extern "C" {
    fn CMIOObjectGetPropertyDataSize(
        object_id: CmioObjectId,
        address: *const CmioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: *mut u32,
    ) -> i32;

    fn CMIOObjectGetPropertyData(
        object_id: CmioObjectId,
        address: *const CmioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: u32,
        data_used: *mut u32,
        data: *mut c_void,
    ) -> i32;
}

fn devices_address() -> CmioObjectPropertyAddress {
    CmioObjectPropertyAddress {
        selector: K_CMIO_HARDWARE_PROPERTY_DEVICES,
        scope: K_CMIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_CMIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    }
}

fn is_running_somewhere_address() -> CmioObjectPropertyAddress {
    CmioObjectPropertyAddress {
        selector: K_CMIO_DEVICE_PROPERTY_DEVICE_IS_RUNNING_SOMEWHERE,
        scope: K_CMIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: K_CMIO_OBJECT_PROPERTY_ELEMENT_MAIN,
    }
}

/// Returns the IDs of all CoreMediaIO devices currently known to the system.
fn list_camera_device_ids() -> Vec<CmioObjectId> {
    let address = devices_address();
    let mut data_size: u32 = 0;

    // SAFETY: `address` points to a properly initialized
    // `CMIOObjectPropertyAddress` and `data_size` is a valid `&mut u32`.
    let status = unsafe {
        CMIOObjectGetPropertyDataSize(
            K_CMIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            &mut data_size,
        )
    };

    if status != 0 || data_size == 0 {
        return Vec::new();
    }

    let count = data_size as usize / mem::size_of::<CmioObjectId>();
    let mut device_ids: Vec<CmioObjectId> = vec![0; count];
    let mut data_used: u32 = 0;

    // SAFETY: `device_ids` is allocated to hold `data_size` bytes, which is
    // exactly the size CoreMediaIO reported.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            K_CMIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            data_size,
            &mut data_used,
            device_ids.as_mut_ptr() as *mut c_void,
        )
    };

    if status != 0 {
        return Vec::new();
    }

    let actual_count = data_used as usize / mem::size_of::<CmioObjectId>();
    device_ids.truncate(actual_count);
    device_ids
}

/// Returns whether the given device is currently being used by any process.
fn is_device_running(device_id: CmioObjectId) -> bool {
    let address = is_running_somewhere_address();
    let mut value: u32 = 0;
    let mut data_used: u32 = 0;

    // SAFETY: `value` is a valid `&mut u32` of the size we report.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            mem::size_of::<u32>() as u32,
            &mut data_used,
            &mut value as *mut u32 as *mut c_void,
        )
    };

    status == 0 && value != 0
}

/// Returns whether any camera on the system is currently in use.
pub fn any_camera_running() -> bool {
    list_camera_device_ids().into_iter().any(is_device_running)
}
