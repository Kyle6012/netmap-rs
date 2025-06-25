#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[cfg(feature = "sys")]
include!(concat!(env!("OUT_DIR"), "/binding.rs"));

// When `sys` is off, provide no symbols:
#[cfg(not(feature = "sys"))]
mod ffi {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_sizes() {
        //verify that struct sizes match expected values
        assert_eq!(std::mem::size_of::<netmap_ring>(), 128);
    }
}
