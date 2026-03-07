/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'brand-primary': '#3A7BFF',
        'dark-bg': '#0F0F1E',
        'dark-surface': '#1E1E2E',
        'dark-surface-alt': '#2A2A3E',
        'dark-text': '#CDD6F4',
        'dark-text-secondary': '#A6ADC8',
        'dark-border': '#45475A',
      },
      fontFamily: {
        sans: ['Arial', 'Segoe UI', 'sans-serif'],
      },
    },
  },
  darkMode: 'class',
  plugins: [],
}
