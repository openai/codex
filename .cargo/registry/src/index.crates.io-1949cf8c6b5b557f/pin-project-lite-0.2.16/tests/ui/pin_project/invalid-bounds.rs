// SPDX-License-Identifier: Apache-2.0 OR MIT

use pin_project_lite::pin_project;

pin_project! {
    struct Generics1<T: 'static : Sized> { //~ ERROR no rules expected the token `:`
        field: T,
    }
}

pin_project! {
    struct Generics2<T: 'static : ?Sized> { //~ ERROR no rules expected the token `:`
        field: T,
    }
}

pin_project! {
    struct Generics3<T: Sized : 'static> { //~ ERROR expected one of `+`, `,`, `=`, or `>`, found `:`
        field: T,
    }
}

pin_project! {
    struct Generics4<T: ?Sized : 'static> { //~ ERROR expected one of `+`, `,`, `=`, or `>`, found `:`
        field: T,
    }
}

pin_project! {
    struct Generics5<T: Sized : ?Sized> { //~ ERROR expected one of `+`, `,`, `=`, or `>`, found `:`
        field: T,
    }
}

pin_project! {
    struct Generics6<T: ?Sized : Sized> { //~ ERROR no rules expected the token `Sized`
        field: T,
    }
}

pin_project! {
    struct WhereClause1<T>
    where
        T: 'static : Sized //~ ERROR no rules expected the token `:`
    {
        field: T,
    }
}

pin_project! {
    struct WhereClause2<T>
    where
        T: 'static : ?Sized //~ ERROR no rules expected the token `:`
    {
        field: T,
    }
}

pin_project! {
    struct WhereClause3<T>
    where
        T: Sized : 'static //~ ERROR expected `where`, or `{` after struct name, found `:`
    {
        field: T,
    }
}

pin_project! {
    struct WhereClause4<T>
    where
        T: ?Sized : 'static //~ ERROR expected `where`, or `{` after struct name, found `:`
    {
        field: T,
    }
}

pin_project! {
    struct WhereClause5<T>
    where
        T: Sized : ?Sized //~ ERROR expected `where`, or `{` after struct name, found `:`
    {
        field: T,
    }
}

pin_project! {
    struct WhereClause6<T>
    where
        T: ?Sized : Sized //~ ERROR no rules expected the token `Sized`
    {
        field: T,
    }
}

fn main() {}
