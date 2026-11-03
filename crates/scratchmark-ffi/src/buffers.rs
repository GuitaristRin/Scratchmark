use scratchmark_core::{Error, Result};
use std::{
    mem::{align_of, size_of},
    slice,
};

pub(crate) fn required<T>(p: *const T) -> Result<()> {
    if p.is_null() || p as usize % align_of::<T>() != 0 {
        Err(Error::InvalidArgument)
    } else {
        Ok(())
    }
}

fn check_array<T>(p: *const T, n: u32, limit: usize) -> Result<usize> {
    let n = n as usize;
    if n > limit || n > isize::MAX as usize / size_of::<T>() {
        return Err(Error::LimitExceeded);
    }
    if n > 0 {
        required(p)?;
    }
    Ok(n)
}

// 调用者承诺底层分配有效且生命周期覆盖借用；空数组不构造 NULL slice。
pub(crate) unsafe fn input<'a, T>(p: *const T, n: u32, limit: usize) -> Result<&'a [T]> {
    let n = check_array(p, n, limit)?;
    if n == 0 {
        return Ok(&[]);
    }
    Ok(unsafe { slice::from_raw_parts(p, n) })
}

pub(crate) unsafe fn output<'a, T>(p: *mut T, n: u32, limit: usize) -> Result<&'a mut [T]> {
    let n = check_array(p, n, limit)?;
    if n == 0 {
        return Ok(&mut []);
    }
    Ok(unsafe { slice::from_raw_parts_mut(p, n) })
}
