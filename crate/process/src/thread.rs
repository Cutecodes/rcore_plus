//! Thread std-like interface
//!
//! Based on Processor. Used in kernel.
//!
//! You need to implement the following functions before use:
//! - `processor`: Get a reference of the current `Processor`
//! - `new_kernel_context`: Construct a `Context` of the new kernel thread

use alloc::boxed::Box;
use core::marker::PhantomData;
use core::time::Duration;
use log::*;
use crate::processor::*;
use crate::process_manager::*;

#[linkage = "weak"]
#[no_mangle]
/// Get a reference of the current `Processor`
fn processor() -> &'static Processor {
    unimplemented!("thread: Please implement and export `processor`")
}

#[linkage = "weak"]
#[no_mangle]
/// Construct a `Context` of the new kernel thread
fn new_kernel_context(_entry: extern fn(usize) -> !, _arg: usize) -> Box<Context> {
    unimplemented!("thread: Please implement and export `new_kernel_context`")
}


/// Gets a handle to the thread that invokes it.
pub fn current() -> Thread {
    Thread { pid: processor().pid() }
}

/// Puts the current thread to sleep for the specified amount of time.
pub fn sleep(dur: Duration) {
    let time = dur_to_ticks(dur);
    trace!("sleep: {:?} ticks", time);
    processor().manager().sleep(current().id(), time);
    park();

    fn dur_to_ticks(dur: Duration) -> usize {
        return dur.as_secs() as usize * 100 + dur.subsec_nanos() as usize / 10_000_000;
    }
}

/// Spawns a new thread, returning a JoinHandle for it.
///
/// `F`: Type of the function `f`
/// `T`: Type of the return value of `f`
pub fn spawn<F, T>(f: F) -> JoinHandle<T>
    where
        F: Send + 'static + FnOnce() -> T,
        T: Send + 'static,
{
    // 注意到下面的问题：
    // Processor只能从入口地址entry+参数arg创建新线程
    // 而我们现在需要让它执行一个未知类型的（闭包）函数f

    // 首先把函数本体（代码数据）置于堆空间中
    let f = Box::into_raw(Box::new(f));

    // 定义一个静态函数作为新线程的入口点
    // 其参数是函数f在堆上的指针
    // 这样我们就把函数f传到了一个静态函数内部
    //
    // 注意到它具有泛型参数，因此对每一次spawn调用，
    // 由于F类型是独特的，因此都会生成一个新的kernel_thread_entry
    extern fn kernel_thread_entry<F, T>(f: usize) -> !
        where
            F: Send + 'static + FnOnce() -> T,
            T: Send + 'static,
    {
        // 在静态函数内部：
        // 根据传进来的指针，恢复f
        let f = unsafe { Box::from_raw(f as *mut F) };
        // 调用f，并将其返回值也放在堆上
        let ret = Box::new(f());
        // 清理本地线程存储
        //   unsafe { LocalKey::<usize>::get_map() }.clear();
        // 让Processor退出当前线程
        // 把f返回值在堆上的指针，以线程返回码的形式传递出去
        let exit_code = Box::into_raw(ret) as usize;
        processor().manager().exit(current().id(), exit_code);
        processor().yield_now();
        // 再也不会被调度回来了
        unreachable!()
    }

    let context = new_kernel_context(kernel_thread_entry::<F, T>, f as usize);
    let pid = processor().manager().add(context, processor().pid());

    return JoinHandle {
        thread: Thread { pid },
        mark: PhantomData,
    };
}

/// Cooperatively gives up a timeslice to the OS scheduler.
pub fn yield_now() {
    trace!("yield:");
    processor().yield_now();
}

/// Blocks unless or until the current thread's token is made available.
pub fn park() {
    trace!("park:");
    processor().manager().sleep(current().id(), 0);
    processor().yield_now();
}

/// A handle to a thread.
pub struct Thread {
    pid: usize,
}

impl Thread {
    /// Atomically makes the handle's token available if it is not already.
    pub fn unpark(&self) {
        processor().manager().wakeup(self.pid);
    }
    /// Gets the thread's unique identifier.
    pub fn id(&self) -> usize {
        self.pid
    }
}

/// An owned permission to join on a thread (block on its termination).
pub struct JoinHandle<T> {
    thread: Thread,
    mark: PhantomData<T>,
}

impl<T> JoinHandle<T> {
    /// Extracts a handle to the underlying thread.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }
    /// Waits for the associated thread to finish.
    pub fn join(self) -> Result<T, ()> {
        loop {
            match processor().manager().get_status(self.thread.pid) {
                Some(Status::Exited(exit_code)) => {
                    processor().manager().remove(self.thread.pid);
                    // Find return value on the heap from the exit code.
                    return Ok(unsafe { *Box::from_raw(exit_code as *mut T) });
                }
                None => return Err(()),
                _ => (),
            }
            processor().manager().wait(current().id(), self.thread.pid);
            processor().yield_now();
        }
    }
    /// Force construct a JoinHandle struct
    pub unsafe fn _of(pid: Pid) -> JoinHandle<T> {
        JoinHandle {
            thread: Thread { pid },
            mark: PhantomData,
        }
    }
}