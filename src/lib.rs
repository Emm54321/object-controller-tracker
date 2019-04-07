#![deny(missing_docs)]

//! A crate used to keep track of `Object-Controller` pairs, where a
//! `Controller` is used to operate on the `Object` (e.g. to cancel it),
//! usually from an other thread.
//!
//! A [Tracker](struct.Tracker.html) object is constructed and used to
//! register `Object-Controller` pairs. When an `Object` is dropped, the
//! `Controller` is automatically unregistered. The
//! [Tracker](struct.Tracker.html) object can be used to operate on
//! all registered objects through their associated controllers.
//!
//!
//! # Example
//!
//! ```no_run
//! # extern crate object_controller_tracker;
//! # use std::sync::atomic::{AtomicBool, Ordering};
//! # use std::sync::Arc;
//! # use std::time::Duration;
//! # use object_controller_tracker::*;
//!
//! // The object we want to track...
//! struct Object {
//!     stop: Arc<AtomicBool>,
//!     //...
//! }
//!
//! // ... and its associated controller.
//! struct Controller(Arc<AtomicBool>);
//!
//! impl Object {
//!
//!     // Create an Object-Controller pair.
//!     fn new2() -> (Object, Controller) {
//!         let stop = Arc::new(AtomicBool::new(false));
//!         let ctl_stop = Arc::clone(&stop);
//!         (Object { stop }, Controller(ctl_stop))
//!     }
//!
//!     // Some method on the object that can be cancelled through the Controller.
//!     fn do_something(&self) {
//!         while !self.stop.load(Ordering::SeqCst) {
//!             println!("Do something.");
//!             std::thread::sleep(Duration::from_secs(1));
//!         }
//!     }
//! }
//!
//! impl Controller {
//!     // Cancel an operation on the Object.
//!     fn cancel(&self) {
//!         self.0.store(true, Ordering::SeqCst);
//!     }
//! }
//!
//! fn main() {
//!     // Create the tracker object.
//!     let mut tracker = Tracker::new();
//!
//!     let tracker2 = tracker.clone();
//!     let thread = std::thread::spawn(move || {
//!         // Create an Object-Controller pair in some thread.
//!         let (object, controller) = Object::new2();
//!
//!         // Register it with the tracker.
//!         let object = tracker2.track(object, controller);
//!
//!         // Do some work with the Object.
//!         object.do_something();
//!     });
//!
//!     std::thread::sleep(Duration::from_secs(5));
//!
//!     // Cancel all registered Object operations.
//!     tracker.for_each(|r| r.cancel());
//!
//!     thread.join().unwrap();
//! }
//! ```

use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

type Id = u32;

struct InnerTracker<C> {
    controllers: HashMap<Id, C>,
    next_id: Id,
}

/// An object used to keep track of controller parts of object-controller pairs.
pub struct Tracker<C>(Arc<Mutex<InnerTracker<C>>>);

/// Wrapper for the object part of tracked object-controller pair.
///
/// When this object is dropped, the associated controller is unregistered.
pub struct Tracked<T, C> {
    tracker: Tracker<C>,
    id: Id,
    object: T,
}

/// An RAII implementation of a Tracker lock. When this structure goes out
/// of scope, the lock is released.
///
/// This structure is created by the [lock](struct.Tracker.html#method.lock) method
/// on [Tracker](struct.Tracker.html).
pub struct TrackerGuard<'a, C>(MutexGuard<'a, InnerTracker<C>>);

/// An iterator over the tracked controllers. Controllers are visited in an
/// unspecified order.
pub struct Iter<'a, C>(std::collections::hash_map::Values<'a, Id, C>);

impl<C> InnerTracker<C> {
    fn register(&mut self, controller: C) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        self.controllers.insert(id, controller);
        id
    }

    fn unregister(&mut self, id: Id) {
        self.controllers.remove(&id);
    }
}

impl<C> Tracker<C> {
    /// Create a new tracker.
    pub fn new() -> Tracker<C> {
        Tracker(Arc::new(Mutex::new(InnerTracker {
            controllers: HashMap::new(),
            next_id: 0,
        })))
    }

    /// Register an object-controller pair.
    ///
    /// The controller part is kept in the [Tracker](struct.Tracker.html),
    /// and the object part is wrapped in a [Tracked](struct.Tracked.html).
    /// When the [Tracked](struct.Tracked.html) object is dropped, the
    /// controller is dropped too.
    pub fn track<T>(&self, object: T, controller: C) -> Tracked<T, C> {
        let mut tracker = self.0.lock().unwrap();
        let id = tracker.register(controller);
        Tracked {
            tracker: Tracker(self.0.clone()),
            id,
            object,
        }
    }

    /// Register an object-contoller pair.
    ///
    /// Same as [track](struct.Tracker.html#method.track)(pair.0, pair.1).
    pub fn track_pair<T>(&self, pair: (T, C)) -> Tracked<T, C> {
        self.track(pair.0, pair.1)
    }

    /// Lock the tracker so that one can iterate over its controllers.
    pub fn lock(&mut self) -> TrackerGuard<C> {
        TrackerGuard(self.0.lock().unwrap())
    }

    /// Call a closure on each tracked controller. The closure must not
    /// register a new controller in this tracker, or drop a tracked
    /// object. The controllers are visited in an unspecified order.
    pub fn for_each<F: FnMut(&C)>(&mut self, f: F) {
        self.lock().iter().for_each(f);
    }
}

impl<C> Clone for Tracker<C> {
    fn clone(&self) -> Tracker<C> {
        Tracker(Arc::clone(&self.0))
    }
}

impl<C> Default for Tracker<C> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, C> Drop for Tracked<T, C> {
    fn drop(&mut self) {
        let mut tracker = self.tracker.0.lock().unwrap();
        tracker.unregister(self.id);
    }
}

impl<T, C> Deref for Tracked<T, C> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.object
    }
}

impl<T, C> DerefMut for Tracked<T, C> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.object
    }
}

impl<'a, C> TrackerGuard<'a, C> {
    /// Create an iterator over tracked controllers.
    pub fn iter(&'a self) -> Iter<'a, C> {
        Iter(self.0.controllers.values())
    }
}

impl<'a, C> Iterator for Iter<'a, C> {
    type Item = &'a C;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test1() {
        let mut tracker = Tracker::new();
        let obj = tracker.track((), 5);
        tracker.lock().iter().for_each(|c| {
            assert_eq!(c, &5);
        });
        assert_eq!(tracker.lock().iter().count(), 1);
        drop(obj);
        assert_eq!(tracker.lock().iter().count(), 0);
    }

    #[test]
    fn test2() {
        let mut tracker = Tracker::new();
        let obj1 = tracker.track((), 1);
        let obj2 = tracker.track((), 2);
        let obj3 = tracker.track((), 3);
        assert_eq!(tracker.lock().iter().count(), 3);
        let mut sum = 0;
        tracker.for_each(|x| {
            sum += x;
        });
        assert_eq!(sum, 6);
        drop(obj2);
        sum = 0;
        tracker.for_each(|x| {
            sum += x;
        });
        assert_eq!(sum, 4);
        assert_eq!(tracker.lock().iter().count(), 2);
        drop(obj3);
        drop(obj1);
        sum = 0;
        tracker.for_each(|x| {
            sum += x;
        });
        assert_eq!(sum, 0);
    }
}
