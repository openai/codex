// SPDX-License-Identifier: Apache-2.0 OR MIT

mod pin_argument {
    use pin_project::pin_project;

    #[pin_project]
    struct Struct {
        #[pin()] //~ ERROR unexpected token in attribute
        f: (),
    }

    #[pin_project]
    struct TupleStruct(#[pin(foo)] ()); //~ ERROR unexpected token in attribute

    #[pin_project]
    enum EnumTuple {
        V(#[pin(foo)] ()), //~ ERROR unexpected token in attribute
    }

    #[pin_project]
    enum EnumStruct {
        V {
            #[pin(foo)] //~ ERROR unexpected token in attribute
            f: (),
        },
    }
}

mod pin_attribute {
    use pin_project::pin_project;

    #[pin_project]
    struct DuplicateStruct {
        #[pin]
        #[pin] //~ ERROR duplicate #[pin] attribute
        f: (),
    }

    #[pin_project]
    struct DuplicateTupleStruct(
        #[pin]
        #[pin]
        (),
        //~^^ ERROR duplicate #[pin] attribute
    );

    #[pin_project]
    enum DuplicateEnumTuple {
        V(
            #[pin]
            #[pin]
            (),
            //~^^ ERROR duplicate #[pin] attribute
        ),
    }

    #[pin_project]
    enum DuplicateEnumStruct {
        V {
            #[pin]
            #[pin] //~ ERROR duplicate #[pin] attribute
            f: (),
        },
    }
}

mod pin_item {
    use pin_project::pin_project;

    #[pin_project]
    #[pin] //~ ERROR may only be used on fields of structs or variants
    struct Struct {
        #[pin]
        f: (),
    }

    #[pin_project]
    enum Variant {
        #[pin] //~ ERROR may only be used on fields of structs or variants
        V(()),
    }

    #[pin_project]
    #[pin] //~ ERROR may only be used on fields of structs or variants
    enum Enum {
        V(()),
    }
}

mod pin_project_argument {
    use pin_project::pin_project;

    #[pin_project(Replace)] //~ ERROR `Replace` argument was removed, use `project_replace` argument instead
    struct RemovedReplace(#[pin] ());

    #[pin_project(UnsafeUnpin,,)] //~ ERROR expected identifier
    struct Unexpected1(#[pin] ());

    #[pin_project(Foo)] //~ ERROR unexpected argument
    struct Unexpected2(#[pin] ());

    #[pin_project(,UnsafeUnpin)] //~ ERROR expected identifier
    struct Unexpected3(#[pin] ());

    #[pin_project()] // Ok
    struct Unexpected4(#[pin] ());

    #[pin_project(PinnedDrop PinnedDrop)] //~ ERROR expected `,`
    struct Unexpected5(#[pin] ());

    #[pin_project(PinnedDrop, PinnedDrop)] //~ ERROR duplicate `PinnedDrop` argument
    struct DuplicatePinnedDrop(#[pin] ());

    #[pin_project(UnsafeUnpin, UnsafeUnpin)] //~ ERROR duplicate `UnsafeUnpin` argument
    struct DuplicateUnsafeUnpin(#[pin] ());

    #[pin_project(!Unpin, !Unpin)] //~ ERROR duplicate `!Unpin` argument
    struct DuplicateNotUnpin(#[pin] ());

    #[pin_project(PinnedDrop, UnsafeUnpin, UnsafeUnpin)] //~ ERROR duplicate `UnsafeUnpin` argument
    struct Duplicate3(#[pin] ());

    #[pin_project(PinnedDrop, UnsafeUnpin, PinnedDrop, UnsafeUnpin)] //~ ERROR duplicate `PinnedDrop` argument
    struct Duplicate4(#[pin] ());

    #[pin_project(project = A, project = B)] //~ ERROR duplicate `project` argument
    struct DuplicateProject(#[pin] ());

    #[pin_project(project = A, project_ref = A, project = B)] //~ ERROR duplicate `project` argument
    struct DuplicateProject2(#[pin] ());

    #[pin_project(project_ref = A, project_ref = B)] //~ ERROR duplicate `project_ref` argument
    struct DuplicateProjectRef(#[pin] ());

    #[pin_project(project_replace = A, project_replace = B)] //~ ERROR duplicate `project_replace` argument
    struct DuplicateProjectReplace1(#[pin] ());

    #[pin_project(project_replace, project_replace = B)] //~ ERROR duplicate `project_replace` argument
    struct DuplicateProjectReplace2(#[pin] ());

    #[pin_project(project_replace = A, project_replace)] //~ ERROR duplicate `project_replace` argument
    struct DuplicateProjectReplace3(#[pin] ());

    #[pin_project(project_replace = A)] // Ok
    struct ProjectReplaceWithoutReplace(#[pin] ());

    #[pin_project(PinnedDrop, project_replace)] //~ ERROR arguments `PinnedDrop` and `project_replace` are mutually exclusive
    struct PinnedDropWithProjectReplace1(#[pin] ());

    #[pin_project(project_replace, UnsafeUnpin, PinnedDrop)] //~ ERROR arguments `PinnedDrop` and `project_replace` are mutually exclusive
    struct PinnedDropWithProjectReplace2(#[pin] ());

    #[pin_project(UnsafeUnpin, !Unpin)] //~ ERROR arguments `UnsafeUnpin` and `!Unpin` are mutually exclusive
    struct UnsafeUnpinWithNotUnpin1(#[pin] ());

    #[pin_project(!Unpin, PinnedDrop, UnsafeUnpin)] //~ ERROR arguments `UnsafeUnpin` and `!Unpin` are mutually exclusive
    struct UnsafeUnpinWithNotUnpin2(#[pin] ());

    #[pin_project(!)] //~ ERROR expected `!Unpin`, found `!`
    struct NotUnpin1(#[pin] ());

    #[pin_project(Unpin)] //~ ERROR unexpected argument
    struct NotUnpin2(#[pin] ());

    #[pin_project(project)] //~ ERROR expected `project = <identifier>`, found `project`
    struct Project1(#[pin] ());

    #[pin_project(project = )] //~ ERROR expected `project = <identifier>`, found `project =`
    struct Project2(#[pin] ());

    #[pin_project(project = !)] //~ ERROR expected identifier
    struct Project3(#[pin] ());

    #[pin_project(project_ref)] //~ ERROR expected `project_ref = <identifier>`, found `project_ref`
    struct ProjectRef1(#[pin] ());

    #[pin_project(project_ref = )] //~ ERROR expected `project_ref = <identifier>`, found `project_ref =`
    struct ProjectRef2(#[pin] ());

    #[pin_project(project_ref = !)] //~ ERROR expected identifier
    struct ProjectRef3(#[pin] ());

    #[pin_project(project_replace)] // Ok
    struct ProjectReplace1(#[pin] ());

    #[pin_project(project_replace = )] //~ ERROR expected `project_replace = <identifier>`, found `project_replace =`
    struct ProjectReplace2(#[pin] ());

    #[pin_project(project_replace = !)] //~ ERROR expected identifier
    struct ProjectReplace3(#[pin] ());

    #[pin_project(project_replace)] //~ ERROR `project_replace` argument requires a value when used on enums
    enum ProjectReplaceEnum {
        V(#[pin] ()),
    }
}

mod pin_project_conflict_naming {
    use pin_project::pin_project;

    #[pin_project(project = OrigAndProj)] //~ ERROR name `OrigAndProj` is the same as the original type name
    struct OrigAndProj(#[pin] ());

    #[pin_project(project_ref = OrigAndProjRef)] //~ ERROR name `OrigAndProjRef` is the same as the original type name
    struct OrigAndProjRef(#[pin] ());

    #[pin_project(project_replace = OrigAndProjOwn)] //~ ERROR name `OrigAndProjOwn` is the same as the original type name
    struct OrigAndProjOwn(#[pin] ());

    #[pin_project(project = A, project_ref = A)] //~ ERROR name `A` is already specified by `project` argument
    struct ProjAndProjRef(#[pin] ());

    #[pin_project(project = A, project_replace = A)] //~ ERROR name `A` is already specified by `project` argument
    struct ProjAndProjOwn(#[pin] ());

    #[pin_project(project_ref = A, project_replace = A)] //~ ERROR name `A` is already specified by `project_ref` argument
    struct ProjRefAndProjOwn(#[pin] ());
}

mod pin_project_attribute {
    use pin_project::pin_project;

    #[pin_project]
    #[pin_project] //~ ERROR duplicate #[pin_project] attribute
    struct Duplicate(#[pin] ());
}

mod pin_project_item {
    use pin_project::pin_project;

    #[pin_project]
    struct Struct {} //~ ERROR may not be used on structs with zero fields

    #[pin_project]
    struct TupleStruct(); //~ ERROR may not be used on structs with zero fields

    #[pin_project]
    struct UnitStruct; //~ ERROR may not be used on structs with zero fields

    #[pin_project]
    enum EnumEmpty {} //~ ERROR may not be used on enums without variants

    #[pin_project]
    enum EnumDiscriminant {
        V = 2, //~ ERROR may not be used on enums with discriminants
    }

    #[pin_project]
    enum EnumZeroFields {
        Unit, //~ ERROR may not be used on enums with zero fields
        Tuple(),
        Struct {},
    }

    #[pin_project]
    union Union {
        //~^ ERROR may only be used on structs or enums
        f: (),
    }

    #[pin_project]
    impl Impl {} //~ ERROR may only be used on structs or enums
}

// #[repr(packed)] is always detected first, even on unsupported structs.
mod pin_project_item_packed {
    use pin_project::pin_project;

    #[pin_project]
    #[repr(packed)]
    struct Struct {} //~ ERROR may not be used on #[repr(packed)] types

    #[pin_project]
    #[repr(packed)]
    struct TupleStruct(); //~ ERROR may not be used on #[repr(packed)] types

    #[pin_project]
    #[repr(packed)]
    struct UnitStruct; //~ ERROR may not be used on #[repr(packed)] types
}

fn main() {}
