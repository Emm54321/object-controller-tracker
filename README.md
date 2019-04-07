# object-controller-tracker

A crate used to keep track of `Object-Controller` pairs, where a
`Controller` is used to operate on the `Object` (e.g. to cancel it),
usually from an other thread.

A [Tracker](struct.Tracker.html) object is constructed and used to
register `Object-Controller` pairs. When an `Object` is dropped, the
`Controller` is automatically unregistered. The
[Tracker](struct.Tracker.html) object can be used to operate on
all registered objects through their associated controllers.


## Example

```rust

// The object we want to track...
struct Object {
    stop: Arc<AtomicBool>,
    //...
}

// ... and its associated controller.
struct Controller(Arc<AtomicBool>);

impl Object {

    // Create an Object-Controller pair.
    fn new2() -> (Object, Controller) {
        let stop = Arc::new(AtomicBool::new(false));
        let ctl_stop = Arc::clone(&stop);
        (Object { stop }, Controller(ctl_stop))
    }

    // Some method on the object that can be cancelled through the Controller.
    fn do_something(&self) {
        while !self.stop.load(Ordering::SeqCst) {
            println!("Do something.");
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}

impl Controller {
    // Cancel an operation on the Object.
    fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

fn main() {
    // Create the tracker object.
    let mut tracker = Tracker::new();

    let tracker2 = tracker.clone();
    let thread = std::thread::spawn(move || {
        // Create an Object-Controller pair in some thread.
        let (object, controller) = Object::new2();

        // Register it with the tracker.
        let object = tracker2.track(object, controller);

        // Do some work with the Object.
        object.do_something();
    });

    std::thread::sleep(Duration::from_secs(5));

    // Cancel all registered Object operations.
    tracker.for_each(|r| r.cancel());

    thread.join().unwrap();
}
```

License: MIT/Apache2.0
