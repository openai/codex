struct Foo<T>(T);

type Bar<T> = Foo<T>;

#[derive(derive_more::AsRef)]
#[as_ref(Bar<T>)]
struct Baz<T>(Foo<T>);

fn main() {
    let item = Baz(Foo(1i32));
    let _: &Bar<i32> = item.as_ref();
}
