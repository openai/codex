import { FormEvent, KeyboardEvent, useState } from "react";

interface ComposerProps {
  disabled: boolean;
  onSubmit: (text: string) => void;
}

export function Composer({ disabled, onSubmit }: ComposerProps) {
  const [value, setValue] = useState("");

  function submit(event?: FormEvent) {
    event?.preventDefault();
    const text = value.trim();
    if (text.length === 0) {
      return;
    }
    onSubmit(text);
    setValue("");
  }

  return (
    <form className="composer" onSubmit={submit}>
      <textarea
        disabled={disabled}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event: KeyboardEvent<HTMLTextAreaElement>) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            submit();
          }
        }}
        placeholder="Send a message to Codex"
        value={value}
      />
      <div className="composer__actions">
        <p>
          Enter to send, Shift+Enter for newline
          {disabled ? " · waiting for connection" : ""}
        </p>
        <button className="solid-btn" disabled={disabled} type="submit">
          Send
        </button>
      </div>
    </form>
  );
}
