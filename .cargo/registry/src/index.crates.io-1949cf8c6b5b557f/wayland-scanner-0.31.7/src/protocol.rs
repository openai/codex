use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Debug)]
pub struct Protocol {
    pub name: String,
    pub copyright: Option<String>,
    pub description: Option<(String, String)>,
    pub interfaces: Vec<Interface>,
}

impl Protocol {
    pub fn new(name: String) -> Protocol {
        Protocol { name, copyright: None, description: None, interfaces: Vec::new() }
    }
}

#[derive(Clone, Debug)]
pub struct Interface {
    pub name: String,
    pub version: u32,
    pub description: Option<(String, String)>,
    pub requests: Vec<Message>,
    pub events: Vec<Message>,
    pub enums: Vec<Enum>,
}

impl Interface {
    pub fn new() -> Interface {
        Interface {
            name: String::new(),
            version: 1,
            description: None,
            requests: Vec::new(),
            events: Vec::new(),
            enums: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Message {
    pub name: String,
    pub typ: Option<Type>,
    pub since: u32,
    pub description: Option<(String, String)>,
    pub args: Vec<Arg>,
}

impl Message {
    pub fn new() -> Message {
        Message { name: String::new(), typ: None, since: 1, description: None, args: Vec::new() }
    }

    pub fn all_null(&self) -> bool {
        self.args
            .iter()
            .all(|a| !((a.typ == Type::Object || a.typ == Type::NewId) && a.interface.is_some()))
    }
}

#[derive(Clone, Debug)]
pub struct Arg {
    pub name: String,
    pub typ: Type,
    pub interface: Option<String>,
    pub summary: Option<String>,
    pub description: Option<(String, String)>,
    pub allow_null: bool,
    pub enum_: Option<String>,
}

impl Arg {
    pub fn new() -> Arg {
        Arg {
            name: String::new(),
            typ: Type::Object,
            interface: None,
            summary: None,
            description: None,
            allow_null: false,
            enum_: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Enum {
    pub name: String,
    pub since: u16,
    pub description: Option<(String, String)>,
    pub entries: Vec<Entry>,
    pub bitfield: bool,
}

impl Enum {
    pub fn new() -> Enum {
        Enum {
            name: String::new(),
            since: 1,
            description: None,
            entries: Vec::new(),
            bitfield: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub value: u32,
    pub since: u16,
    pub description: Option<(String, String)>,
    pub summary: Option<String>,
}

impl Entry {
    pub fn new() -> Entry {
        Entry { name: String::new(), value: 0, since: 1, description: None, summary: None }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Type {
    Int,
    Uint,
    Fixed,
    String,
    Object,
    NewId,
    Array,
    Fd,
    Destructor,
}

impl Type {
    pub fn nullable(self) -> bool {
        matches!(self, Type::String | Type::Object)
    }

    pub fn common_type(self) -> TokenStream {
        match self {
            Type::Int => quote!(Int),
            Type::Uint => quote!(Uint),
            Type::Fixed => quote!(Fixed),
            Type::Array => quote!(Array),
            Type::Fd => quote!(Fd),
            Type::String => quote!(Str),
            Type::Object => quote!(Object),
            Type::NewId => quote!(NewId),
            Type::Destructor => panic!("Destructor is not a valid argument type."),
        }
    }
}
