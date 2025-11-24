struct Foo<T>(T);

type Bar<T> = Foo<T>;

#[derive(derive_more::AsMut)]
#[as_mut(Bar<T>)]
struct Baz<T>(Foo<T>);

fn main() {
    let mut item = Baz(Foo(1i32));
    let _: &mut Bar<i32> = item.as_mut();
}
