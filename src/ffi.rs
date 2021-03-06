extern crate core;

use libc::*;

use core::ptr;
use core::convert::TryFrom;
use core::fmt;
use core::slice;

use crate::policies::*;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub enum ErPolicyRaw {
    Nil,
    Redundancy,
    ReedSolomon,
    Encrypted,
}

#[derive(Debug, Copy, Clone)]
pub enum FfiError {
    PolicyValueUnknown,
    PolicyDataWasNull,
    MoreThanMaxPolicies,
}

impl fmt::Display for FfiError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct ErPolicyListRaw {
    policy: ErPolicyRaw,
    policy_data: *const c_void,
    er_list_policy_raw: *const ErPolicyListRaw,
}

impl ErPolicyListRaw {
    fn new(policy: ErPolicyRaw, policy_data: *const c_void, er_list_policy_raw: *const ErPolicyListRaw) -> Self {
        ErPolicyListRaw { policy, policy_data, er_list_policy_raw }
    }
}

impl Default for ErPolicyListRaw {
    fn default() -> Self {
        ErPolicyListRaw::new(ErPolicyRaw::Nil, ptr::null(), ptr::null())
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ErPolicyListNonNull {
    policy: ErPolicyRaw,
    policy_data: Option<ptr::NonNull<c_void>>,
    er_list_policy: Option<ptr::NonNull<ErPolicyListRaw>>,
}

impl ErPolicyListNonNull {
    fn new(policy: ErPolicyRaw, policy_data: Option<ptr::NonNull<c_void>>, er_list_policy: Option<ptr::NonNull<ErPolicyListRaw>>) -> Self {
        ErPolicyListNonNull { policy, policy_data, er_list_policy }
    }
}

impl Default for ErPolicyListNonNull {
    fn default() -> Self {
        ErPolicyListNonNull::new(ErPolicyRaw::Nil, None, None)
    }
}

impl Iterator for ErPolicyListNonNull {
    type Item = Self;

    fn next(&mut self) -> Option<Self::Item> {
        match self.er_list_policy {
            None => None,
            Some(ptr) => {
                unsafe {
                    ErPolicyListNonNull::try_from(*ptr.as_ptr()).ok()
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.er_list_policy {
            None => (0, None),
            Some(_) => (1, None)
        }
    }
}

impl From<ErPolicyListNonNull> for Policy {
    fn from(raw: ErPolicyListNonNull) -> Self {
        match raw.policy {
            ErPolicyRaw::Nil => Policy::Nil,
            ErPolicyRaw::Redundancy => {
                let num;
                match raw.policy_data {
                    Some(data) => {
                        unsafe {
                            let ptr = data.clone().cast::<u32>();
                            num = *(ptr.as_ptr());
                        } 
                    },
                    None => {
                        num = default_redundancy();
                    }
                }
                Policy::Redundancy(num)
            },
            ErPolicyRaw::ReedSolomon => {
                let num;
                match raw.policy_data {
                    Some(data) => {
                        unsafe {
                            let ptr = data.clone().cast::<u32>();
                            num = *(ptr.as_ptr());
                        } 
                    },
                    None => {
                        num = default_rs();
                    }
                }
                Policy::ReedSolomon(num)
            },
            ErPolicyRaw::Encrypted => Policy::Encrypted
        }
    }
}

impl TryFrom<ErPolicyListRaw> for ErPolicyListNonNull {
    type Error = FfiError;

    fn try_from(raw: ErPolicyListRaw) -> Result<Self, Self::Error> {
        let next;
        if raw.er_list_policy_raw.is_null() {
            next = None;
        } else {
            unsafe {
                next = Some(ptr::NonNull::new_unchecked(raw.er_list_policy_raw as *mut _));
            }
        }
        match raw.policy {
            ErPolicyRaw::Nil | ErPolicyRaw::Encrypted => {
                Ok(ErPolicyListNonNull::new(raw.policy, None, next))
            },
            ErPolicyRaw::Redundancy => {
                if raw.policy_data.is_null() {
                    let policy_data = None;
                    Ok(ErPolicyListNonNull::new(raw.policy, policy_data, next))
                } else {
                    let policy_data = unsafe {
                        Some(ptr::NonNull::new_unchecked(raw.policy_data as *mut _))
                    };
                    Ok(ErPolicyListNonNull::new(raw.policy, policy_data, next))
                }
            },
            ErPolicyRaw::ReedSolomon => {
                if raw.policy_data.is_null() {
                    let policy_data = None;
                    Ok(ErPolicyListNonNull::new(raw.policy, policy_data, next))
                } else {
                    let policy_data = unsafe {
                        Some(ptr::NonNull::new_unchecked(raw.policy_data as *mut _))
                    };
                    Ok(ErPolicyListNonNull::new(raw.policy, policy_data, next))
                }
            }
        }
    }
}

// TODO: move to appropriate file once params are determined
// TODO: customize based on use case
fn default_redundancy() -> u32 {
    3
}

fn default_rs() -> u32 {
    3
}

fn setup_policy_helper(size: size_t, policies: *const ErPolicyListRaw) -> Option<[Policy; MAX_POLICIES]> {
    if size == 0 {
        return None;
    }

    let mut policy_arr = [Policy::Nil; MAX_POLICIES];
    let mut policy_arr_ordered = [Policy::Nil; MAX_POLICIES];
    if policies != ptr::null() {
        let mut head = ErPolicyListNonNull::try_from(unsafe { *policies }).expect("policy list generation error");
        for i in 0.. {
            if i >= MAX_POLICIES {
                panic!("{}", FfiError::MoreThanMaxPolicies);
            }
            let pol = Policy::from(head);
            match pol {
                Policy::Redundancy(_) => {
                    policy_arr[0] = pol;
                }
                Policy::ReedSolomon(_) => {
                    policy_arr[1] = pol;
                }
                Policy::Encrypted => {
                    policy_arr[2] = pol;
                }
                _ => (),
            }
            head = match head.next() {
                None => break,
                Some(erplnn) => erplnn
            };
        }

        // order the policies Redundancy -> ReedSol -> Encrypt
        let mut idx = 0;
        for pol in policy_arr.iter() {
            match pol {
                Policy::Nil => continue,
                _ => {
                    policy_arr_ordered[idx] = *pol;
                    idx += 1
                }
            }
        }
    }

    Some(policy_arr)
}

#[no_mangle]
pub unsafe extern "C" fn er_malloc(size: size_t, policies: *const ErPolicyListRaw) -> *mut c_void {
    match setup_policy_helper(size, policies) {
        Some(policy_arr) => AllocBlock::new(size, &policy_arr, false).as_ptr().add(1) as *mut c_void,
        None => ptr::null::<c_void>() as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn er_free(ptr: *const c_void)  {
    AllocBlock::drop(AllocBlock::from_usr_ptr_mut(ptr as *mut u8));
}

#[no_mangle]
pub unsafe extern "C" fn er_calloc(nmemb: size_t, size: size_t, policies: *const ErPolicyListRaw) -> *mut c_void {
    let bytes: size_t = match nmemb.checked_mul(size) {
        Some(u) => u,
        None => return ptr::null::<c_void>() as *mut c_void
    };
    match setup_policy_helper(size, policies) {
        Some(policy_arr) => AllocBlock::new(bytes, &policy_arr, true).as_ptr().add(1) as *mut c_void,
        None => ptr::null::<c_void>() as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn er_realloc(ptr: *const c_void, size: size_t, policies: *const ErPolicyListRaw) -> *mut c_void {
    if size == 0 {
        er_free(ptr);
        return ptr::null::<c_void>() as *mut c_void
    }
    match setup_policy_helper(size, policies) {
        Some(policy_arr) => AllocBlock::renew(AllocBlock::from_usr_ptr_mut(ptr as *mut u8), size, &policy_arr).as_ptr().add(1) as *mut c_void,
        None => ptr::null::<c_void>() as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn er_reallocarray(ptr: *const c_void, nmemb: size_t, size: size_t, policies: *const ErPolicyListRaw) -> *mut c_void {
    match nmemb.checked_mul(size) {
        Some(b) => er_realloc(ptr, b, policies),
        None => ptr::null::<c_void>() as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn er_setup_policies(ptr: *const c_void) {
    let w = AllocBlock::from_usr_ptr_mut(ptr as *mut u8);
    AllocBlock::apply_policy_ffi(w);
}

#[no_mangle]
pub unsafe extern "C" fn er_correct_buffer(ptr: *mut c_void) -> c_int {
    let w = AllocBlock::from_usr_ptr_mut(ptr as *mut u8);
    AllocBlock::correct_buffer_ffi(w) as c_int
}

#[no_mangle]
pub unsafe extern "C" fn er_read_buf(base: *mut c_void, dest: *mut c_void, offset: size_t, len: size_t) -> c_int {
    let c = er_correct_buffer(base);
    if c < 0 {
        return c;
    }
    
    let w_decrypted = AllocBlock::from_usr_ptr_mut(base as *mut u8);
    AllocBlock::decrypt_buffer_ffi(w_decrypted);

    let w = AllocBlock::from_usr_ptr_mut(base as *mut u8);
    let src_buf = AllocBlock::data_slice_ffi(w).split_at_mut(offset).1.split_at_mut(len).0;
    let dst_buf = slice::from_raw_parts_mut(dest as *mut u8, len);
    dst_buf.copy_from_slice(src_buf);

    let w_recrypt = AllocBlock::from_usr_ptr_mut(base as *mut u8);
    AllocBlock::encrypt_buffer_ffi(w_recrypt);
    c
}

#[no_mangle]
pub unsafe extern "C" fn er_write_buf(base: *mut c_void, src: *const c_void, offset: size_t, len: size_t) -> c_int {
    let w = AllocBlock::from_usr_ptr_mut(base as *mut u8);
    let dst_buf = AllocBlock::data_slice_ffi(w).split_at_mut(offset).1.split_at_mut(len).0;
    let src_buf = slice::from_raw_parts_mut(src as *mut u8, len);

    let w_decrypted = AllocBlock::from_usr_ptr_mut(base as *mut u8);
    AllocBlock::decrypt_buffer_ffi(w_decrypted);

    dst_buf.copy_from_slice(src_buf);

    er_setup_policies(base);
    0
}
