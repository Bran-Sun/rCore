use memory::MemoryController;
use spin::{Once, Mutex};
use core::slice;
use alloc::String;

use self::process::*;
use self::processor::*;

mod process;
mod processor;

/// 平台相关依赖：struct TrapFrame
///
/// ## 必须实现的特性
///
/// * Debug: 用于Debug输出
use arch::interrupt::TrapFrame;

pub fn init(mut mc: MemoryController) {
    PROCESSOR.call_once(|| {Mutex::new({
        let initproc = Process::new_init(&mut mc);
        let idleproc = Process::new("idle", idle_thread, &mut mc);
        let mut processor = Processor::new();
        processor.add(initproc);
        processor.add(idleproc);
        processor
    })});
    MC.call_once(|| Mutex::new(mc));
}

static PROCESSOR: Once<Mutex<Processor>> = Once::new();
static MC: Once<Mutex<MemoryController>> = Once::new();

extern fn idle_thread() {
    loop {
        println!("idle ...");
        let mut i = 0;
        while i < 1 << 22 {
            i += 1;
        }
    }
}

/// Fork the current process. Return the child's PID.
pub fn sys_fork(tf: &TrapFrame) -> i32 {
    let mut processor = PROCESSOR.try().unwrap().lock();
    let mut mc = MC.try().unwrap().lock();
    let new = processor.current().fork(tf, &mut mc);
    let pid = processor.add(new);
    info!("fork: {} -> {}", processor.current().pid, pid);
    pid as i32
}

/// Wait the process exit.
/// Return the PID. Store exit code to `code` if it's not null.
pub fn sys_wait(rsp: &mut usize, pid: usize, code: *mut i32) -> i32 {
    let mut processor = PROCESSOR.try().unwrap().lock();
    let target = match pid {
        0 => WaitTarget::AnyChild,
        _ => WaitTarget::Proc(pid),
    };
    match processor.current_wait_for(target) {
        WaitResult::Ok(pid, error_code) => {
            if !code.is_null() {
                unsafe { *code = error_code as i32 };
            }
            0 // pid as i32
        },
        WaitResult::Blocked => {
            processor.schedule(rsp);
            0 /* unused */
        },
        WaitResult::NotExist => -1,
    }
}

pub fn sys_yield(rsp: &mut usize) -> i32 {
    info!("yield:");
    let mut processor = PROCESSOR.try().unwrap().lock();
    processor.schedule(rsp);
    0
}

/// Kill the process
pub fn sys_kill(pid: usize) -> i32 {
    PROCESSOR.try().unwrap().lock().kill(pid);
    0
}

/// Get the current process id
pub fn sys_getpid() -> i32 {
    PROCESSOR.try().unwrap().lock().current().pid as i32
}

/// Exit the current process
pub fn sys_exit(rsp: &mut usize, error_code: ErrorCode) -> i32 {
    let mut processor = PROCESSOR.try().unwrap().lock();
    let pid = processor.current().pid;
    processor.schedule(rsp);
    processor.exit(pid, error_code);
    0
}

pub fn sys_sleep(rsp: &mut usize, time: usize) -> i32 {
    info!("sleep: {} ticks", time);
    let mut processor = PROCESSOR.try().unwrap().lock();
    let pid = processor.current().pid;
    processor.schedule(rsp);
    processor.sleep(pid, time);
    0
}

pub fn sys_get_time() -> i32 {
    let processor = PROCESSOR.try().unwrap().lock();
    processor.get_time() as i32
}

pub fn timer_handler(tf: &TrapFrame, rsp: &mut usize) {
    let mut processor = PROCESSOR.try().unwrap().lock();
    processor.tick(rsp);
}

pub fn add_user_process(name: impl AsRef<str>, data: &[u8]) {
    let mut processor = PROCESSOR.try().unwrap().lock();
    let mut mc = MC.try().unwrap().lock();
    let mut new = Process::new_user(data, &mut mc);
    new.name = String::from(name.as_ref());
    processor.add(new);
}

pub fn print() {
    debug!("{:#x?}", *PROCESSOR.try().unwrap().lock());
}