import * as vscode from "vscode";

export type UserInputQuestionOption = {
  label: string;
  description: string;
};

export type UserInputQuestion = {
  id: string;
  header: string;
  question: string;
  allowMultiple: boolean;
  isOther: boolean;
  isSecret: boolean;
  options: UserInputQuestionOption[] | null;
};

export type UserInputAnswersById = Record<string, string[]>;
export type UserInputPromptResult = {
  cancelled: boolean;
  answersById: UserInputAnswersById;
};

function normalizeAnswers(values: Array<string | null | undefined>): string[] {
  return values.map((v) => String(v ?? "").trim()).filter(Boolean);
}

export async function promptRequestUserInput(args: {
  title: string;
  questions: UserInputQuestion[];
}): Promise<UserInputPromptResult> {
  const answersById: UserInputAnswersById = {};
  for (const q of args.questions) {
    const hasOptions = Array.isArray(q.options) && q.options.length > 0;
    const placeHolder =
      q.header.trim() || q.question.trim()
        ? `${q.header.trim()} — ${q.question.trim()}`.trim()
        : args.title;

    if (hasOptions) {
      const items: Array<
        vscode.QuickPickItem & { ruiKind: "option" | "other" }
      > = [
        ...(q.options ?? []).map((o) => ({
          ruiKind: "option" as const,
          label: o.label,
          description: o.description,
        })),
      ];
      if (q.isOther) {
        items.push({
          ruiKind: "other",
          label: "Other…",
          description: "Enter a custom answer",
        });
      }

      let selected: Array<
        vscode.QuickPickItem & { ruiKind: "option" | "other" }
      > = [];

      if (q.allowMultiple) {
        const pickedAny = await vscode.window.showQuickPick(items, {
          title: args.title,
          placeHolder,
          canPickMany: true,
          ignoreFocusOut: true,
          matchOnDescription: true,
        });
        if (!pickedAny) {
          return { cancelled: true, answersById };
        }
        selected = pickedAny;
      } else {
        const pickedOne = await vscode.window.showQuickPick(items, {
          title: args.title,
          placeHolder,
          ignoreFocusOut: true,
          matchOnDescription: true,
        });
        if (!pickedOne) {
          return { cancelled: true, answersById };
        }
        selected = [pickedOne];
      }

      const otherPicked = selected.some((p) => p.ruiKind === "other");
      const optionAnswers = selected
        .filter((p) => p.ruiKind === "option")
        .map((p) => p.label);

      const otherAnswer = otherPicked
        ? await vscode.window.showInputBox({
            title: args.title,
            prompt: q.header.trim() || q.question.trim() || "Other",
            placeHolder: "Enter a custom answer",
            ignoreFocusOut: true,
            password: q.isSecret,
          })
        : null;
      if (otherPicked && otherAnswer === undefined) {
        return { cancelled: true, answersById };
      }

      const finalAnswers = normalizeAnswers([
        ...optionAnswers,
        otherPicked ? otherAnswer : null,
      ]);
      answersById[q.id] = finalAnswers;
      continue;
    }

    const input = await vscode.window.showInputBox({
      title: args.title,
      prompt: q.header.trim() || q.question.trim() || args.title,
      placeHolder,
      ignoreFocusOut: true,
      password: q.isSecret,
    });
    if (input === undefined) {
      return { cancelled: true, answersById };
    }
    answersById[q.id] = normalizeAnswers([input]);
  }

  return { cancelled: false, answersById };
}
