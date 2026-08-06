use std::{ffi, marker::PhantomData, ops::Deref, sync::Arc};

unsafe extern "C-unwind" {
  fn cpp_create_string() -> *mut ffi::c_void;
  fn cpp_create_string_from_cstr(cstr: *const ffi::c_char) -> *mut ffi::c_void;
  fn cpp_fill_string(ch: u8, len: usize) -> *mut ffi::c_void;
  fn cpp_delete_string(ptr: *mut ffi::c_void);
  pub fn cpp_assign_string(target: *mut ffi::c_void, cstr: *const ffi::c_char);
}

// C++ string wrapper
pub struct CppString<'a> {
  inner: Arc<CppStringInner<'a>>,
}

impl<'a> Deref for CppString<'a> {
  type Target = CppStringInner<'a>;

  // Deref implementation to access the inner CppStringInner
  fn deref(&self) -> &Self::Target {
    unsafe { Arc::as_ptr(&self.inner).as_ref_unchecked() }
  }
}

impl<'a> CppString<'a> {
  // Create a new CppString
  pub fn new() -> Self {
    let ptr = unsafe { cpp_create_string() };
    let inner = CppStringInner {
      ptr,
      _marker: PhantomData,
    };
    CppString { inner: Arc::new(inner) }
  }

  // Create a CppString from a Rust string slice
  pub fn from_slice(s: &[u8]) -> Self {
    let c_string = ffi::CString::new(s).unwrap();
    let ptr = unsafe { cpp_create_string_from_cstr(c_string.as_ptr()) };
    let inner = CppStringInner {
      ptr,
      _marker: PhantomData,
    };
    CppString { inner: Arc::new(inner) }
  }

  // Create a CppString filled with a specific character
  pub fn fill(ch: u8, len: usize) -> Self {
    let ptr = unsafe { cpp_fill_string(ch, len) };
    let inner = CppStringInner {
      ptr,
      _marker: PhantomData,
    };
    CppString { inner: Arc::new(inner) }
  }
}

// Inner structure for CppString reference counting
pub struct CppStringInner<'a> {
  ptr: *mut ffi::c_void,
  _marker: PhantomData<&'a ()>,
}

unsafe impl Send for CppStringInner<'static> {}
unsafe impl Sync for CppStringInner<'static> {}
impl<'a> Drop for CppStringInner<'_> {
  // Destroy the C++ string when the CppStringInner is dropped
  fn drop(&mut self) {
    unsafe { cpp_delete_string(self.ptr) };
  }
}

impl CppStringInner<'_> {
  // Get the raw C++ string pointer
  pub fn raw(&self) -> *const ffi::c_void {
    self.ptr
  }

  // Get a mutable raw C++ string pointer
  pub fn raw_mut(&self) -> *mut ffi::c_void {
    self.ptr
  }

  // Assign bytes into the existing C++ string
  pub fn assign(&self, bytes: &[u8]) {
    let c_string = ffi::CString::new(bytes).unwrap();
    unsafe { cpp_assign_string(self.ptr, c_string.as_ptr()) };
  }
}
