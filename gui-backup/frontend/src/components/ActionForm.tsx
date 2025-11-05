import { useEffect, useMemo, useState } from "react";
import type { ActionMetadata } from "../types";

interface ActionFormProps {
  action: ActionMetadata;
  onSubmit: (values: Record<string, string>) => void;
  isSubmitting: boolean;
  submitLabel?: string;
}

export function ActionForm({ action, onSubmit, isSubmitting, submitLabel }: ActionFormProps) {
  const initialValues = useMemo(() => {
    const defaults: Record<string, string> = {};
    for (const field of action.fields) {
      if (field.defaultValue) {
        defaults[field.id] = field.defaultValue;
      } else {
        defaults[field.id] = "";
      }
    }
    return defaults;
  }, [action]);

  const [values, setValues] = useState<Record<string, string>>(initialValues);
  const [errors, setErrors] = useState<Record<string, string>>({});

  useEffect(() => {
    setValues(initialValues);
    setErrors({});
  }, [initialValues]);

  const validate = () => {
    const validationErrors: Record<string, string> = {};
    for (const field of action.fields) {
      const value = (values[field.id] ?? "").trim();
      if (field.required && !value) {
        validationErrors[field.id] = "This field is required";
      }
    }
    setErrors(validationErrors);
    return Object.keys(validationErrors).length === 0;
  };

  const handleSubmit = (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    if (!validate()) {
      return;
    }
    onSubmit(values);
  };

  const handleChange = (fieldId: string, value: string) => {
    setValues((prev) => ({ ...prev, [fieldId]: value }));
    if (errors[fieldId]) {
      setErrors((prev) => {
        const next = { ...prev };
        delete next[fieldId];
        return next;
      });
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
      event.preventDefault();
      if (validate()) {
        onSubmit(values);
      }
    }
  };

  return (
    <form className="action-form" onSubmit={handleSubmit}>
      {action.fields.length === 0 ? (
        <p className="action-form__empty">This action has no required inputs.</p>
      ) : (
        action.fields.map((field) => (
          <div key={field.id} className="action-form__field">
            <label className="action-form__label" htmlFor={field.id}>
              {field.label}
            </label>
            {field.kind === "text" && (
              <input
                id={field.id}
                type="text"
                value={values[field.id] ?? ""}
                placeholder={field.placeholder}
                onChange={(event) => handleChange(field.id, event.target.value)}
                className="action-form__input"
                autoComplete="off"
              />
            )}
            {field.kind === "textarea" && (
              <textarea
                id={field.id}
                value={values[field.id] ?? ""}
                placeholder={field.placeholder}
                onChange={(event) => handleChange(field.id, event.target.value)}
                onKeyDown={handleKeyDown}
                className="action-form__textarea"
                rows={6}
              />
            )}
            {field.kind === "select" && (
              <select
                id={field.id}
                value={values[field.id] ?? field.defaultValue ?? ""}
                onChange={(event) => handleChange(field.id, event.target.value)}
                className="action-form__select"
              >
                {(field.options ?? []).map((option) => (
                  <option key={option.value} value={option.value}>
                    {option.label}
                  </option>
                ))}
              </select>
            )}
            {field.helperText && <p className="action-form__helper">{field.helperText}</p>}
            {errors[field.id] && <p className="action-form__error">{errors[field.id]}</p>}
          </div>
        ))
      )}
      <button className="action-form__submit" type="submit" disabled={isSubmitting}>
        {isSubmitting ? "Running..." : submitLabel ?? action.ctaLabel}
      </button>
    </form>
  );
}
