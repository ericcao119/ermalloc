#![no_std]
#![allow(dead_code)]

mod policies;
mod weak;
mod ffi;
mod alloc;

#[cfg(test)]
mod tests {

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
