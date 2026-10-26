//! Resource accounting for the chapter 8 deadlock detector.
//! AI assistance: OpenAI Codex. Reference: rCore Guide chapter 8 exercise.
use alloc::{collections::VecDeque, vec, vec::Vec};

#[derive(Default)]
/// Available, held, and pending units for one family of resources.
pub struct ResourceTracker {
    available: Vec<usize>,
    allocation: Vec<Vec<usize>>,
    need: Vec<Vec<usize>>,
    waiters: Vec<VecDeque<usize>>,
    handoff: Vec<bool>,
}
impl ResourceTracker {
    /// Register a resource and its initial number of units.
    pub fn create(&mut self, id: usize, count: usize, handoff: bool) {
        while self.available.len() <= id {
            self.available.push(0);
            self.waiters.push(VecDeque::new());
            self.handoff.push(false);
            for row in &mut self.allocation {
                row.push(0);
            }
            for row in &mut self.need {
                row.push(0);
            }
        }
        self.available[id] = count;
        self.handoff[id] = handoff;
    }
    fn thread(&mut self, tid: usize) {
        while self.allocation.len() <= tid {
            self.allocation.push(vec![0; self.available.len()]);
            self.need.push(vec![0; self.available.len()]);
        }
    }
    fn safe(&self) -> bool {
        let mut work = self.available.clone();
        let mut finish = vec![false; self.allocation.len()];
        loop {
            let mut progress = false;
            for tid in 0..finish.len() {
                if !finish[tid]
                    && self.need[tid]
                        .iter()
                        .zip(&work)
                        .all(|(need, free)| need <= free)
                {
                    for (free, held) in work.iter_mut().zip(&self.allocation[tid]) {
                        *free += held;
                    }
                    finish[tid] = true;
                    progress = true;
                }
            }
            if !progress {
                return finish.iter().all(|done| *done);
            }
        }
    }
    /// Tentatively request one unit and reject unsafe waits when enabled.
    pub fn request(&mut self, tid: usize, id: usize, enabled: bool) -> bool {
        self.thread(tid);
        self.need[tid][id] += 1;
        if enabled && !self.safe() {
            self.need[tid][id] -= 1;
            return false;
        }
        if self.available[id] > 0 {
            self.available[id] -= 1;
            self.need[tid][id] -= 1;
            self.allocation[tid][id] += 1;
        } else if self.handoff[id] {
            self.waiters[id].push_back(tid);
        }
        true
    }
    /// Finish accounting for a spin mutex after it actually obtains the lock.
    pub fn acquired(&mut self, tid: usize, id: usize) {
        // Blocking primitives transfer ownership at wakeup. Spin mutexes retry.
        if !self.handoff[id] && self.need[tid][id] > 0 {
            self.need[tid][id] -= 1;
            self.available[id] -= 1;
            self.allocation[tid][id] += 1;
        }
    }
    /// Return a unit, transferring it directly to the first blocked waiter.
    pub fn release(&mut self, tid: usize, id: usize) {
        self.thread(tid);
        if self.allocation[tid][id] > 0 {
            self.allocation[tid][id] -= 1;
        }
        if self.handoff[id] {
            if let Some(next) = self.waiters[id].pop_front() {
                self.need[next][id] -= 1;
                self.allocation[next][id] += 1;
                return;
            }
        }
        self.available[id] += 1;
    }
}

#[derive(Default)]
/// Process-local switch and independent mutex/semaphore resource models.
pub struct DeadlockDetector {
    /// Whether an unsafe acquisition should be rejected.
    pub enabled: bool,
    /// Accounting for mutexes.
    pub mutex: ResourceTracker,
    /// Accounting for counting semaphores.
    pub semaphore: ResourceTracker,
}

#[cfg(test)]
mod tests {
    use super::ResourceTracker;
    #[test]
    fn rejected_self_lock_does_not_change_resource_state() {
        let mut r = ResourceTracker::default();
        r.create(0, 1, true);
        assert!(r.request(0, 0, true));
        assert!(!r.request(0, 0, true));
        r.release(0, 0);
        assert!(r.request(0, 0, true));
    }
    #[test]
    fn cycle_is_rejected_but_safe_wait_is_allowed() {
        let mut r = ResourceTracker::default();
        r.create(0, 1, true);
        r.create(1, 1, true);
        assert!(r.request(0, 0, true));
        assert!(r.request(1, 1, true));
        assert!(r.request(0, 1, true));
        assert!(!r.request(1, 0, true));
        r.release(1, 1);
        r.release(0, 1);
        r.release(0, 0);
        assert!(r.request(1, 0, true));
    }
    #[test]
    fn wakeup_reserves_resource_for_the_first_waiter() {
        let mut r = ResourceTracker::default();
        r.create(0, 1, true);
        assert!(r.request(0, 0, true));
        assert!(r.request(1, 0, true));
        r.release(0, 0);
        assert!(r.request(2, 0, true));
        assert!(!r.request(1, 0, true));
        r.release(1, 0);
        assert!(!r.request(2, 0, true));
    }
    #[test]
    fn multi_instance_semaphore_detects_circular_wait() {
        let mut r = ResourceTracker::default();
        r.create(0, 2, true);
        assert!(r.request(0, 0, true));
        assert!(r.request(1, 0, true));
        assert!(r.request(0, 0, true));
        assert!(!r.request(1, 0, true));
        r.release(1, 0);
        r.release(0, 0);
        r.release(0, 0);
        assert!(r.request(1, 0, true));
    }
    #[test]
    fn enabling_detection_after_acquisition_keeps_existing_holdings() {
        let mut r = ResourceTracker::default();
        r.create(0, 1, true);
        assert!(r.request(0, 0, false));
        assert!(!r.request(0, 0, true));
        r.release(0, 0);
        assert!(r.request(1, 0, true));
    }
    #[test]
    fn spin_waiter_accounts_only_after_actually_acquiring() {
        let mut r = ResourceTracker::default();
        r.create(0, 1, false);
        assert!(r.request(0, 0, true));
        assert!(r.request(1, 0, true));
        r.release(0, 0);
        assert!(r.request(2, 0, true));
        r.release(2, 0);
        r.acquired(1, 0);
        assert!(!r.request(1, 0, true));
    }
}
