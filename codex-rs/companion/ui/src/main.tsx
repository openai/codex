import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("Missing #root element");
}

const query = new URLSearchParams(window.location.search);
const token = query.get("token") ?? "";
const initialPrompt = query.get("prompt") ?? "";
const backend = query.get("backend");

createRoot(root).render(<App backend={backend} initialPrompt={initialPrompt} token={token} />);
