use std::ffi::{CStr, CString, OsString};
use std::os::raw::c_char;
use std::{fs, path, ptr};

use glib::translate::FromGlibPtrFull;

/// creates a rust-owned string, copies the memory from c-allocated
/// glib code - and then frees it correctly so we do not leak
/// or hold dangling pointers
pub unsafe fn take_c_owned_string(ptr: *mut c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }

    // Copy to Rust String
    let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();

    // Free with GLib
    glib::ffi::g_free(ptr as *mut _);

    // returned an owned rust-allocated string
    Some(s)
}

pub fn call_with_gerror<T, F>(f: F) -> Result<*mut T, glib::error::Error>
where
    F: FnOnce(*mut *mut glib::ffi::GError) -> *mut T,
{
    // initialize error to a null-pointer
    let mut err = ptr::null_mut();

    // call the c-library function
    let return_value = f(&mut err as *mut *mut glib::ffi::GError);

    if return_value.is_null() {
        unsafe { Err(glib::error::Error::from_glib_full(err)) }
    } else {
        Ok(return_value)
    }
}

pub fn path_to_glib_url<P: AsRef<path::Path>>(p: P) -> Result<CString, glib::error::Error> {
    // canonicalize path, try to wrap failures into a glib error
    let canonical = fs::canonicalize(p).map_err(|_| {
        glib::error::Error::new(
            glib::FileError::Noent,
            "Could not turn path into canonical path. Maybe it does not exist?",
        )
    })?;

    // construct path string
    let mut osstr_path: OsString = "file:///".into();
    osstr_path.push(canonical);

    // we need to round-trip to string, as not all os strings are 8 bytes
    let pdf_string = osstr_path.into_string().map_err(|_| {
        glib::error::Error::new(
            glib::FileError::Inval,
            "Path invalid (contains non-utf8 characters)",
        )
    })?;

    CString::new(pdf_string).map_err(|_| {
        glib::error::Error::new(
            glib::FileError::Inval,
            "Path invalid (contains NUL characters)",
        )
    })
}
