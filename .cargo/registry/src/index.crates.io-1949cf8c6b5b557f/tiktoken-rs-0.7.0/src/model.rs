/*!
 * contains information about OpenAI models.
 */

/// Macro to check if a given str starts with any of the specified prefixes.
macro_rules! starts_with_any {
    ($str:expr, $($prefix:expr),* $(,)?) => {
        false $(|| $str.starts_with($prefix))*
    };
}

/// Returns the context size of a specified model.
///
/// The context size represents the maximum number of tokens a model can process in a single input.
/// This function checks the model name and returns the corresponding context size.
/// See <https://platform.openai.com/docs/models> for up-to-date information.
///
/// # Arguments
///
/// * `model` - A string slice that holds the name of the model.
///
/// # Examples
///
/// ```
/// use tiktoken_rs::model::get_context_size;
/// let model = "gpt-4-32k";
/// let context_size = get_context_size(model);
/// assert_eq!(context_size, 32768);
/// ```
///
/// # Panics
///
/// This function does not panic. It returns a default value of 4096 if the model is not recognized.
pub fn get_context_size(model: &str) -> usize {
    if starts_with_any!(model, "o1-") {
        return 128_000;
    }
    if starts_with_any!(model, "gpt-4o") {
        return 128_000;
    }
    if starts_with_any!(model, "gpt-4-turbo-") {
        return 128_000;
    }
    if starts_with_any!(model, "gpt-4-0125") {
        return 128_000;
    }
    if starts_with_any!(model, "gpt-4-1106") {
        return 128_000;
    }
    if starts_with_any!(model, "gpt-4-32k") {
        return 32_768;
    }
    if starts_with_any!(model, "gpt-4") {
        return 8192;
    }
    if starts_with_any!(model, "gpt-3.5-turbo-0125") {
        return 16_385;
    }
    if starts_with_any!(model, "gpt-3.5-turbo-1106") {
        return 16_385;
    }
    if starts_with_any!(model, "gpt-3.5-turbo-16k") {
        return 16_385;
    }
    if starts_with_any!(model, "gpt-3.5-turbo") {
        return 16_385;
    }
    if starts_with_any!(model, "text-davinci-002", "text-davinci-003") {
        return 4097;
    }
    if starts_with_any!(model, "ada", "babbage", "curie") {
        return 2049;
    }
    if starts_with_any!(model, "code-cushman-001") {
        return 2048;
    }
    if starts_with_any!(model, "code-davinci-002") {
        return 8001;
    }
    if starts_with_any!(model, "davinci") {
        return 2049;
    }
    if starts_with_any!(model, "text-ada-001", "text-babbage-001", "text-curie-001") {
        return 2049;
    }
    if starts_with_any!(model, "text-embedding-ada-002") {
        return 8192;
    }
    4096
}
