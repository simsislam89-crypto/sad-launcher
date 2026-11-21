//! Vendored `webview2-com-sys` 0.39.1. Upstream links the WebView2 loader
//! at build time; on the GNU toolchain that is `WebView2Loader.dll` in the
//! exe's import table — no DLL next to the exe, no start at all. Here the
//! loader functions resolve at run time from Microsoft's own loader DLL,
//! embedded in the binary and written next to the user data on first use.
//! MSVC builds keep the upstream static link.

#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::all
)]
pub mod Microsoft {
    pub mod Web {
        pub mod WebView2 {
            pub mod Win32 {
                mod windows_core {
                    #[cfg(target_env = "msvc")]
                    macro_rules! link_webview2 {
                        ($library:literal $abi:literal fn $($function:tt)*) => (
                            #[link(name = "WebView2LoaderStatic", kind = "static")]
                            extern $abi {
                                pub fn $($function)*;
                            }
                        )
                    }

                    #[cfg(not(target_env = "msvc"))]
                    macro_rules! link_webview2 {
                        ($library:literal $abi:literal fn $name:ident ($($arg:ident : $ty:ty),* $(,)?) -> $ret:ty) => (
                            #[allow(non_snake_case)]
                            unsafe fn $name($($arg: $ty),*) -> $ret {
                                type F = unsafe extern $abi fn($($ty),*) -> $ret;
                                match crate::loader::resolve(concat!(stringify!($name), "\0")) {
                                    // SAFETY: the symbol comes from Microsoft's
                                    // loader and has exactly this signature.
                                    Some(p) => unsafe { core::mem::transmute::<*const core::ffi::c_void, F>(p)($($arg),*) },
                                    // HRESULT_FROM_WIN32(ERROR_MOD_NOT_FOUND)
                                    None => windows_core::HRESULT(0x8007_007Eu32 as i32),
                                }
                            }
                        )
                    }

                    pub(crate) use ::windows_core::*;
                    pub(crate) use link_webview2 as link;
                }

                include!("bindings.rs");
            }
        }
    }
}

pub mod declared_interfaces;

#[cfg(not(target_env = "msvc"))]
mod loader {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use std::sync::OnceLock;

    #[cfg(target_arch = "x86_64")]
    static DLL: &[u8] = include_bytes!("../x64/WebView2Loader.dll");
    #[cfg(not(target_arch = "x86_64"))]
    static DLL: &[u8] = &[];

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *const c_void;
    }

    /// Module handle of the loader, loaded once per process.
    fn module() -> Option<usize> {
        static MODULE: OnceLock<Option<usize>> = OnceLock::new();
        *MODULE.get_or_init(|| {
            if DLL.is_empty() {
                return None;
            }
            // %LOCALAPPDATA%\Sad Launcher\WebView2Loader-<size>.dll: the
            // size in the name keeps an old copy from shadowing a new one.
            let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
            let dir = base.join("Sad Launcher");
            let path = dir.join(format!("WebView2Loader-{}.dll", DLL.len()));
            let ok = std::fs::metadata(&path).map(|m| m.len() == DLL.len() as u64).unwrap_or(false);
            if !ok {
                std::fs::create_dir_all(&dir).ok()?;
                let tmp = path.with_extension("tmp");
                std::fs::write(&tmp, DLL).ok()?;
                let _ = std::fs::remove_file(&path);
                std::fs::rename(&tmp, &path).ok()?;
            }
            let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            // SAFETY: NUL-terminated path; the module stays loaded for the
            // life of the process.
            let h = unsafe { LoadLibraryW(wide.as_ptr()) };
            (!h.is_null()).then_some(h as usize)
        })
    }

    /// Address of `name` (NUL-terminated) in the loader, if available.
    pub fn resolve(name: &str) -> Option<*const c_void> {
        let module = module()?;
        // SAFETY: valid module handle and NUL-terminated ASCII name.
        let p = unsafe { GetProcAddress(module as *mut c_void, name.as_ptr()) };
        (!p.is_null()).then_some(p)
    }
}
