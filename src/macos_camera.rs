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

/// Returns the IDs of all CoreMediaIO devices currently known to the system,
/// or `None` if a CoreMediaIO query failed (so the caller can distinguish
/// transient errors from "no devices present").
fn list_camera_device_ids() -> Option<Vec<CmioObjectId>> {
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

    if status != 0 {
        log::warn!(
            "CMIOObjectGetPropertyDataSize for device list failed with status {}",
            status
        );
        return None;
    }

    if data_size == 0 {
        return Some(Vec::new());
    }

    let id_size = mem::size_of::<CmioObjectId>();
    if (data_size as usize) % id_size != 0 {
        log::warn!(
            "CoreMediaIO reported a device-list byte size ({}) that is not a multiple of \
             size_of::<CMIOObjectID>() ({}); refusing to read",
            data_size,
            id_size
        );
        return None;
    }

    let count = data_size as usize / id_size;
    let mut device_ids: Vec<CmioObjectId> = vec![0; count];
    // The number of bytes we actually allocated; pass this (not the
    // originally-reported `data_size`) to CoreMediaIO so we never tell it
    // the buffer is larger than it is.
    let buffer_size = (count * id_size) as u32;
    let mut data_used: u32 = 0;

    // SAFETY: `device_ids` is allocated to hold `buffer_size` bytes, which
    // is what we pass as the buffer size below.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            K_CMIO_OBJECT_SYSTEM_OBJECT,
            &address,
            0,
            ptr::null(),
            buffer_size,
            &mut data_used,
            device_ids.as_mut_ptr() as *mut c_void,
        )
    };

    if status != 0 {
        log::warn!(
            "CMIOObjectGetPropertyData for device list failed with status {}",
            status
        );
        return None;
    }

    if (data_used as usize) % id_size != 0 {
        log::warn!(
            "CoreMediaIO returned a device-list byte count ({}) that is not a multiple of \
             size_of::<CMIOObjectID>() ({})",
            data_used,
            id_size
        );
        return None;
    }

    let actual_count = data_used as usize / id_size;
    device_ids.truncate(actual_count);
    Some(device_ids)
}

/// Returns whether the given device is currently being used by any process,
/// or `None` if the CoreMediaIO query failed or returned an unexpected
/// payload size.
fn is_device_running(device_id: CmioObjectId) -> Option<bool> {
    let address = is_running_somewhere_address();
    let mut value: u32 = 0;
    let mut data_used: u32 = 0;
    let expected_size = mem::size_of::<u32>() as u32;

    // SAFETY: `value` is a valid `&mut u32` of the size we report.
    let status = unsafe {
        CMIOObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            expected_size,
            &mut data_used,
            &mut value as *mut u32 as *mut c_void,
        )
    };

    if status != 0 {
        log::warn!(
            "CMIOObjectGetPropertyData(IsRunningSomewhere) for device {} failed with status {}",
            device_id,
            status
        );
        return None;
    }

    if data_used != expected_size {
        log::warn!(
            "CMIOObjectGetPropertyData(IsRunningSomewhere) for device {} returned {} bytes, \
             expected {}",
            device_id,
            data_used,
            expected_size
        );
        return None;
    }

    Some(value != 0)
}

/// Returns whether any camera on the system is currently in use, or `None`
/// if a CoreMediaIO query failed. Callers should treat `None` as "unknown"
/// (e.g. keep the previous observed state) rather than as "no camera
/// running".
pub fn any_camera_running() -> Option<bool> {
    let device_ids = list_camera_device_ids()?;
    for id in device_ids {
        match is_device_running(id) {
            Some(true) => return Some(true),
            Some(false) => {}
            None => return None,
        }
    }
    Some(false)
}
