enum Material {
  //  ^ type
  DENIM,
  // ^ constant
  CANVAS,
  // ^ constant
  SPANDEX_3_PERCENT
  // ^ constant
}

class Person {
  //   ^ type

  Person(string name) {
    // <- type
    //    ^ type
    this.name = name;
    //            ^ variable
    this.pants = new Pants<Pocket>();
    //                ^ type
    //                       ^ type
  }

  string getName() {
    // <- type
    //    ^ function.method
    a = this.name;
    b = new one.two.Three();
    //      ^ type
    //          ^ type
    //              ^ type
    c = Material.DENIM;
    //      ^ type
    //            ^ constant
  }
}
