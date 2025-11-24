macro_rules! clone_fields {
    ($name:ident, $($field:ident),+ $(,)*) => (
        fn clone(&self) -> Self {
            $name {
                $(
                    $field : self . $field .clone()
                ),*
            }
        }
    );
}

macro_rules! iterator_wrap {
    (impl () for
    struct $name: ident <$($typarm:tt),*> where { $($bounds: tt)* }
     item: $item: ty,
     iter: $iter: ty,
     ) => ();
    (
    impl (Iterator $($rest:tt)*)  for
    $(#[$derive:meta])*
     struct $name: ident <$($typarm:tt),*> where { $($bounds: tt)* }
     item: $item: ty,
     iter: $iter: ty,
     ) => (
        // having complex iterator types is kind of the point of this macro
        #[allow(clippy::type_complexity)]
        $(#[$derive])*
        pub struct $name <$($typarm),*> where $($bounds)* {
            iter: $iter,
        }
        impl<$($typarm),*> Iterator for $name <$($typarm),*>
            where $($bounds)*
        {
            type Item = $item;
            #[inline]
            fn next(&mut self) -> Option<Self::Item> {
                self.iter.next()
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.iter.size_hint()
            }
        }
        iterator_wrap!(
            impl ($($rest)*)  for
            struct $name <$($typarm),*> where { $($bounds)* }
            item: $item,
            iter: $iter,
            );
        );

    (
impl (ExactSizeIterator $($rest:tt)*)  for
    $(#[$derive:meta])*
     struct $name: ident <$($typarm:tt),*> where { $($bounds: tt)* }
     item: $item: ty,
     iter: $iter: ty,
     ) => (
        impl<$($typarm),*> ExactSizeIterator for $name <$($typarm),*>
            where $($bounds)*
        {
            #[inline]
            fn len(&self) -> usize {
                self.iter.len()
            }
        }
        iterator_wrap!(
            impl ($($rest)*)  for
         $(#[$derive])*
            struct $name <$($typarm),*> where { $($bounds)* }
            item: $item,
            iter: $iter,
            );
    );

    (
impl (DoubleEndedIterator $($rest:tt)*)  for
    $(#[$derive:meta])*
     struct $name: ident <$($typarm:tt),*> where { $($bounds: tt)* }
     item: $item: ty,
     iter: $iter: ty,
     ) => (
        impl<$($typarm),*> DoubleEndedIterator for $name <$($typarm),*>
            where $($bounds)*
        {
            fn next_back(&mut self) -> Option<Self::Item> {
                self.iter.next_back()
            }
            fn rfold<B, F>(self, accum: B, f: F) -> B
                where
                    F: FnMut(B, Self::Item) -> B,
                { self.iter.rfold(accum, f) }
            fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
                where
                    P: FnMut(&Self::Item) -> bool,
                { self.iter.rfind(predicate) }
        }
        iterator_wrap!(
            impl ($($rest)*)  for
         $(#[$derive])*
           struct $name <$($typarm),*> where { $($bounds)* }
            item: $item,
            iter: $iter,
            );
    );
}
