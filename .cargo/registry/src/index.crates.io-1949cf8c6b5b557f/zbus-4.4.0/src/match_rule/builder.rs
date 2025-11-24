use static_assertions::assert_impl_all;

use crate::{
    match_rule::PathSpec,
    message::Type,
    names::{BusName, InterfaceName, MemberName, UniqueName},
    zvariant::{ObjectPath, Str},
    Error, MatchRule, Result,
};

const MAX_ARGS: u8 = 64;

/// Builder for [`MatchRule`].
///
/// This is created by [`MatchRule::builder`].
#[derive(Debug)]
pub struct Builder<'m>(MatchRule<'m>);

assert_impl_all!(Builder<'_>: Send, Sync, Unpin);

impl<'m> Builder<'m> {
    /// Build the `MatchRule`.
    pub fn build(self) -> MatchRule<'m> {
        self.0
    }

    /// Set the sender.
    pub fn sender<B>(mut self, sender: B) -> Result<Self>
    where
        B: TryInto<BusName<'m>>,
        B::Error: Into<Error>,
    {
        self.0.sender = Some(sender.try_into().map_err(Into::into)?);

        Ok(self)
    }

    /// Set the message type.
    pub fn msg_type(mut self, msg_type: Type) -> Self {
        self.0.msg_type = Some(msg_type);

        self
    }

    /// Set the interface.
    pub fn interface<I>(mut self, interface: I) -> Result<Self>
    where
        I: TryInto<InterfaceName<'m>>,
        I::Error: Into<Error>,
    {
        self.0.interface = Some(interface.try_into().map_err(Into::into)?);

        Ok(self)
    }

    /// Set the member.
    pub fn member<M>(mut self, member: M) -> Result<Self>
    where
        M: TryInto<MemberName<'m>>,
        M::Error: Into<Error>,
    {
        self.0.member = Some(member.try_into().map_err(Into::into)?);

        Ok(self)
    }

    /// Set the path.
    ///
    /// Note: Since both a path and a path namespace are not allowed to appear in a match rule at
    /// the same time, this overrides any path namespace previously set.
    pub fn path<P>(mut self, path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'m>>,
        P::Error: Into<Error>,
    {
        self.0.path_spec = path
            .try_into()
            .map(PathSpec::Path)
            .map(Some)
            .map_err(Into::into)?;

        Ok(self)
    }

    /// Set the path namespace.
    ///
    /// Note: Since both a path and a path namespace are not allowed to appear in a match rule at
    /// the same time, this overrides any path previously set.
    pub fn path_namespace<P>(mut self, path_namespace: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'m>>,
        P::Error: Into<Error>,
    {
        self.0.path_spec = path_namespace
            .try_into()
            .map(PathSpec::PathNamespace)
            .map(Some)
            .map_err(Into::into)?;

        Ok(self)
    }

    /// Set the destination.
    pub fn destination<B>(mut self, destination: B) -> Result<Self>
    where
        B: TryInto<UniqueName<'m>>,
        B::Error: Into<Error>,
    {
        self.0.destination = Some(destination.try_into().map_err(Into::into)?);

        Ok(self)
    }

    /// Append an arguments.
    ///
    /// Use this in instead of [`Builder::arg`] if you want to sequentially add args.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidMatchRule`] on attempt to add the 65th argument.
    pub fn add_arg<S>(self, arg: S) -> Result<Self>
    where
        S: Into<Str<'m>>,
    {
        let idx = self.0.args.len() as u8;

        self.arg(idx, arg)
    }

    /// Add an argument of a specified index.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidMatchRule`] if `idx` is greater than 64.
    pub fn arg<S>(mut self, idx: u8, arg: S) -> Result<Self>
    where
        S: Into<Str<'m>>,
    {
        if idx >= MAX_ARGS {
            return Err(Error::InvalidMatchRule);
        }
        let value = (idx, arg.into());
        let vec_idx = match self.0.args().binary_search_by(|(i, _)| i.cmp(&idx)) {
            Ok(i) => {
                // If the argument is already present, replace it.
                self.0.args.remove(i);

                i
            }
            Err(i) => i,
        };
        self.0.args.insert(vec_idx, value);

        Ok(self)
    }

    /// Append a path argument.
    ///
    /// Use this in instead of [`Builder::arg_path`] if you want to sequentially add args.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidMatchRule`] on attempt to add the 65th path argument.
    pub fn add_arg_path<P>(self, arg_path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'m>>,
        P::Error: Into<Error>,
    {
        let idx = self.0.arg_paths.len() as u8;

        self.arg_path(idx, arg_path)
    }

    /// Add a path argument of a specified index.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidMatchRule`] if `idx` is greater than 64.
    pub fn arg_path<P>(mut self, idx: u8, arg_path: P) -> Result<Self>
    where
        P: TryInto<ObjectPath<'m>>,
        P::Error: Into<Error>,
    {
        if idx >= MAX_ARGS {
            return Err(Error::InvalidMatchRule);
        }

        let value = (idx, arg_path.try_into().map_err(Into::into)?);
        let vec_idx = match self.0.arg_paths().binary_search_by(|(i, _)| i.cmp(&idx)) {
            Ok(i) => {
                // If the argument is already present, replace it.
                self.0.arg_paths.remove(i);

                i
            }
            Err(i) => i,
        };
        self.0.arg_paths.insert(vec_idx, value);

        Ok(self)
    }

    /// Set 0th argument's namespace.
    ///
    /// The namespace be a valid bus name or a valid element of a bus name. For more information,
    /// see [the spec](https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus).
    ///
    /// # Examples
    ///
    /// ```
    /// # use zbus::MatchRule;
    /// // Valid namespaces
    /// MatchRule::builder().arg0ns("org.mpris.MediaPlayer2").unwrap();
    /// MatchRule::builder().arg0ns("org").unwrap();
    /// MatchRule::builder().arg0ns(":org").unwrap();
    /// MatchRule::builder().arg0ns(":1org").unwrap();
    ///
    /// // Invalid namespaces
    /// MatchRule::builder().arg0ns("org.").unwrap_err();
    /// MatchRule::builder().arg0ns(".org").unwrap_err();
    /// MatchRule::builder().arg0ns("1org").unwrap_err();
    /// MatchRule::builder().arg0ns(".").unwrap_err();
    /// MatchRule::builder().arg0ns("org..freedesktop").unwrap_err();
    /// ````
    pub fn arg0ns<S>(mut self, namespace: S) -> Result<Self>
    where
        S: Into<Str<'m>>,
    {
        let namespace: Str<'m> = namespace.into();

        // Rules: https://dbus.freedesktop.org/doc/dbus-specification.html#message-protocol-names-bus
        // minus the requirement to have more than one element.

        if namespace.is_empty() || namespace.len() > 255 {
            return Err(Error::InvalidMatchRule);
        }

        let (is_unique, namespace_str) = match namespace.strip_prefix(':') {
            Some(s) => (true, s),
            None => (false, namespace.as_str()),
        };

        let valid_first_char = |s: &str| match s.chars().next() {
            None | Some('.') => false,
            Some('0'..='9') if !is_unique => false,
            _ => true,
        };

        if !valid_first_char(namespace_str)
            || !namespace_str.split('.').all(valid_first_char)
            || !namespace_str
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return Err(Error::InvalidMatchRule);
        }

        self.0.arg0ns = Some(namespace);

        Ok(self)
    }

    /// Create a builder for `MatchRule`.
    pub(crate) fn new() -> Self {
        Self(MatchRule {
            msg_type: None,
            sender: None,
            interface: None,
            member: None,
            path_spec: None,
            destination: None,
            args: Vec::with_capacity(MAX_ARGS as usize),
            arg_paths: Vec::with_capacity(MAX_ARGS as usize),
            arg0ns: None,
        })
    }
}
