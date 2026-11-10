/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        background: "#faf9f5",
        surface: {
          DEFAULT: "#f5f4ee",
          header: "#f7f6f2",
          user: "#ede9de",
          dropdown: "#e9e6dc",
          hover: "#eeebe2",
        },
        border: {
          DEFAULT: "#dad9d4",
          subtle: "#e6e4de",
        },
        text: {
          DEFAULT: "#141413",
          body: "#3d3929",
          secondary: "#535146",
          muted: "#83827d",
        },
        accent: {
          DEFAULT: "#c96442",
          hover: "#b5523f",
          dark: "#b05730",
          contrast: "#ffffff",
        },
        success: "#5f8a5a",
        danger: "#b5523f",
      },
      fontFamily: {
        serif: ['"Newsreader"', 'Georgia', 'Cambria', '"Times New Roman"', 'Times', 'serif'],
        sans: ['"Inter"', 'system-ui', '-apple-system', 'BlinkMacSystemFont', '"Segoe UI"', 'Roboto', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
      },
    },
  },
  plugins: [],
}
