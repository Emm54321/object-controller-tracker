extern crate object_controller_tracker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use object_controller_tracker::*;

struct Object {
    id: u32,
    stop: Arc<AtomicBool>,
    //...
}

struct Controller {
    id: u32,
    stop: Arc<AtomicBool>,
}

impl Object {
    fn new2(id: u32) -> (Object, Controller) {
        let stop = Arc::new(AtomicBool::new(false));
        let remote_stop = Arc::clone(&stop);
        (
            Object { id, stop },
            Controller {
                id,
                stop: remote_stop,
            },
        )
    }

    fn do_something(&self) {
        for _ in 0..(self.id * 2) {
            if self.stop.load(Ordering::SeqCst) {
                break;
            }
            println!("Do something {}.", self.id);
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

impl Controller {
    fn cancel(&self) {
        println!("Cancel {}.", self.id);
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn main() {
    let mut tracker = Tracker::new();

    let mut threads = Vec::new();

    for i in 1..5 {
        let tracker2 = tracker.clone();
        let thread = std::thread::spawn(move || {
            let (object, controller) = Object::new2(i);
            let object = tracker2.track(object, controller);

            object.do_something();

            println!("Quit {}", i);
        });
        threads.push(thread);
    }

    std::thread::sleep(Duration::from_secs(5));

    tracker.for_each(|r| r.cancel());

    for thread in threads {
        thread.join().unwrap();
    }
}
