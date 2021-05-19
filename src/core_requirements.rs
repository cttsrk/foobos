//! This holds the [`core`] basic requirements for things like libc routines

use core::mem::size_of;

/// libc `memcpy` implementation in Rust
///
/// # Parameters
///
/// * `dest` - Pointer to memory to copy to
/// * `src`  - Pointer to memory to copy from
/// * `n`    - Number of bytes to copy
///
/// # Returns
///
/// Pointer to `dest`
///
#[no_mangle]
#[cfg(target_arch = "x86_64")]
unsafe extern fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    asm!("rep movsb",
        inout("rcx") n    => _,
        inout("rdi") dest => _,
        inout("rsi") src  => _,
        options(nostack));

    dest
}

/// libc `memcpy` implementation in Rust
///
/// # Parameters
///
/// * `dest` - Pointer to memory to copy to
/// * `src`  - Pointer to memory to copy from
/// * `n`    - Number of bytes to copy
///
/// # Returns
///
/// Pointer to `dest`
///
#[no_mangle]
#[cfg(not(target_arch = "x86_64"))]
unsafe extern fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut ii = 0;

    while ii < n {
        let dest = dest.add(ii);
        let src  = src.add(ii);
        core::ptr::write(dest, core::ptr::read(src));
        ii += 1;
    }

    dest
}

/// libc `memmove` implementation in Rust
///
/// # Parameters
///
/// * `dest` - Pointer to memory to copy to
/// * `src`  - Pointer to memory to copy from
/// * `n`    - Number of bytes to copy
///
/// # Returns
///
/// Pointer to `dest`
///
#[no_mangle]
unsafe extern fn memmove(dest: *mut u8, src: *const u8, mut n: usize)
        -> *mut u8 {
    // Check if there is overlap with the source coming prior to the dest.
    // Even if there is overlap, if the destination is earlier in memory
    // than the source, we can just copy forwards
    // +----------+
    // | src      | src + n
    // +---+------+---+
    //     | dest     | dest + n
    //     +----------+

    // Determine if the dest comes after the source and there is overlap
    // between them
    if (dest as usize) > (src as usize) &&
            (src as usize).wrapping_add(n) > (dest as usize) {
        // There is at least one byte of overlap and the source is prior to
        // the destination

        // Compute the delta between the source and destination
        let overhang = dest as usize - src as usize;

        // If the non-overlapping region is quite small, don't even bother
        // doing forward based chunk copies, just copy in reverse
        if overhang < 64 {
            // `u64`-align the dest with one byte copies
            while n != 0 && (dest as usize)
                    .wrapping_add(n) & (size_of::<u64>() - 1) != 0 {
                n = n.wrapping_sub(1);
                core::ptr::write(dest.add(n),
                    core::ptr::read(src.add(n)));
            }

            // Do a reverse copy one `u64` at a time
            while n >= size_of::<u64>() {
                n = n.wrapping_sub(size_of::<u64>());

                // Read the value to copy
                let val = core::ptr::read_unaligned(src.add(n) as *const u64);

                // Write out the value
                core::ptr::write(dest.add(n) as *mut u64, val);
            }

            // Just copy the remainder
            while n != 0 {
                n = n.wrapping_sub(1);
                core::ptr::write(dest.add(n),
                    core::ptr::read(src.add(n)));
            }

            return dest;
        }

        // Copy the non-overlapping tail part while there are overhang
        // sized chunks
        while n >= overhang {
            // Update the length remaining
            n = n.wrapping_sub(overhang);

            // Copy the remaining parts
            let src  = src.add(n);
            let dest = dest.add(n);
            memcpy(dest, src, overhang);
        }

        // Check if we copied everything
        if n == 0 {
            return dest;
        }

        // At this point there is no longer any overlap that matters, just
        // fall through and copy remaining parts
    }

    // Plain forwards copy
    memcpy(dest, src, n);

    dest
}

/// libc `memset` implementation in Rust
///
/// # Parameters
///
/// * `s` - Pointer to memory to set
/// * `c` - Character to set bytes to
/// * `n` - Number of bytes to set
///
/// # Returns
///
/// Original pointer to `s`
///
#[no_mangle]
#[cfg(target_arch = "x86_64")]
unsafe extern fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    asm!("rep stosb",
         inout("rcx") n => _,
         inout("rdi") s => _,
         in("eax")    c as u32,
         options(nostack));

    s
}

/// libc `memset` implementation in Rust
///
/// # Parameters
///
/// * `s` - Pointer to memory to set
/// * `c` - Character to set bytes to
/// * `n` - Number of bytes to set
///
/// # Returns
///
/// Original pointer to `s`
///
#[no_mangle]
#[cfg(not(target_arch = "x86_64"))]
unsafe extern fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    let mut ii = 0;

    while ii < n {
        let s = s.add(ii);
        core::ptr::write(s, c as u8);
        ii += 1;
    }
    
    s
}

/// libc `memcmp` implementation in Rust
/// 
/// # Parameters
///
/// * `s1` - Pointer to memory to compare with s2
/// * `s2` - Pointer to memory to compare with s1
/// * `n`  - Number of bytes to compare
///
/// # Returns
/// 
/// The difference between the first unmatching byte of `s1` and `s2`, or
/// zero if both memory regions are identical
#[no_mangle]
unsafe extern fn memcmp(s1: *const u8, s2: *const u8, n: usize)
        -> i32 {
    let mut ii = 0;

    while ii < n {
        let a = core::ptr::read(s1.add(ii));
        let b = core::ptr::read(s2.add(ii));
        if a != b {
            return (a as i32).wrapping_sub(b as i32);
        }
        ii = ii.wrapping_add(1);
    }

    0
}
