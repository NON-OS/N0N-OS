import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc"; // or '@vitejs/plugin-react'
import path from "path";

export default defineConfig(({ mode }) => ({
  base: './',
  server: {
    host: "0.0.0.0",
    port: 8080,
  },
  plugins: [
    react(),
  ],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
}));
