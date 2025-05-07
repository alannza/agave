//! Utility to synchronize event notifications among asynchronous notifiers

use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex, RwLock},
};

#[derive(Debug, Default)]
pub struct EventNotificationSynchronizer {
    bank_slot_to_notification_tracker: RwLock<HashMap<u64, Arc<BankNotificationDependencyTracker>>>,
}

impl EventNotificationSynchronizer {
    pub fn create_bank_tracker(&self, bank_slot: u64) -> Arc<BankNotificationDependencyTracker> {
        let tracker: Arc<BankNotificationDependencyTracker> =
            Arc::new(BankNotificationDependencyTracker::new(bank_slot));
        let mut slot_to_bank_tracker = self.bank_slot_to_notification_tracker.write().unwrap();
        slot_to_bank_tracker.insert(bank_slot, tracker.clone());
        tracker.clone()
    }

    pub fn get_or_create_bank_tracker(
        &self,
        bank_slot: u64,
    ) -> Arc<BankNotificationDependencyTracker> {
        let slot_to_bank_tracker = self.bank_slot_to_notification_tracker.read().unwrap();
        let tracker = slot_to_bank_tracker.get(&bank_slot);
        match tracker {
            None => {
                drop(slot_to_bank_tracker); // release read lock
                self.create_bank_tracker(bank_slot)
            }
            Some(tracker) => tracker.clone(),
        }
    }
}

#[derive(Debug, Default)]
// pub struct EventNotificationSynchronizer {
pub struct BankNotificationDependencyTracker {
    pub bank_slot: u64,
    transaction_status_service_notified: Mutex<bool>,
    condvar: Condvar,
}

impl BankNotificationDependencyTracker {
    pub fn new(bank_slot: u64) -> Self {
        BankNotificationDependencyTracker {
            bank_slot,
            transaction_status_service_notified: Mutex::new(false),
            condvar: Condvar::default(),
        }
    }

    pub fn wait_for_unfinished_dependencies(&self) {
        let mut notified = self.transaction_status_service_notified.lock().unwrap();
        while *notified == true {
            notified = self.condvar.wait(notified).unwrap();
        }
    }

    pub fn mark_transaction_status_service_notified(&self) {
        {
            let mut notified = self.transaction_status_service_notified.lock().unwrap();
            *notified = true;
        }
        self.condvar.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{sync::Arc, thread},
    };

    #[test]
    fn test_create_bank_tracker() {
        let manager = EventNotificationSynchronizer::default();
        let tracker52 = manager.create_bank_tracker(52);
        let tracker37 = manager.create_bank_tracker(37);
        let tracker13 = manager.create_bank_tracker(13);

        // already exists, but that's ok
        let tracker52prime = manager.create_bank_tracker(52);

        assert_ne!(tracker52.bank_slot, tracker37.bank_slot);
        assert_ne!(tracker52.bank_slot, tracker13.bank_slot);

        assert_eq!(tracker52.bank_slot, tracker52prime.bank_slot);
    }

    #[test]
    fn test_get_or_create_bank_tracker() {
        let manager = EventNotificationSynchronizer::default();

        let tracker52 = manager.get_or_create_bank_tracker(52);
        let tracker37 = manager.get_or_create_bank_tracker(37);
        let tracker13 = manager.get_or_create_bank_tracker(13);
        let tracker52prime = manager.get_or_create_bank_tracker(52);

        assert_ne!(tracker52.bank_slot, tracker37.bank_slot);
        assert_ne!(tracker52.bank_slot, tracker13.bank_slot);

        assert_eq!(tracker52.bank_slot, tracker52prime.bank_slot);
    }

    #[test]
    fn test_wait_for_unfinished_dependencies() {
        let manager = Arc::new(EventNotificationSynchronizer::default());
        let manager_clone1 = Arc::clone(&manager);
        let manager_clone2 = Arc::clone(&manager);
        let manager_clone3 = Arc::clone(&manager);
        let manager_clone4 = Arc::clone(&manager);

        thread::scope(|s| {
            s.spawn(move || {
                let tracker = manager_clone1.get_or_create_bank_tracker(52);
                tracker.wait_for_unfinished_dependencies();
            });

            s.spawn(move || {
                let tracker = manager_clone2.get_or_create_bank_tracker(37);
                tracker.wait_for_unfinished_dependencies();
            });

            // make sure above threads are spawned first
            thread::sleep(std::time::Duration::from_millis(100));

            s.spawn(move || {
                let tracker = manager_clone3.get_or_create_bank_tracker(52);
                tracker.mark_transaction_status_service_notified();
            });

            s.spawn(move || {
                let tracker = manager_clone4.get_or_create_bank_tracker(37);
                tracker.mark_transaction_status_service_notified();
            });
        });
    }

    #[test]
    fn test_wait_for_unfinished_dependencies_reverse_order() {
        let manager = Arc::new(EventNotificationSynchronizer::default());
        let manager_clone1 = Arc::clone(&manager);
        let manager_clone2 = Arc::clone(&manager);
        let manager_clone3 = Arc::clone(&manager);
        let manager_clone4 = Arc::clone(&manager);

        thread::scope(|s| {
            s.spawn(move || {
                let tracker = manager_clone1.get_or_create_bank_tracker(52);
                tracker.wait_for_unfinished_dependencies();
            });

            s.spawn(move || {
                let tracker = manager_clone2.get_or_create_bank_tracker(37);
                tracker.wait_for_unfinished_dependencies();
            });

            // make sure above threads are spawned first
            thread::sleep(std::time::Duration::from_millis(100));

            s.spawn(move || {
                let tracker = manager_clone3.get_or_create_bank_tracker(52);
                tracker.mark_transaction_status_service_notified();
            });

            s.spawn(move || {
                let tracker = manager_clone4.get_or_create_bank_tracker(37);
                tracker.mark_transaction_status_service_notified();
            });
        });
    }
}
