use dyn_clone::DynClone;
use std::fmt::{self, Display};
use std::sync::{Arc, Mutex};

struct Log {
    id: u64,
    events: Arc<Mutex<Vec<String>>>,
}

impl Clone for Log {
    fn clone(&self) -> Self {
        Log {
            id: self.id + 1,
            events: self.events.clone(),
        }
    }
}

impl Display for Log {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "id={}", self.id)
    }
}

impl Drop for Log {
    fn drop(&mut self) {
        self.events
            .lock()
            .unwrap()
            .push(format!("dropping {}", self));
    }
}

#[test]
fn clone_sized() {
    let arc = Arc::new(0);
    assert_eq!(Arc::strong_count(&arc), 1);

    let c = dyn_clone::clone(&arc);
    assert_eq!(Arc::strong_count(&arc), 2);
    drop(c);
    assert_eq!(Arc::strong_count(&arc), 1);
}

#[test]
fn clone_trait_object() {
    trait MyTrait: Display + Sync + DynClone {}

    impl MyTrait for Log {}

    let events = Arc::new(Mutex::new(Vec::new()));
    let mut expected = Vec::new();
    {
        let b11: Box<dyn MyTrait> = Box::new(Log {
            id: 11,
            events: events.clone(),
        });
        let b12 = dyn_clone::clone_box(&*b11);
        assert_eq!(b11.to_string(), "id=11");
        assert_eq!(b12.to_string(), "id=12");
        expected.push("dropping id=12".to_owned());
        expected.push("dropping id=11".to_owned());
    }
    assert_eq!(*events.lock().unwrap(), expected);
}
