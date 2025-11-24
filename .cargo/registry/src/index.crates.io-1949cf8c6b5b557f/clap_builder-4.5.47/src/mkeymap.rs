use std::iter::Iterator;
use std::ops::Index;

use crate::builder::OsStr;
use crate::Arg;
use crate::INTERNAL_ERROR_MSG;

#[derive(PartialEq, Eq, Debug, Clone)]
pub(crate) struct Key {
    key: KeyType,
    index: usize,
}

#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub(crate) struct MKeyMap {
    /// All of the arguments.
    args: Vec<Arg>,

    // Cache part:
    /// Will be set after `_build()`.
    keys: Vec<Key>,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub(crate) enum KeyType {
    Short(char),
    Long(OsStr),
    Position(usize),
}

impl KeyType {
    pub(crate) fn is_position(&self) -> bool {
        matches!(self, KeyType::Position(_))
    }
}

impl PartialEq<usize> for KeyType {
    fn eq(&self, rhs: &usize) -> bool {
        match self {
            KeyType::Position(x) => x == rhs,
            _ => false,
        }
    }
}

impl PartialEq<&str> for KeyType {
    fn eq(&self, rhs: &&str) -> bool {
        match self {
            KeyType::Long(l) => l == rhs,
            _ => false,
        }
    }
}

impl PartialEq<str> for KeyType {
    fn eq(&self, rhs: &str) -> bool {
        match self {
            KeyType::Long(l) => l == rhs,
            _ => false,
        }
    }
}

impl PartialEq<OsStr> for KeyType {
    fn eq(&self, rhs: &OsStr) -> bool {
        match self {
            KeyType::Long(l) => l == rhs,
            _ => false,
        }
    }
}

impl PartialEq<char> for KeyType {
    fn eq(&self, rhs: &char) -> bool {
        match self {
            KeyType::Short(c) => c == rhs,
            _ => false,
        }
    }
}

impl MKeyMap {
    /// If any arg has corresponding key in this map, we can search the key with
    /// `u64` (for positional argument), `char` (for short flag), `&str` and `OsString`
    /// (for long flag)
    pub(crate) fn contains<K>(&self, key: K) -> bool
    where
        KeyType: PartialEq<K>,
    {
        self.keys.iter().any(|x| x.key == key)
    }

    /// Push an argument in the map.
    pub(crate) fn push(&mut self, new_arg: Arg) {
        self.args.push(new_arg);
    }

    /// Find the arg have corresponding key in this map, we can search the key
    /// with `u64` (for positional argument), `char` (for short flag), `&str` and
    /// `OsString` (for long flag)
    pub(crate) fn get<K: ?Sized>(&self, key: &K) -> Option<&Arg>
    where
        KeyType: PartialEq<K>,
    {
        self.keys
            .iter()
            .find(|k| &k.key == key)
            .map(|k| &self.args[k.index])
    }

    /// Return iterators of all keys.
    pub(crate) fn keys(&self) -> impl Iterator<Item = &KeyType> {
        self.keys.iter().map(|x| &x.key)
    }

    /// Return iterators of all args.
    pub(crate) fn args(&self) -> impl Iterator<Item = &Arg> {
        self.args.iter()
    }

    /// Return mutable iterators of all args.
    pub(crate) fn args_mut(&mut self) -> impl Iterator<Item = &mut Arg> {
        self.args.iter_mut()
    }

    /// Mutate every argument.
    pub(crate) fn mut_args<F>(&mut self, f: F)
    where
        F: FnMut(Arg) -> Arg,
    {
        let mut args = std::mem::take(&mut self.args);
        self.args.extend(args.drain(..).map(f));
    }

    /// We need a lazy build here since some we may change args after creating
    /// the map, you can checkout who uses `args_mut`.
    pub(crate) fn _build(&mut self) {
        // There will be at least as many keys as args, so that is a good starting point
        self.keys.reserve(self.args.len());
        for (i, arg) in self.args.iter().enumerate() {
            append_keys(&mut self.keys, arg, i);
        }
    }

    /// Remove an arg in the graph by Id, usually used by `mut_arg`. Return
    /// `Some(arg)` if removed.
    pub(crate) fn remove_by_name(&mut self, name: &str) -> Option<Arg> {
        self.args
            .iter()
            .position(|arg| arg.id == name)
            // since it's a cold function, using this wouldn't hurt much
            .map(|i| self.args.remove(i))
    }
}

impl Index<&'_ KeyType> for MKeyMap {
    type Output = Arg;

    fn index(&self, key: &KeyType) -> &Self::Output {
        self.get(key).expect(INTERNAL_ERROR_MSG)
    }
}

/// Generate key types for an specific Arg.
fn append_keys(keys: &mut Vec<Key>, arg: &Arg, index: usize) {
    if let Some(pos_index) = arg.index {
        let key = KeyType::Position(pos_index);
        keys.push(Key { key, index });
    } else {
        if let Some(short) = arg.short {
            let key = KeyType::Short(short);
            keys.push(Key { key, index });
        }
        if let Some(long) = arg.long.clone() {
            let key = KeyType::Long(long.into());
            keys.push(Key { key, index });
        }

        for (short, _) in arg.short_aliases.iter() {
            let key = KeyType::Short(*short);
            keys.push(Key { key, index });
        }
        for (long, _) in arg.aliases.iter() {
            let key = KeyType::Long(long.into());
            keys.push(Key { key, index });
        }
    }
}
