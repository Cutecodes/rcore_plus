#![feature(ptr_internals)]
#![feature(lang_items)]
#![feature(const_fn)]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(unboxed_closures)]
#![feature(naked_functions)]
#![feature(asm)]
#![feature(optin_builtin_traits)]
#![feature(panic_info_message)]
#![feature(global_asm)]
#![feature(compiler_builtins_lib)]
#![no_std]

extern crate alloc;
extern crate bit_field;
extern crate bitflags;
extern crate lazy_static;
extern crate linked_list_allocator;
#[macro_use]
extern crate log;
extern crate once;
extern crate spin;
extern crate volatile;
extern crate xmas_elf;
use linked_list_allocator::LockedHeap;

#[macro_use]    // print!
pub mod logging;

mod lang;

#[cfg(target_arch = "riscv32")]
#[path = "arch/riscv32/mod.rs"]
pub mod arch;

/// The entry point of Rust kernel
#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    println!("Hello World{}", "!");
    logging::init();
    arch::init();
    unsafe { arch::interrupt::enable(); }
    loop {}
}

/// Global heap allocator
/// Available after `memory::init()`.
/// It should be defined in memory mod, but in Rust `global_allocator` must be in root mod.
#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();