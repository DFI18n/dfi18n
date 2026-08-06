mod string;
pub use string::*;

mod vector;
pub use vector::*;

// Assign UTF-8 bytes into an existing C++ std::string at `target`.
pub fn string_assign(target: *mut std::ffi::c_void, bytes: &[u8]) {
  let c_string = std::ffi::CString::new(bytes).unwrap();
  unsafe { string::cpp_assign_string(target, c_string.as_ptr()) };
}
