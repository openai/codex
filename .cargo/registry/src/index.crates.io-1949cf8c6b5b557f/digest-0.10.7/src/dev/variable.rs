use crate::{VariableOutput, VariableOutputReset};
use core::fmt::Debug;

/// Variable-output resettable digest test
pub fn variable_reset_test<D>(input: &[u8], output: &[u8]) -> Option<&'static str>
where
    D: VariableOutputReset + Debug + Clone,
{
    let mut hasher = D::new(output.len()).unwrap();
    let mut buf = [0u8; 128];
    let buf = &mut buf[..output.len()];
    // Test that it works when accepting the message all at once
    hasher.update(input);
    let mut hasher2 = hasher.clone();
    hasher.finalize_variable(buf).unwrap();
    if buf != output {
        return Some("whole message");
    }
    buf.iter_mut().for_each(|b| *b = 0);

    // Test if reset works correctly
    hasher2.reset();
    hasher2.update(input);
    hasher2.finalize_variable_reset(buf).unwrap();
    if buf != output {
        return Some("whole message after reset");
    }
    buf.iter_mut().for_each(|b| *b = 0);

    // Test that it works when accepting the message in chunks
    for n in 1..core::cmp::min(17, input.len()) {
        let mut hasher = D::new(output.len()).unwrap();
        for chunk in input.chunks(n) {
            hasher.update(chunk);
            hasher2.update(chunk);
        }
        hasher.finalize_variable(buf).unwrap();
        if buf != output {
            return Some("message in chunks");
        }
        buf.iter_mut().for_each(|b| *b = 0);

        hasher2.finalize_variable_reset(buf).unwrap();
        if buf != output {
            return Some("message in chunks");
        }
        buf.iter_mut().for_each(|b| *b = 0);
    }

    None
}

/// Variable-output resettable digest test
pub fn variable_test<D>(input: &[u8], output: &[u8]) -> Option<&'static str>
where
    D: VariableOutput + Debug + Clone,
{
    let mut hasher = D::new(output.len()).unwrap();
    let mut buf = [0u8; 128];
    let buf = &mut buf[..output.len()];
    // Test that it works when accepting the message all at once
    hasher.update(input);
    hasher.finalize_variable(buf).unwrap();
    if buf != output {
        return Some("whole message");
    }
    buf.iter_mut().for_each(|b| *b = 0);

    // Test that it works when accepting the message in chunks
    for n in 1..core::cmp::min(17, input.len()) {
        let mut hasher = D::new(output.len()).unwrap();
        for chunk in input.chunks(n) {
            hasher.update(chunk);
        }
        hasher.finalize_variable(buf).unwrap();
        if buf != output {
            return Some("message in chunks");
        }
        buf.iter_mut().for_each(|b| *b = 0);
    }
    None
}
