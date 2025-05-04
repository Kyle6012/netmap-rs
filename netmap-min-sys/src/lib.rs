#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

include!(concat!(env!("OUT_DIR"), "/binding.rs"));

#[cfg(test)]
mod tests {
    user super::*;

    #[test]
    fn test_struct_sizes(){
        //verify that struct sizes match expected values
        assert_eq!(std::mem::size_of::<netmap_ring>(, 128));
    }
}