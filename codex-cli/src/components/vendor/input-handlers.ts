import { useInput } from "ink";

export type InputHandler = {
  /**
   * The handler function to supply to `useInput`.
   */
  handler: Parameters<typeof useInput>[0]
  /**
   * Output rendered for the user.
   */
  output: string | undefined
}

export type TextInputProps = {
  /**
   * Text to display when `value` is empty.
   */
  readonly placeholder?: string;

  /**
   * Listen to user's input. Useful in case there are multiple input components
   * at the same time and input must be "routed" to a specific component.
   */
  readonly focus?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Replace all chars and mask the value. Useful for password inputs.
   */
  readonly mask?: string;

  /**
   * Whether to show cursor and allow navigation inside text input with arrow keys.
   */
  readonly showCursor?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Highlight pasted text
   */
  readonly highlightPastedText?: boolean; // eslint-disable-line react/boolean-prop-naming

  /**
   * Value to display in a text input.
   */
  readonly value: string;

  /**
   * Function to call when value updates.
   */
  readonly onChange: (value: string) => void;

  /**
   * Function to call when `Enter` is pressed, where first argument is a value of the input.
   */
  readonly onSubmit?: (value: string) => void;

  /**
   * Explicitly set the cursor position to the end of the text
   */
  readonly cursorToEnd?: boolean;
};
