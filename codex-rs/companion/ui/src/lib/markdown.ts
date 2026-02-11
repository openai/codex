import DOMPurify from "dompurify";
import { marked } from "marked";

marked.setOptions({
  breaks: true,
  gfm: true,
});

export function renderMarkdown(markdown: string): string {
  const html = marked.parse(markdown ?? "") as string;
  return DOMPurify.sanitize(html, {
    USE_PROFILES: { html: true },
  });
}
