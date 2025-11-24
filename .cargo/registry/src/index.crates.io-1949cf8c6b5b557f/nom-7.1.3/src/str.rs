#[cfg(test)]
mod test {
  #[cfg(feature = "alloc")]
  use crate::{branch::alt, bytes::complete::tag_no_case, combinator::recognize, multi::many1};
  use crate::{
    bytes::complete::{is_a, is_not, tag, take, take_till, take_until},
    error::{self, ErrorKind},
    Err, IResult,
  };

  #[test]
  fn tagtr_succeed() {
    const INPUT: &str = "Hello World!";
    const TAG: &str = "Hello";
    fn test(input: &str) -> IResult<&str, &str> {
      tag(TAG)(input)
    }

    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(extra == " World!", "Parser `tag` consumed leftover input.");
        assert!(
          output == TAG,
          "Parser `tag` doesn't return the tag it matched on success. \
           Expected `{}`, got `{}`.",
          TAG,
          output
        );
      }
      other => panic!(
        "Parser `tag` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn tagtr_incomplete() {
    use crate::bytes::streaming::tag;

    const INPUT: &str = "Hello";
    const TAG: &str = "Hello World!";

    let res: IResult<_, _, error::Error<_>> = tag(TAG)(INPUT);
    match res {
      Err(Err::Incomplete(_)) => (),
      other => {
        panic!(
          "Parser `tag` didn't require more input when it should have. \
           Got `{:?}`.",
          other
        );
      }
    };
  }

  #[test]
  fn tagtr_error() {
    const INPUT: &str = "Hello World!";
    const TAG: &str = "Random"; // TAG must be closer than INPUT.

    let res: IResult<_, _, error::Error<_>> = tag(TAG)(INPUT);
    match res {
      Err(Err::Error(_)) => (),
      other => {
        panic!(
          "Parser `tag` didn't fail when it should have. Got `{:?}`.`",
          other
        );
      }
    };
  }

  #[test]
  fn take_s_succeed() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";

    let res: IResult<_, _, error::Error<_>> = take(9_usize)(INPUT);
    match res {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_s` consumed leftover input. Leftover `{}`.",
          extra
        );
        assert!(
          output == CONSUMED,
          "Parser `take_s` doesn't return the string it consumed on success. Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_s` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_until_succeed() {
    const INPUT: &str = "βèƒôřèÂßÇ∂áƒƭèř";
    const FIND: &str = "ÂßÇ∂";
    const CONSUMED: &str = "βèƒôřè";
    const LEFTOVER: &str = "ÂßÇ∂áƒƭèř";

    let res: IResult<_, _, (_, ErrorKind)> = take_until(FIND)(INPUT);
    match res {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_until`\
           consumed leftover input. Leftover `{}`.",
          extra
        );
        assert!(
          output == CONSUMED,
          "Parser `take_until`\
           doesn't return the string it consumed on success. Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_until` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_s_incomplete() {
    use crate::bytes::streaming::take;

    const INPUT: &str = "βèƒôřèÂßÇá";

    let res: IResult<_, _, (_, ErrorKind)> = take(13_usize)(INPUT);
    match res {
      Err(Err::Incomplete(_)) => (),
      other => panic!(
        "Parser `take` didn't require more input when it should have. \
         Got `{:?}`.",
        other
      ),
    }
  }

  use crate::internal::Needed;

  fn is_alphabetic(c: char) -> bool {
    (c as u8 >= 0x41 && c as u8 <= 0x5A) || (c as u8 >= 0x61 && c as u8 <= 0x7A)
  }

  #[test]
  fn take_while() {
    use crate::bytes::streaming::take_while;

    fn f(i: &str) -> IResult<&str, &str> {
      take_while(is_alphabetic)(i)
    }
    let a = "";
    let b = "abcd";
    let c = "abcd123";
    let d = "123";

    assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
    assert_eq!(f(&b[..]), Err(Err::Incomplete(Needed::new(1))));
    assert_eq!(f(&c[..]), Ok((&d[..], &b[..])));
    assert_eq!(f(&d[..]), Ok((&d[..], &a[..])));
  }

  #[test]
  fn take_while1() {
    use crate::bytes::streaming::take_while1;

    fn f(i: &str) -> IResult<&str, &str> {
      take_while1(is_alphabetic)(i)
    }
    let a = "";
    let b = "abcd";
    let c = "abcd123";
    let d = "123";

    assert_eq!(f(&a[..]), Err(Err::Incomplete(Needed::new(1))));
    assert_eq!(f(&b[..]), Err(Err::Incomplete(Needed::new(1))));
    assert_eq!(f(&c[..]), Ok((&"123"[..], &b[..])));
    assert_eq!(
      f(&d[..]),
      Err(Err::Error(error_position!(&d[..], ErrorKind::TakeWhile1)))
    );
  }

  #[test]
  fn take_till_s_succeed() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";
    fn till_s(c: char) -> bool {
      c == 'á'
    }
    fn test(input: &str) -> IResult<&str, &str> {
      take_till(till_s)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_till` consumed leftover input."
        );
        assert!(
          output == CONSUMED,
          "Parser `take_till` doesn't return the string it consumed on success. \
           Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_till` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_while_succeed_none() {
    use crate::bytes::complete::take_while;

    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const CONSUMED: &str = "";
    const LEFTOVER: &str = "βèƒôřèÂßÇáƒƭèř";
    fn while_s(c: char) -> bool {
      c == '9'
    }
    fn test(input: &str) -> IResult<&str, &str> {
      take_while(while_s)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_while` consumed leftover input."
        );
        assert!(
          output == CONSUMED,
          "Parser `take_while` doesn't return the string it consumed on success. \
           Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_while` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn is_not_succeed() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const AVOID: &str = "£úçƙ¥á";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";
    fn test(input: &str) -> IResult<&str, &str> {
      is_not(AVOID)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `is_not` consumed leftover input. Leftover `{}`.",
          extra
        );
        assert!(
          output == CONSUMED,
          "Parser `is_not` doesn't return the string it consumed on success. Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `is_not` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_while_succeed_some() {
    use crate::bytes::complete::take_while;

    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";
    fn while_s(c: char) -> bool {
      c == 'β'
        || c == 'è'
        || c == 'ƒ'
        || c == 'ô'
        || c == 'ř'
        || c == 'è'
        || c == 'Â'
        || c == 'ß'
        || c == 'Ç'
    }
    fn test(input: &str) -> IResult<&str, &str> {
      take_while(while_s)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_while` consumed leftover input."
        );
        assert!(
          output == CONSUMED,
          "Parser `take_while` doesn't return the string it consumed on success. \
           Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_while` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn is_not_fail() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const AVOID: &str = "βúçƙ¥";
    fn test(input: &str) -> IResult<&str, &str> {
      is_not(AVOID)(input)
    }
    match test(INPUT) {
      Err(Err::Error(_)) => (),
      other => panic!(
        "Parser `is_not` didn't fail when it should have. Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_while1_succeed() {
    use crate::bytes::complete::take_while1;

    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";
    fn while1_s(c: char) -> bool {
      c == 'β'
        || c == 'è'
        || c == 'ƒ'
        || c == 'ô'
        || c == 'ř'
        || c == 'è'
        || c == 'Â'
        || c == 'ß'
        || c == 'Ç'
    }
    fn test(input: &str) -> IResult<&str, &str> {
      take_while1(while1_s)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `take_while1` consumed leftover input."
        );
        assert!(
          output == CONSUMED,
          "Parser `take_while1` doesn't return the string it consumed on success. \
           Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `take_while1` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_until_incomplete() {
    use crate::bytes::streaming::take_until;

    const INPUT: &str = "βèƒôřè";
    const FIND: &str = "βèƒôřèÂßÇ";

    let res: IResult<_, _, (_, ErrorKind)> = take_until(FIND)(INPUT);
    match res {
      Err(Err::Incomplete(_)) => (),
      other => panic!(
        "Parser `take_until` didn't require more input when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn is_a_succeed() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const MATCH: &str = "βèƒôřèÂßÇ";
    const CONSUMED: &str = "βèƒôřèÂßÇ";
    const LEFTOVER: &str = "áƒƭèř";
    fn test(input: &str) -> IResult<&str, &str> {
      is_a(MATCH)(input)
    }
    match test(INPUT) {
      Ok((extra, output)) => {
        assert!(
          extra == LEFTOVER,
          "Parser `is_a` consumed leftover input. Leftover `{}`.",
          extra
        );
        assert!(
          output == CONSUMED,
          "Parser `is_a` doesn't return the string it consumed on success. Expected `{}`, got `{}`.",
          CONSUMED,
          output
        );
      }
      other => panic!(
        "Parser `is_a` didn't succeed when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_while1_fail() {
    use crate::bytes::complete::take_while1;

    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    fn while1_s(c: char) -> bool {
      c == '9'
    }
    fn test(input: &str) -> IResult<&str, &str> {
      take_while1(while1_s)(input)
    }
    match test(INPUT) {
      Err(Err::Error(_)) => (),
      other => panic!(
        "Parser `take_while1` didn't fail when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn is_a_fail() {
    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const MATCH: &str = "Ûñℓúçƙ¥";
    fn test(input: &str) -> IResult<&str, &str> {
      is_a(MATCH)(input)
    }
    match test(INPUT) {
      Err(Err::Error(_)) => (),
      other => panic!(
        "Parser `is_a` didn't fail when it should have. Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  fn take_until_error() {
    use crate::bytes::streaming::take_until;

    const INPUT: &str = "βèƒôřèÂßÇáƒƭèř";
    const FIND: &str = "Ráñδô₥";

    let res: IResult<_, _, (_, ErrorKind)> = take_until(FIND)(INPUT);
    match res {
      Err(Err::Incomplete(_)) => (),
      other => panic!(
        "Parser `take_until` didn't fail when it should have. \
         Got `{:?}`.",
        other
      ),
    };
  }

  #[test]
  #[cfg(feature = "alloc")]
  fn recognize_is_a() {
    let a = "aabbab";
    let b = "ababcd";

    fn f(i: &str) -> IResult<&str, &str> {
      recognize(many1(alt((tag("a"), tag("b")))))(i)
    }

    assert_eq!(f(&a[..]), Ok((&a[6..], &a[..])));
    assert_eq!(f(&b[..]), Ok((&b[4..], &b[..4])));
  }

  #[test]
  fn utf8_indexing() {
    fn dot(i: &str) -> IResult<&str, &str> {
      tag(".")(i)
    }

    let _ = dot("點");
  }

  #[cfg(feature = "alloc")]
  #[test]
  fn case_insensitive() {
    fn test(i: &str) -> IResult<&str, &str> {
      tag_no_case("ABcd")(i)
    }
    assert_eq!(test("aBCdefgh"), Ok(("efgh", "aBCd")));
    assert_eq!(test("abcdefgh"), Ok(("efgh", "abcd")));
    assert_eq!(test("ABCDefgh"), Ok(("efgh", "ABCD")));
  }
}
