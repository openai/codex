import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  server: {
    host: process.env.VITE_HOST ?? "0.0.0.0",
    proxy: {
      "/api": "http://localhost:3000",
    },
  },
});
