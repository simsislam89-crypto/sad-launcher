//! Native Win32 in place of helper programs: registry values (was
//! `reg.exe`), `.lnk` shortcuts through the shell's COM object (was a
//! PowerShell script, ~1 s and blocked by some policies/antiviruses) and
//! the IPv4 route table (was parsing `route print`, whose output is
//! localized). Shared with the installer through `#[path]`.
//!
//! Plain `extern "system"` like the rest of the project — no new crates.

#![cfg(windows)]
#![allow(dead_code)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

type Hkey = isize;

const HKEY_CURRENT_USER: Hkey = 0x8000_0001u32 as i32 as isize;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_SZ: u32 = 1;
const RRF_RT_ANY: u32 = 0x0000_ffff;
const ERROR_SUCCESS: i32 = 0;

#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(
        key: Hkey,
        sub_key: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        sam: u32,
        security: *const c_void,
        result: *mut Hkey,
        disposition: *mut u32,
    ) -> i32;
    fn RegSetValueExW(key: Hkey, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
    fn RegCloseKey(key: Hkey) -> i32;
    fn RegDeleteKeyValueW(key: Hkey, sub_key: *const u16, name: *const u16) -> i32;
    fn RegGetValueW(
        key: Hkey,
        sub_key: *const u16,
        name: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        len: *mut u32,
    ) -> i32;
}

/// NUL-terminated UTF-16 for Win32 «W» functions.
pub fn wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

/// Writes a string value under HKCU (`HKCU\<sub_key>`), creating the key.
pub fn reg_set_string(sub_key: &str, name: &str, value: &str) -> bool {
    let (sub, name_w, value_w) = (wide(sub_key), wide(name), wide(value));
    let mut key: Hkey = 0;
    // SAFETY: every pointer is a live NUL-terminated buffer or null where
    // the API allows it; `key` is closed below on success.
    unsafe {
        let rc = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            0,
            std::ptr::null(),
            0,
            KEY_SET_VALUE,
            std::ptr::null(),
            &mut key,
            std::ptr::null_mut(),
        );
        if rc != ERROR_SUCCESS {
            return false;
        }
        let bytes = value_w.len() * 2;
        let rc = RegSetValueExW(key, name_w.as_ptr(), 0, REG_SZ, value_w.as_ptr().cast(), bytes as u32);
        RegCloseKey(key);
        rc == ERROR_SUCCESS
    }
}

/// Removes a value under HKCU; true when it is gone (or never existed).
pub fn reg_delete_value(sub_key: &str, name: &str) -> bool {
    let (sub, name_w) = (wide(sub_key), wide(name));
    // SAFETY: both pointers are live NUL-terminated buffers.
    let rc = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, sub.as_ptr(), name_w.as_ptr()) };
    rc == ERROR_SUCCESS || rc == 2 // ERROR_FILE_NOT_FOUND
}

/// True when HKCU\<sub_key> has a value called `name`.
pub fn reg_has_value(sub_key: &str, name: &str) -> bool {
    let (sub, name_w) = (wide(sub_key), wide(name));
    let mut len = 0u32;
    // SAFETY: null data with a length pointer only asks for the size.
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            sub.as_ptr(),
            name_w.as_ptr(),
            RRF_RT_ANY,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut len,
        )
    };
    rc == ERROR_SUCCESS
}

// ------------------------------------------------------------ shortcuts

#[repr(C)]
struct Guid(u32, u16, u16, [u8; 8]);

const CLSID_SHELL_LINK: Guid = Guid(0x0002_1401, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
const IID_ISHELL_LINK_W: Guid = Guid(0x0002_14F9, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
const IID_IPERSIST_FILE: Guid = Guid(0x0000_010B, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const CLSCTX_INPROC_SERVER: u32 = 0x1;

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        clsid: *const Guid,
        outer: *mut c_void,
        context: u32,
        iid: *const Guid,
        out: *mut *mut c_void,
    ) -> i32;
}

// Slots in the COM vtables (IUnknown first: 0 QueryInterface, 2 Release).
const QUERY_INTERFACE: usize = 0;
const RELEASE: usize = 2;
const LINK_SET_DESCRIPTION: usize = 7;
const LINK_SET_WORKING_DIRECTORY: usize = 9;
const LINK_SET_ICON_LOCATION: usize = 17;
const LINK_SET_PATH: usize = 20;
const PERSIST_SAVE: usize = 6;

type FnRelease = unsafe extern "system" fn(*mut c_void) -> u32;
type FnQuery = unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32;
type FnSetStr = unsafe extern "system" fn(*mut c_void, *const u16) -> i32;
type FnSetIcon = unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32;
type FnSave = unsafe extern "system" fn(*mut c_void, *const u16, i32) -> i32;

/// Function pointer at `slot` of a COM object's vtable.
///
/// # Safety
/// `obj` must be a live COM interface pointer whose vtable has `slot`.
unsafe fn slot(obj: *mut c_void, slot: usize) -> *const c_void {
    let vtable = *(obj as *const *const *const c_void);
    *vtable.add(slot)
}

/// Creates (or replaces) `link` pointing at `target`, with the target's
/// folder as working directory and its first icon. Runs on its own thread,
/// so COM state never mixes with the UI thread's (file dialogs use COM too).
pub fn create_shortcut(link: &Path, target: &Path, description: &str) -> bool {
    let (link, target, description) = (link.to_path_buf(), target.to_path_buf(), description.to_string());
    std::thread::spawn(move || shortcut_on_this_thread(&link, &target, &description))
        .join()
        .unwrap_or(false)
}

fn shortcut_on_this_thread(link: &Path, target: &Path, description: &str) -> bool {
    if let Some(parent) = link.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let workdir = target.parent().map(wide).unwrap_or_else(|| wide(""));
    let (link_w, target_w, desc_w) = (wide(link), wide(target), wide(description));
    // SAFETY: COM is initialized on this fresh thread and torn down at the
    // end; every interface pointer comes from CoCreateInstance /
    // QueryInterface, is checked for null and released exactly once; vtable
    // slots follow the documented IShellLinkW / IPersistFile layouts; all
    // strings are NUL-terminated buffers that outlive the calls.
    unsafe {
        let init = CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED);
        let mut shell: *mut c_void = std::ptr::null_mut();
        let created = CoCreateInstance(
            &CLSID_SHELL_LINK,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            &IID_ISHELL_LINK_W,
            &mut shell,
        );
        let mut ok = false;
        if created >= 0 && !shell.is_null() {
            let set_path: FnSetStr = std::mem::transmute(slot(shell, LINK_SET_PATH));
            let set_dir: FnSetStr = std::mem::transmute(slot(shell, LINK_SET_WORKING_DIRECTORY));
            let set_desc: FnSetStr = std::mem::transmute(slot(shell, LINK_SET_DESCRIPTION));
            let set_icon: FnSetIcon = std::mem::transmute(slot(shell, LINK_SET_ICON_LOCATION));
            let filled = set_path(shell, target_w.as_ptr()) >= 0
                && set_dir(shell, workdir.as_ptr()) >= 0
                && set_desc(shell, desc_w.as_ptr()) >= 0
                && set_icon(shell, target_w.as_ptr(), 0) >= 0;
            let query: FnQuery = std::mem::transmute(slot(shell, QUERY_INTERFACE));
            let mut persist: *mut c_void = std::ptr::null_mut();
            if filled && query(shell, &IID_IPERSIST_FILE, &mut persist) >= 0 && !persist.is_null() {
                let save: FnSave = std::mem::transmute(slot(persist, PERSIST_SAVE));
                ok = save(persist, link_w.as_ptr(), 1) >= 0;
                let release: FnRelease = std::mem::transmute(slot(persist, RELEASE));
                release(persist);
            }
            let release: FnRelease = std::mem::transmute(slot(shell, RELEASE));
            release(shell);
        }
        // S_OK / S_FALSE: this thread owns an init to undo.
        if init == 0 || init == 1 {
            CoUninitialize();
        }
        ok
    }
}

// ---------------------------------------------------------------- routes

#[link(name = "iphlpapi")]
extern "system" {
    fn GetIpForwardTable(table: *mut u8, size: *mut u32, order: i32) -> u32;
    fn GetIpAddrTable(table: *mut u8, size: *mut u32, order: i32) -> u32;
}

/// Calls a «size first» IP helper function into a byte buffer.
fn ip_table(call: unsafe extern "system" fn(*mut u8, *mut u32, i32) -> u32) -> Option<Vec<u8>> {
    let mut size = 0u32;
    // SAFETY: a null buffer with size 0 only reports the needed size; the
    // second call gets a buffer of exactly that many bytes.
    unsafe {
        call(std::ptr::null_mut(), &mut size, 0);
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        (call(buf.as_mut_ptr(), &mut size, 0) == 0).then_some(buf)
    }
}

fn u32_at(buf: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_ne_bytes(buf.get(at..at + 4)?.try_into().ok()?))
}

/// IPv4 addresses are stored in network byte order: the bytes as they lie.
fn ip_at(buf: &[u8], at: usize) -> Option<std::net::Ipv4Addr> {
    let b = buf.get(at..at + 4)?;
    Some(std::net::Ipv4Addr::new(b[0], b[1], b[2], b[3]))
}

/// One default route: (gateway, interface address, metric).
pub type DefaultRoute = (std::net::Ipv4Addr, std::net::Ipv4Addr, u32);

/// Every IPv4 default route (0.0.0.0/0) with its interface address — what
/// `route print -4` lists under «Active Routes» for 0.0.0.0.
pub fn default_routes() -> Vec<DefaultRoute> {
    // MIB_IPADDRROW: addr, index, mask, bcast, reasm, (u16, u16) = 24 bytes.
    let mut addrs: Vec<(u32, std::net::Ipv4Addr)> = Vec::new();
    if let Some(buf) = ip_table(GetIpAddrTable) {
        let n = u32_at(&buf, 0).unwrap_or(0) as usize;
        for i in 0..n {
            let at = 4 + i * 24;
            if let (Some(ip), Some(index)) = (ip_at(&buf, at), u32_at(&buf, at + 4)) {
                addrs.push((index, ip));
            }
        }
    }
    // MIB_IPFORWARDROW: 14 DWORDs = 56 bytes: dest, mask, policy, next hop,
    // if index, type, proto, age, next hop AS, metric1..5. On Vista+
    // metric1 already includes the interface metric (as `route print`).
    let mut out = Vec::new();
    if let Some(buf) = ip_table(GetIpForwardTable) {
        let n = u32_at(&buf, 0).unwrap_or(0) as usize;
        for i in 0..n {
            let at = 4 + i * 56;
            let (Some(dest), Some(mask), Some(gw), Some(index), Some(metric)) = (
                ip_at(&buf, at),
                ip_at(&buf, at + 4),
                ip_at(&buf, at + 12),
                u32_at(&buf, at + 16),
                u32_at(&buf, at + 36),
            ) else {
                continue;
            };
            if !dest.is_unspecified() || !mask.is_unspecified() {
                continue;
            }
            if let Some((_, iface)) = addrs.iter().find(|(idx, _)| *idx == index) {
                out.push((gw, *iface, metric));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_value_round_trip() {
        let key = r"Software\SadLauncherTest";
        let name = format!("probe-{}", std::process::id());
        assert!(reg_set_string(key, &name, "GpuPreference=2;"));
        assert!(reg_has_value(key, &name));
        assert!(reg_delete_value(key, &name));
        assert!(!reg_has_value(key, &name));
        assert!(reg_delete_value(key, &name), "deleting a missing value is fine");
    }

    #[test]
    fn shortcut_is_written() {
        let dir = std::env::temp_dir().join(format!("sad-lnk-{}", std::process::id()));
        let link = dir.join("Проверка ярлыка.lnk");
        let target = std::env::current_exe().unwrap();
        assert!(create_shortcut(&link, &target, "Sad Launcher"));
        let bytes = std::fs::read(&link).unwrap();
        // ShellLinkHeader: size 0x4C, then the shell link CLSID.
        assert_eq!(&bytes[..4], &[0x4C, 0, 0, 0]);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn route_table_reads() {
        // Any machine with a network has a default route; offline CI may not.
        for (gw, iface, metric) in default_routes() {
            println!("default via {gw} on {iface} metric {metric}");
            assert!(!iface.is_unspecified());
        }
    }
}
