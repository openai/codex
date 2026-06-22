use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::io::IsTerminal;
use std::io::Write;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_protocol::ToolRequestUserInputAnswer;
use codex_app_server_protocol::ToolRequestUserInputParams;
use codex_app_server_protocol::ToolRequestUserInputResponse;

pub(super) fn prompt_for_answers(
    params: &ToolRequestUserInputParams,
) -> Result<ToolRequestUserInputResponse> {
    let stdin = io::stdin();
    if !stdin.is_terminal() {
        bail!("request_user_input requires an interactive stdin terminal");
    }

    let stdout = io::stdout();
    prompt_for_answers_with(&mut stdin.lock(), &mut stdout.lock(), params)
}

fn prompt_for_answers_with(
    input: &mut impl BufRead,
    output: &mut impl Write,
    params: &ToolRequestUserInputParams,
) -> Result<ToolRequestUserInputResponse> {
    writeln!(
        output,
        "\n[request_user_input for thread {}, turn {}]",
        params.thread_id, params.turn_id
    )?;
    if let Some(auto_resolution_ms) = params.auto_resolution_ms {
        writeln!(
            output,
            "The app-server may auto-resolve this request after {auto_resolution_ms} ms."
        )?;
    }

    let mut answers = HashMap::new();
    for question in &params.questions {
        writeln!(output, "\n{}: {}", question.header, question.question)?;
        let options = question
            .options
            .as_deref()
            .filter(|options| !options.is_empty());
        let answer_values = if let Some(options) = options {
            for (index, option) in options.iter().enumerate() {
                writeln!(
                    output,
                    "  {}. {} - {}",
                    index + 1,
                    option.label,
                    option.description
                )?;
            }
            if question.is_other {
                writeln!(output, "  o. Other (free-form)")?;
            }

            loop {
                if question.is_other {
                    write!(output, "Choose 1-{} or o: ", options.len())?;
                } else {
                    write!(output, "Choose 1-{}: ", options.len())?;
                }
                output.flush()?;

                let mut line = String::new();
                if input
                    .read_line(&mut line)
                    .context("failed to read request_user_input selection")?
                    == 0
                {
                    bail!("stdin closed while waiting for request_user_input selection");
                }
                let selection = line.trim();

                if let Ok(index) = selection.parse::<usize>()
                    && let Some(option) = index.checked_sub(1).and_then(|index| options.get(index))
                {
                    break vec![option.label.clone()];
                }

                if let Some(option) = options
                    .iter()
                    .find(|option| option.label.eq_ignore_ascii_case(selection))
                {
                    break vec![option.label.clone()];
                }

                if question.is_other && selection.eq_ignore_ascii_case("o") {
                    write!(output, "Other: ")?;
                    output.flush()?;
                    line.clear();
                    if input
                        .read_line(&mut line)
                        .context("failed to read request_user_input free-form answer")?
                        == 0
                    {
                        bail!("stdin closed while waiting for request_user_input free-form answer");
                    }
                    let answer = line.trim();
                    if !answer.is_empty() {
                        break vec![format!("user_note: {answer}")];
                    }
                }

                writeln!(output, "Invalid selection; try again.")?;
            }
        } else {
            loop {
                write!(output, "Answer: ")?;
                output.flush()?;

                let mut line = String::new();
                if input
                    .read_line(&mut line)
                    .context("failed to read request_user_input answer")?
                    == 0
                {
                    bail!("stdin closed while waiting for request_user_input answer");
                }
                let answer = line.trim();
                if !answer.is_empty() {
                    break vec![format!("user_note: {answer}")];
                }
                writeln!(output, "Answer cannot be empty; try again.")?;
            }
        };

        answers.insert(
            question.id.clone(),
            ToolRequestUserInputAnswer {
                answers: answer_values,
            },
        );
    }

    Ok(ToolRequestUserInputResponse { answers })
}

#[cfg(test)]
#[path = "request_user_input_tests.rs"]
mod tests;
