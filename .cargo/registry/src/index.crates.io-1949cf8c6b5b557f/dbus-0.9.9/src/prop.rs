use super::{Connection, Message, MessageItem, Error, Path, Interface, BusName};
use std::collections::BTreeMap;

/// Client side properties - get and set properties on a remote application.
pub struct Props<'a> {
    name: BusName<'a>,
    path: Path<'a>,
    interface: Interface<'a>,
    timeout_ms: i32,
    conn: &'a Connection,
}

impl<'a> Props<'a> {
    /// Create a new Props.
    pub fn new<N, P, I>(conn: &'a Connection, name: N, path: P, interface: I, timeout_ms: i32) -> Props<'a>
    where N: Into<BusName<'a>>, P: Into<Path<'a>>, I: Into<Interface<'a>> {
        Props {
            name: name.into(),
            path: path.into(),
            interface: interface.into(),
            timeout_ms: timeout_ms,
            conn: conn,
        }
    }

    /// Get a single property's value.
    pub fn get(&self, propname: &str) -> Result<MessageItem, Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"Get".into());
        m.append_items(&[self.interface.to_string().into(), propname.to_string().into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        let reply = r.as_result()?.get_items();
        if reply.len() == 1 {
            if let &MessageItem::Variant(ref v) = &reply[0] {
                return Ok((**v).clone())
            }
       }
       let f = format!("Invalid reply for property get {}: '{:?}'", propname, reply);
       return Err(Error::new_custom("InvalidReply", &f));
    }

    /// Set a single property's value.
    pub fn set(&self, propname: &str, value: MessageItem) -> Result<(), Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"Set".into());
        m.append_items(&[self.interface.to_string().into(), propname.to_string().into(), Box::new(value).into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        r.as_result()?;
        Ok(())
    }

    /// Get a map of all the properties' names and their values.
    pub fn get_all(&self) -> Result<BTreeMap<String, MessageItem>, Error> {
        let mut m = Message::method_call(&self.name, &self.path,
            &"org.freedesktop.DBus.Properties".into(), &"GetAll".into());
        m.append_items(&[self.interface.to_string().into()]);
        let mut r = self.conn.send_with_reply_and_block(m, self.timeout_ms)?;
        let reply = r.as_result()?.get_items();

        (|| {
            if reply.len() != 1 { return Err(()) };
            let mut t = BTreeMap::new();
            let a: &[MessageItem] = reply[0].inner()?;
            for p in a.iter() {
                let (k, v) = p.inner()?;
                let (k, v): (&String, &MessageItem) = (k.inner()?, v.inner()?);
                t.insert(k.clone(), v.clone());
            }
            Ok(t)
        })().map_err(|_| {
            let f = format!("Invalid reply for property GetAll: '{:?}'", reply);
            Error::new_custom("InvalidReply", &f)
        })
    }
}

/// Wrapper around Props that keeps a map of fetched properties.
pub struct PropHandler<'a> {
    p: Props<'a>,
    map: BTreeMap<String, MessageItem>,
}

impl<'a> PropHandler<'a> {
    /// Create a new PropHandler from a Props.
    pub fn new(p: Props) -> PropHandler {
        PropHandler { p: p, map: BTreeMap::new() }
    }

    /// Get a map of all the properties' names and their values.
    pub fn get_all(&mut self) -> Result<(), Error> {
        self.map = self.p.get_all()?;
        Ok(())
    }

    /// Get a mutable reference to the PropHandler's fetched properties.
    pub fn map_mut(&mut self) -> &mut BTreeMap<String, MessageItem> { &mut self.map }

    /// Get a reference to the PropHandler's fetched properties.
    pub fn map(&self) -> &BTreeMap<String, MessageItem> { &self.map }

    /// Get a single property's value.
    pub fn get(&mut self, propname: &str) -> Result<&MessageItem, Error> {
        let v = self.p.get(propname)?;
        self.map.insert(propname.to_string(), v);
        Ok(self.map.get(propname).unwrap())
    }

    /// Set a single property's value.
    pub fn set(&mut self, propname: &str, value: MessageItem) -> Result<(), Error> {
        self.p.set(propname, value.clone())?;
        self.map.insert(propname.to_string(), value);
        Ok(())
    }
}


/* Unfortunately org.freedesktop.DBus has no properties we can use for testing, but PolicyKit should be around on most distros. */
#[test]
fn test_get_policykit_version() {
    use super::BusType;
    let c = Connection::get_private(BusType::System).unwrap();
    let p = Props::new(&c, "org.freedesktop.PolicyKit1", "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority", 10000);

    /* Let's use both the get and getall methods and see if we get the same result */
    let v = p.get("BackendVersion").unwrap();
    let vall = p.get_all().unwrap();
    let v2 = vall.get("BackendVersion").unwrap();

    assert_eq!(&v, &*v2);
    match v {
        MessageItem::Str(ref s) => { println!("Policykit Backend version is {}", s); }
        _ => { panic!("Invalid Get: {:?}", v); }
    };
}

