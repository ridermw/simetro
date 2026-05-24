import js from "@eslint/js";
import tseslint from "@typescript-eslint/eslint-plugin";
import tsparser from "@typescript-eslint/parser";
import noUnsanitized from "eslint-plugin-no-unsanitized";

// PLAN §5.1 / §12 — JSON-derived UI strings MUST flow through
// `textContent`, never `innerHTML`. eslint-plugin-no-unsanitized
// enforces this at lint time.
export default [
  {
    ignores: ["dist", "node_modules", "playwright-report"],
  },
  js.configs.recommended,
  {
    files: ["src/**/*.ts", "*.ts"],
    languageOptions: {
      parser: tsparser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: "module",
      },
      globals: {
        window: "readonly",
        document: "readonly",
        navigator: "readonly",
        console: "readonly",
        HTMLElement: "readonly",
        HTMLCanvasElement: "readonly",
        CanvasRenderingContext2D: "readonly",
        Path2D: "readonly",
        AudioContext: "readonly",
        requestAnimationFrame: "readonly",
        cancelAnimationFrame: "readonly",
        URLSearchParams: "readonly",
        performance: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tseslint,
      "no-unsanitized": noUnsanitized,
    },
    rules: {
      // Banned: innerHTML, outerHTML, document.write, insertAdjacentHTML.
      "no-unsanitized/method": "error",
      "no-unsanitized/property": "error",
      // Base rule flags param NAMES inside TS function-type aliases as
      // unused; the @typescript-eslint version below is type-aware and
      // correct.
      "no-unused-vars": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
      "@typescript-eslint/no-explicit-any": "warn",
      "no-undef": "off",
      "no-console": "off",
    },
  },
];
