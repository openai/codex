use x11rb_protocol::protocol::xproto::EventMask;

#[test]
fn test_conversion() {
    assert_eq!(0, u32::from(EventMask::NO_EVENT));
    assert_eq!(1, u32::from(EventMask::KEY_PRESS));
    assert_eq!(4, u32::from(EventMask::BUTTON_PRESS));
    assert_eq!(Some(16u32), EventMask::ENTER_WINDOW.into());
    assert_eq!(EventMask::NO_EVENT, 0u8.into());
    assert_eq!(EventMask::KEY_PRESS, 1u8.into());
}

#[test]
fn test_bit_or() {
    assert_eq!(
        EventMask::KEY_PRESS,
        EventMask::KEY_PRESS | EventMask::NO_EVENT
    );
    assert_eq!(EventMask::KEY_PRESS, 1 | EventMask::NO_EVENT);
    assert_eq!(EventMask::KEY_PRESS, EventMask::NO_EVENT | 1);

    let mut mask = EventMask::KEY_PRESS;
    mask |= EventMask::BUTTON_PRESS;
    assert_eq!(5, u32::from(mask));

    let mut mask = EventMask::KEY_PRESS;
    mask |= 4u32;
    assert_eq!(5, u32::from(mask));

    let mut mask = 1u32;
    mask |= EventMask::BUTTON_PRESS;
    assert_eq!(5, mask);
}

#[test]
fn test_bit_and() {
    assert_eq!(
        EventMask::NO_EVENT,
        EventMask::KEY_PRESS & EventMask::NO_EVENT
    );
    assert_eq!(
        EventMask::KEY_PRESS,
        EventMask::KEY_PRESS & EventMask::KEY_PRESS
    );
    assert_eq!(
        EventMask::KEY_PRESS,
        EventMask::from(5u32) & EventMask::KEY_PRESS
    );
    assert_eq!(EventMask::KEY_PRESS, 5 & EventMask::KEY_PRESS);
    assert_eq!(EventMask::KEY_PRESS, EventMask::KEY_PRESS & 5);

    let mut mask = EventMask::from(5u32);
    mask &= EventMask::BUTTON_PRESS;
    assert_eq!(EventMask::BUTTON_PRESS, mask);

    let mut mask = EventMask::from(5u32);
    mask &= 4u32;
    assert_eq!(EventMask::BUTTON_PRESS, mask);

    let mut mask = 7u32;
    mask &= EventMask::from(21u32);
    assert_eq!(5, mask);
}

#[test]
fn test_contains() {
    let mask = EventMask::KEY_PRESS;
    assert!(mask.contains(EventMask::KEY_PRESS));
    assert!(mask.contains(EventMask::NO_EVENT));
    assert!(!mask.contains(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS));
    assert!(!mask.contains(EventMask::BUTTON_PRESS));

    let mask = EventMask::KEY_PRESS | EventMask::BUTTON_PRESS;
    assert!(mask.contains(EventMask::KEY_PRESS));
    assert!(mask.contains(EventMask::BUTTON_PRESS));
    assert!(mask.contains(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS));
    assert!(!mask.contains(EventMask::ENTER_WINDOW));
    assert!(!mask.contains(EventMask::ENTER_WINDOW | EventMask::BUTTON_PRESS));

    assert!(mask.contains(1u32));
    assert!(mask.contains(4u32));
    assert!(mask.contains(5u32));
    assert!(!mask.contains(16u32));
    assert!(!mask.contains(20u32));
}

#[test]
fn test_intersects() {
    let mask = EventMask::KEY_PRESS;
    assert!(mask.intersects(EventMask::KEY_PRESS));
    assert!(!mask.intersects(EventMask::NO_EVENT));
    assert!(mask.intersects(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS));
    assert!(!mask.intersects(EventMask::BUTTON_PRESS));

    let mask = EventMask::KEY_PRESS | EventMask::BUTTON_PRESS;
    assert!(mask.intersects(EventMask::KEY_PRESS));
    assert!(mask.intersects(EventMask::BUTTON_PRESS));
    assert!(mask.intersects(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS));
    assert!(!mask.intersects(EventMask::ENTER_WINDOW));
    assert!(mask.intersects(EventMask::ENTER_WINDOW | EventMask::BUTTON_PRESS));

    assert!(mask.intersects(1u32));
    assert!(mask.intersects(4u32));
    assert!(mask.intersects(5u32));
    assert!(!mask.intersects(16u32));
    assert!(mask.intersects(20u32));
}

#[test]
fn test_remove() {
    let no_event = EventMask::NO_EVENT;
    let key_press = EventMask::KEY_PRESS;
    let button_press = EventMask::BUTTON_PRESS;
    let exposure = EventMask::EXPOSURE;
    let key_and_button_press = key_press | button_press;

    assert_eq!(no_event, key_press.remove(key_press));
    assert_eq!(key_press, key_press.remove(no_event));
    assert_eq!(key_press, key_press.remove(button_press));
    assert_eq!(key_press, key_and_button_press.remove(button_press));
    assert_eq!(key_press, key_and_button_press.remove(button_press));
    assert_eq!(key_and_button_press, key_and_button_press.remove(exposure));

    assert_eq!(no_event, key_press.remove(1u32));
    assert_eq!(key_press, key_press.remove(0u32));
    assert_eq!(key_press, key_press.remove(4u32));
    assert_eq!(key_press, key_and_button_press.remove(4u32));
    assert_eq!(
        key_and_button_press,
        key_and_button_press.remove(1u32 << 15)
    );
}
