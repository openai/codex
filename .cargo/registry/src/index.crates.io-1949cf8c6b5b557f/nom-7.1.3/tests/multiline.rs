use nom::{
  character::complete::{alphanumeric1 as alphanumeric, line_ending as eol},
  multi::many0,
  sequence::terminated,
  IResult,
};

pub fn end_of_line(input: &str) -> IResult<&str, &str> {
  if input.is_empty() {
    Ok((input, input))
  } else {
    eol(input)
  }
}

pub fn read_line(input: &str) -> IResult<&str, &str> {
  terminated(alphanumeric, end_of_line)(input)
}

pub fn read_lines(input: &str) -> IResult<&str, Vec<&str>> {
  many0(read_line)(input)
}

#[cfg(feature = "alloc")]
#[test]
fn read_lines_test() {
  let res = Ok(("", vec!["Duck", "Dog", "Cow"]));

  assert_eq!(read_lines("Duck\nDog\nCow\n"), res);
  assert_eq!(read_lines("Duck\nDog\nCow"), res);
}
