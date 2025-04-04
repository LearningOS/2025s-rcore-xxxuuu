
use alloc::{
    collections::{btree_map::BTreeMap, BTreeSet},
    vec::Vec,
};

/// Lock dependency graph
#[derive(Clone)]
pub struct LockGraph<T, L> {
    /// resources maintains (lock_type, lock_id) -> (resource number, tid sets)
    resources: BTreeMap<(LockType, L), (usize, Vec<T>)>,
    /// processes maintains (tid) -> (lock_type, lock_id)
    processes: BTreeMap<T, Vec<(LockType, L)>>,
}

/// Lock type
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LockType {
    /// Mutex
    Mutex = 0,
    /// Semaphore
    Semaphore = 1,
}

impl<T, L> LockGraph<T, L>
where
    T: Ord + Clone,
    L: Ord + Clone,
{
    /// Create a new lock graph
    pub fn new() -> Self {
        Self {
            resources: BTreeMap::new(),
            processes: BTreeMap::new(),
        }
    }

    /// add a dependency to the lock graph
    /// means the tid is waiting for lock to be released
    pub fn add_dependency(&mut self, tid: T, lock_type: LockType, lock_id: L) {
        self.processes
            .entry(tid)
            .or_insert(Vec::new())
            .push((lock_type, lock_id));
    }

    /// remove a dependency from the lock graph
    /// means the tid acquired the lock, so tid no longer depends it
    pub fn remove_dependency(&mut self, tid: T, lock_type: LockType, lock_id: L) {
        self.processes.entry(tid).and_modify(|deps| {
            if let Some(idx) = deps.iter().position(|d| d.1 == lock_id && d.0 == lock_type) {
                deps.remove(idx);
            }
        });
    }

    /// allocate a lock
    pub fn allocate_lock(&mut self, lock_type: LockType, lock_id: L, res_max_count: usize, tid: T) {
        self.resources
            .entry((lock_type, lock_id))
            .or_insert((res_max_count, Vec::new()))
            .1
            .push(tid);
    }

    /// release a lock
    pub fn release_lock(&mut self, lock_type: LockType, lock_id: L, tid: T) {
        self.resources
            .entry((lock_type, lock_id))
            .and_modify(|(_, tids)| {
                if let Some(idx) = tids.iter().position(|id| *id == tid) {
                    tids.remove(idx);
                }
            });
    }

    /// check if the lock graph has a cycle, is other words, there is a deadlock
    pub fn has_cycle(&self, lock_type: LockType, lock_id: L) -> bool {
        let mut resource_visited = BTreeSet::new();
        let mut process_visited = BTreeSet::new();
        self.dfs_check_cycle_with_resource(
            (lock_type, lock_id),
            &mut resource_visited,
            &mut process_visited,
        )
    }

    fn dfs_check_cycle_with_resource(
        &self,
        lock: (LockType, L),
        resource_visited: &mut BTreeSet<(LockType, L)>,
        process_visited: &mut BTreeSet<T>,
    ) -> bool {
        if resource_visited.contains(&lock) {
            return true;
        }
        resource_visited.insert(lock.clone());
        let Some((resource_num, owners)) = self.resources.get(&lock) else {
            return false;
        };
        let mut owners = owners.clone();
        owners.sort();
        owners.dedup();
        for owner in owners.iter() {
            if self.dfs_check_cycle_with_process(owner.clone(), resource_visited, process_visited) {
                return owners.len() >= *resource_num;
            }
        }
        resource_visited.remove(&lock);
        false
    }

    fn dfs_check_cycle_with_process(
        &self,
        tid: T,
        resource_visited: &mut BTreeSet<(LockType, L)>,
        process_visited: &mut BTreeSet<T>,
    ) -> bool {
        if process_visited.contains(&tid) {
            return true;
        }
        process_visited.insert(tid.clone());
        let Some(locks) = self.processes.get(&tid) else {
            return false;
        };
        for lock in locks.iter() {
            if self.dfs_check_cycle_with_resource(
                (lock.0, lock.1.clone()),
                resource_visited,
                process_visited,
            ) {
                return true;
            }
        }
        process_visited.remove(&tid);
        false
    }
}
