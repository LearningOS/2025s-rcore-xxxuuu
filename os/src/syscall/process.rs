//! Process management syscalls
use core::mem::size_of;

use alloc::collections::vec_deque::VecDeque;

use crate::{
    mm::{translated_byte_buffer, MapPermission, PageTable, VirtAddr},
    task::{
        self, change_program_brk, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let buffers =
        translated_byte_buffer(current_user_token(), ts as *const u8, size_of::<TimeVal>());
    let mut sec = us / 1_000_000;
    let mut usec = us % 1_000_000;
    let mut time_buf = VecDeque::with_capacity(size_of::<TimeVal>());
    for _ in 0..size_of::<usize>() {
        time_buf.push_back((sec & 0xff) as u8);
        sec = sec >> 8;
    }
    for _ in 0..size_of::<usize>() {
        time_buf.push_back((usec & 0xff) as u8);
        usec = usec >> 8;
    }
    for table in buffers {
        for i in 0..table.len() {
            match time_buf.pop_front() {
                Some(val) => table[i] = val,
                None => return 0,
            }
        }
    }
    0
}

#[repr(usize)]
enum TraceRequest {
    ReadMem = 0,
    WriteMem = 1,
    QuerySyscallCount = 2,
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    if trace_request == TraceRequest::ReadMem as usize
        || trace_request == TraceRequest::WriteMem as usize
    {
        let addr = VirtAddr::from(id);
        let page_table = PageTable::from_token(current_user_token());
        let pte = page_table.translate(addr.floor());
        if pte.is_none() {
            return -1;
        }
        let pte = pte.unwrap();
        if !pte.is_valid() || !pte.is_user() {
            return -1;
        }
        if trace_request == TraceRequest::ReadMem as usize && pte.readable() {
            let value = pte.ppn().get_bytes_array()[addr.page_offset()];
            value as isize
        } else if trace_request == TraceRequest::WriteMem as usize && pte.writable() {
            let value = &mut pte.ppn().get_bytes_array()[addr.page_offset()];
            *value = data as u8;
            0
        } else {
            -1
        }
    } else if trace_request == TraceRequest::QuerySyscallCount as usize {
        let count = task::get_syscall_count(task::current_task_id(), id);
        count as isize
    } else {
        panic!("invalid trace request value: {}", trace_request)
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    let mut flag = MapPermission::U;
    if port & 0x7 == 0 || port & !0x7 != 0 {
        return -1;
    }
    if port & 0x1 != 0 {
        flag.insert(MapPermission::R);
    }
    if port & 0x2 != 0 {
        flag.insert(MapPermission::W);
    }
    if port & 0x4 != 0 {
        flag.insert(MapPermission::X);
    }
    let vaddr = VirtAddr::from(start);
    if !vaddr.aligned() {
        return -1;
    }
    // [start, end)
    if task::map_memory(vaddr, VirtAddr::from(start + len), flag) {
        0
    } else {
        -1
    }
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let vaddr = VirtAddr::from(start);
    if !vaddr.aligned() {
        return -1;
    }
    if task::unmap_memory(vaddr, VirtAddr::from(start + len)) {
        0
    } else {
        -1
    }
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
