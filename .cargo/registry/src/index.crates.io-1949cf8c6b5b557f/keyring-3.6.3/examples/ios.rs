use keyring::{Entry, Error};

#[no_mangle]
extern "C" fn test() {
    test_invalid_parameter();
    test_empty_keyring();
    test_empty_password_input();
    test_round_trip_ascii_password();
    test_round_trip_non_ascii_password();
    test_update_password();
    #[cfg(target_os = "ios")]
    test_get_credential();
}

fn test_invalid_parameter() {
    let entry = Entry::new("", "user");
    assert!(
        matches!(entry, Err(Error::Invalid(_, _))),
        "Created entry with empty service"
    );
    let entry = Entry::new("service", "");
    assert!(
        matches!(entry, Err(Error::Invalid(_, _))),
        "Created entry with empty user"
    );
    let entry = Entry::new_with_target("test", "service", "user");
    assert!(
        matches!(entry, Err(Error::Invalid(_, _))),
        "Created entry with non-default target"
    );
}

fn test_empty_keyring() {
    let name = "test_empty_keyring".to_string();
    let entry = Entry::new(&name, &name).expect("Failed to create entry");
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)))
}

fn test_empty_password_input() {
    let name = "test_empty_password_input".to_string();
    let entry = Entry::new(&name, &name).expect("Failed to create entry");
    let in_pass = "";
    entry
        .set_password(in_pass)
        .expect("Couldn't set empty password");
    let out_pass = entry.get_password().expect("Couldn't get empty password");
    assert_eq!(in_pass, out_pass);
    entry
        .delete_credential()
        .expect("Couldn't delete credential with empty password");
    assert!(
        matches!(entry.get_password(), Err(Error::NoEntry)),
        "Able to read a deleted password"
    )
}

fn test_round_trip_ascii_password() {
    let name = "test_round_trip_ascii_password".to_string();
    let entry = Entry::new(&name, &name).expect("Failed to create entry");
    let password = "test ascii password";
    entry.set_password(password).unwrap();
    let stored_password = entry.get_password().unwrap();
    assert_eq!(stored_password, password);
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)))
}

fn test_round_trip_non_ascii_password() {
    let name = "test_round_trip_non_ascii_password".to_string();
    let entry = Entry::new(&name, &name).expect("Failed to create entry");
    let password = "このきれいな花は桜です";
    entry.set_password(password).unwrap();
    let stored_password = entry.get_password().unwrap();
    assert_eq!(stored_password, password);
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)))
}

fn test_update_password() {
    let name = "test_update_password".to_string();
    let entry = Entry::new(&name, &name).expect("Failed to create entry");
    let password = "test ascii password";
    entry.set_password(password).unwrap();
    let stored_password = entry.get_password().unwrap();
    assert_eq!(stored_password, password);
    let password = "このきれいな花は桜です";
    entry.set_password(password).unwrap();
    let stored_password = entry.get_password().unwrap();
    assert_eq!(stored_password, password);
    entry.delete_credential().unwrap();
    assert!(matches!(entry.get_password(), Err(Error::NoEntry)))
}

#[cfg(target_os = "ios")]
fn test_get_credential() {
    use keyring::ios::IosCredential;
    let name = "test_get_credential".to_string();
    let entry = Entry::new(&name, &name).expect("Can't create entry for get_credential");
    let credential: &IosCredential = entry
        .get_credential()
        .downcast_ref()
        .expect("Not an iOS credential");
    assert!(
        credential.get_credential().is_err(),
        "Platform credential shouldn't exist yet!"
    );
    entry
        .set_password("test get password for get_credential")
        .expect("Can't get password for get_credential");
    assert!(credential.get_credential().is_ok());
    entry.delete_credential().unwrap();
    assert!(
        matches!(entry.get_password(), Err(Error::NoEntry)),
        "Platform credential exists after delete password"
    )
}
