#![cfg_attr(
	not(any(feature = "std", feature = "tokio1")),
	allow(unused_macros, unused_imports)
)]

macro_rules! Wrap {
	($name:ident, $command:ty, $wrapper:ident, $child:ty, $childer:ident, $first_child_wrapper:expr) => {
		/// A wrapper around a `Command` that allows for additional functionality to be added.
		///
		/// This is the core type of the `process-wrap` crate. It is a wrapper around a
		#[doc = concat!("[`", stringify!($command), "`].")]
		#[derive(Debug)]
		pub struct $name {
			command: $command,
			wrappers: ::indexmap::IndexMap<::std::any::TypeId, Box<dyn $wrapper>>,
		}

		impl $name {
			/// Create from a program name and a closure to configure the command.
			///
			/// This is a convenience method that creates a new `Command` and then calls the closure
			/// to configure it. The `Command` is then wrapped and returned.
			///
			#[doc = concat!("Alternatively, use `From`/`Into` to convert a [`", stringify!($command),"`] to a [`", stringify!($name), "`].")]
			pub fn with_new(
				program: impl AsRef<::std::ffi::OsStr>,
				init: impl FnOnce(&mut $command),
			) -> Self {
				let mut command = <$command>::new(program);
				init(&mut command);
				Self {
					command,
					wrappers: ::indexmap::IndexMap::new(),
				}
			}

			/// Get a reference to the wrapped command.
			pub fn command(&self) -> &$command {
				&self.command
			}

			/// Get a mutable reference to the wrapped command.
			pub fn command_mut(&mut self) -> &mut $command {
				&mut self.command
			}

			/// Get the wrapped command.
			pub fn into_command(self) -> $command {
				self.command
			}

			/// Add a wrapper to the command.
			///
			/// This is a lazy method, and the wrapper is not actually applied until `spawn` is
			/// called.
			///
			/// Only one wrapper of a given type can be applied to a command. If `wrap` is called
			/// twice with the same type, the existing wrapper will have its `extend` hook called,
			/// which gives it a chance to absorb the new wrapper. If it does not, the _new_ wrapper
			/// will be silently discarded.
			///
			/// Returns `&mut self` for chaining.
			pub fn wrap<W: $wrapper + 'static>(&mut self, wrapper: W) -> &mut Self {
				let typeid = ::std::any::TypeId::of::<W>();
				let mut wrapper = Some(Box::new(wrapper));
				let extant = self
					.wrappers
					.entry(typeid)
					.or_insert_with(|| wrapper.take().unwrap());
				if let Some(wrapper) = wrapper {
					extant.extend(wrapper);
				}

				self
			}

			// poor man's try..finally block
			#[inline]
			fn spawn_inner(
				&self,
				command: &mut $command,
				wrappers: &mut ::indexmap::IndexMap<::std::any::TypeId, Box<dyn $wrapper>>,
			) -> ::std::io::Result<Box<dyn $childer>> {
				for (id, wrapper) in wrappers.iter_mut() {
					#[cfg(feature = "tracing")]
					::tracing::debug!(?id, "pre_spawn");
					wrapper.pre_spawn(command, self)?;
				}

				let mut child = command.spawn()?;
				for (id, wrapper) in wrappers.iter_mut() {
					#[cfg(feature = "tracing")]
					::tracing::debug!(?id, "post_spawn");
					wrapper.post_spawn(&mut child, self)?;
				}

				let mut child = Box::new(
					#[allow(clippy::redundant_closure_call)]
					$first_child_wrapper(child),
				) as Box<dyn $childer>;

				for (id, wrapper) in wrappers.iter_mut() {
					#[cfg(feature = "tracing")]
					::tracing::debug!(?id, "wrap_child");
					child = wrapper.wrap_child(child, self)?;
				}

				Ok(child)
			}

			/// Spawn the command, returning a `Child` that can be interacted with.
			///
			/// In order, this runs all the `pre_spawn` hooks, then spawns the command, then runs
			/// all the `post_spawn` hooks, then stacks all the `wrap_child`s. As it returns a boxed
			/// trait object, only the methods from the trait are available directly; however you
			/// may downcast to the concrete type of the last applied wrapper if you need to.
			pub fn spawn(&mut self) -> ::std::io::Result<Box<dyn $childer>> {
				let mut command = ::std::mem::replace(&mut self.command, <$command>::new(""));
				let mut wrappers = ::std::mem::take(&mut self.wrappers);

				let res = self.spawn_inner(&mut command, &mut wrappers);

				self.command = command;
				self.wrappers = wrappers;

				res
			}

			/// Check if a wrapper of a given type is present.
			pub fn has_wrap<W: $wrapper + 'static>(&self) -> bool {
				let typeid = ::std::any::TypeId::of::<W>();
				self.wrappers.contains_key(&typeid)
			}

			/// Get a reference to a wrapper of a given type.
			///
			/// This is useful for getting access to the state of a wrapper, generally from within
			/// another wrapper.
			///
			/// Returns `None` if the wrapper is not present. To merely check if a wrapper is
			/// present, use `has_wrap` instead.
			pub fn get_wrap<W: $wrapper + 'static>(&self) -> Option<&W> {
				let typeid = ::std::any::TypeId::of::<W>();
				self.wrappers.get(&typeid).map(|w| {
					let w_any = w as &dyn ::std::any::Any;
					w_any
						.downcast_ref()
						.expect("downcasting is guaranteed to succeed due to wrap()'s internals")
				})
			}
		}

		impl From<Command> for $name {
			fn from(command: $command) -> Self {
				Self {
					command,
					wrappers: ::indexmap::IndexMap::new(),
				}
			}
		}

		/// A trait for adding functionality to a `Command`.
		///
		/// This trait provides extension or hook points into the lifecycle of a `Command`. See the
		/// [crate-level doc](crate) for an overview.
		///
		/// All methods are optional, so a minimal impl may be:
		///
		/// ```rust,ignore
		/// #[derive(Debug)]
		/// pub struct YourWrapper;
		#[doc = concat!("impl ", stringify!($wrapper), " for YourWrapper {}\n```")]
		pub trait $wrapper: ::std::fmt::Debug + Send + Sync {
			/// Called on a first instance if a second of the same type is added.
			///
			/// Only one of a wrapper type can exist within a Wrap at a time. The default behaviour
			/// is to silently discard any further invocations. However in some cases it might be
			/// useful to merge the two. This method is called on the instance already stored within
			/// the Wrap, with the new instance.
			///
			/// The `other` argument is guaranteed by process-wrap to be of the same type as `self`,
			/// so you can downcast it with `.unwrap()` and not panic. Note that it is possible for
			/// other code to use this trait and not guarantee this, so you should still panic if
			/// downcasting fails, instead of using unchecked downcasting and unleashing UB.
			///
			/// Default impl: no-op.
			fn extend(&mut self, _other: Box<dyn $wrapper>) {}

			/// Called before the command is spawned, to mutate it as needed.
			///
			/// This is where to modify the command before it is spawned. It also gives mutable
			/// access to the wrapper instance, so state can be stored if needed. The `core`
			/// reference gives access to data from other wrappers; for example, that's how
			/// `CreationFlags` on Windows works along with `JobObject`.
			///
			/// Defaut impl: no-op.
			fn pre_spawn(&mut self, _command: &mut $command, _core: &$name) -> Result<()> {
				Ok(())
			}

			/// Called after spawn, but before the child is wrapped.
			///
			/// The `core` reference gives access to data from other wrappers; for example, that's
			/// how `CreationFlags` on Windows works along with `JobObject`.
			///
			/// Default: no-op.
			fn post_spawn(&mut self, _child: &mut $child, _core: &$name) -> Result<()> {
				Ok(())
			}

			/// Called to wrap a child into this command wrapper's child wrapper.
			///
			/// If the wrapper needs to override the methods on Child, then it should create an
			/// instance of its own type implementing `ChildWrapper` and return it here. Child wraps
			/// are _in order_: you may end up with a `Foo(Bar(Child))` or a `Bar(Foo(Child))`
			/// depending on if `.wrap(Foo).wrap(Bar)` or `.wrap(Bar).wrap(Foo)` was called.
			///
			/// The `core` reference gives access to data from other wrappers; for example, that's
			/// how `CreationFlags` on Windows works along with `JobObject`.
			///
			/// Default: no-op (ie, returns the child unchanged).
			fn wrap_child(
				&mut self,
				child: Box<dyn $childer>,
				_core: &$name,
			) -> Result<Box<dyn $childer>> {
				Ok(child)
			}
		}
	};
}

macro_rules! MaybeAnyTrait {
	(
		$(#[$($attrss:tt)*])*
		$v:vis trait $name:ident $body:tt
	) => {
		#[cfg(feature = "downcasting")]
		$(#[$($attrss)*])*
		$v trait $name: std::fmt::Debug + Send + Sync + std::any::Any $body

		#[cfg(not(feature = "downcasting"))]
		$(#[$($attrss)*])*
		$v trait $name: std::fmt::Debug + Send + Sync $body
	}
}

pub(crate) use MaybeAnyTrait;
pub(crate) use Wrap;
