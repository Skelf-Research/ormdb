/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'bg': '#0f0f23',
        'bg-secondary': '#1a1a2e',
        'bg-tertiary': '#16213e',
        'primary': '#00d4ff',
        'accent': '#7c3aed',
      },
    },
  },
  plugins: [],
}
