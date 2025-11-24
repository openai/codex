/*
 * Copyright 2018 The Starlark in Rust Authors.
 * Copyright (c) Facebook, Inc. and its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     https://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::fmt::Display;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;

#[derive(Debug, Allocative)]
enum Inner<T: ?Sized + 'static> {
    Arc(Arc<T>),
    Static(&'static T),
}

#[derive(Debug, Allocative)]
pub(crate) struct ArcOrStatic<T: ?Sized + 'static>(Inner<T>);

impl<T: ?Sized + 'static> ArcOrStatic<T> {
    pub(crate) fn new_static(a: &'static T) -> Self {
        ArcOrStatic(Inner::Static(a))
    }

    pub(crate) fn new_arc(a: Arc<T>) -> Self {
        ArcOrStatic(Inner::Arc(a))
    }

    pub(crate) fn new(a: T) -> Self
    where
        T: Sized,
    {
        Self::new_arc(Arc::new(a))
    }
}

impl<T: ?Sized + 'static> Deref for ArcOrStatic<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self.0 {
            Inner::Arc(a) => a,
            Inner::Static(s) => s,
        }
    }
}

impl<T: Display + 'static> Display for ArcOrStatic<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + 'static> Clone for ArcOrStatic<T> {
    fn clone(&self) -> Self {
        Self(match &self.0 {
            Inner::Arc(a) => Inner::Arc(a.dupe()),
            Inner::Static(s) => Inner::Static(*s),
        })
    }
}

impl<T: ?Sized + 'static> Dupe for ArcOrStatic<T> {}

impl<T: ?Sized + 'static> PartialEq for ArcOrStatic<T>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T: ?Sized + 'static> Eq for ArcOrStatic<T> where T: Eq {}

impl<T: ?Sized + 'static> PartialOrd for ArcOrStatic<T>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<T: ?Sized + 'static> Ord for ArcOrStatic<T>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<T: ?Sized + 'static> Hash for ArcOrStatic<T>
where
    T: Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}
