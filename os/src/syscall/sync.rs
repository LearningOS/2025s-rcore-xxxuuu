use crate::sync::{Condvar, LockType, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, TaskStatus};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    trace!("kernel:pid[{}] tid[{}] sys_mutex_lock", pid, tid);

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let deadlock_detection = process_inner.deadlock_detection_enabled;

    if deadlock_detection {
        process_inner
            .lock_graph
            .add_dependency(tid, LockType::Mutex, mutex_id);
        if process_inner
            .lock_graph
            .has_cycle(LockType::Mutex, mutex_id)
        {
            process_inner
                .lock_graph
                .remove_dependency(tid, LockType::Mutex, mutex_id);
            return -0xDEAD;
        }
    }

    drop(process_inner);
    mutex.lock();
    if deadlock_detection {
        let mut process_inner = process.inner_exclusive_access();
        process_inner
            .lock_graph
            .remove_dependency(tid, LockType::Mutex, mutex_id);
        process_inner
            .lock_graph
            .allocate_lock(LockType::Mutex, mutex_id, 1, tid);
    }
    0
}

/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    trace!("kernel:pid[{}] tid[{}] sys_mutex_unlock", pid, tid);

    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    let deadlock_detection = process_inner.deadlock_detection_enabled;
    drop(process_inner);

    mutex.unlock();
    if deadlock_detection {
        let mut process_inner = process.inner_exclusive_access();
        process_inner
            .lock_graph
            .release_lock(LockType::Mutex, mutex_id, tid);
    }
    0
}

/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}

/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    trace!("kernel:pid[{}] tid[{}] sys_semaphore_up", pid, tid);

    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    let deadlock_detection = process_inner.deadlock_detection_enabled;
    drop(process_inner);

    sem.up();
    if deadlock_detection {
        let mut process_inner = process.inner_exclusive_access();
        process_inner
            .lock_graph
            .release_lock(LockType::Semaphore, sem_id, tid);
    }
    0
}

/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let task = current_task().unwrap();
    let pid = task.process.upgrade().unwrap().getpid();
    let tid = task.inner_exclusive_access().res.as_ref().unwrap().tid;
    trace!("kernel:pid[{}] tid[{}] sys_semaphore_down", pid, tid);

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    let deadlock_detection = process_inner.deadlock_detection_enabled;
    let sem_cnt = sem.get_count();
    let sem_max_count = sem.get_max_count();
    if deadlock_detection && sem_cnt <= 0 {
        process_inner
            .lock_graph
            .add_dependency(tid, LockType::Semaphore, sem_id);
        let has_cycle = process_inner
            .lock_graph
            .has_cycle(LockType::Semaphore, sem_id);
        let ready_tasks = process_inner
            .tasks
            .iter()
            .filter_map(|task| task.as_ref())
            .filter(|t| t.inner_exclusive_access().task_status == TaskStatus::Ready)
            .count();
        if has_cycle && ready_tasks <= 1 {
            process_inner
                .lock_graph
                .remove_dependency(tid, LockType::Semaphore, sem_id);
            return -0xDEAD;
        }
    }

    drop(process_inner);
    sem.down();
    if deadlock_detection {
        let mut process_inner = process.inner_exclusive_access();
        process_inner
            .lock_graph
            .remove_dependency(tid, LockType::Semaphore, sem_id);
        process_inner
            .lock_graph
            .allocate_lock(LockType::Semaphore, sem_id, sem_max_count, tid);
    }
    0
}

/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}

/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.deadlock_detection_enabled = enabled == 1;
    0
}
