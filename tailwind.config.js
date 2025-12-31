module.exports = {
  content: [
    './index.html',
    './src/renderer/**/*.{js,jsx}'
  ],
  theme: {
    extend: {
      fontFamily: {
        display: ['\"Space Grotesk\"', 'sans-serif'],
        body: ['\"IBM Plex Sans\"', 'sans-serif']
      },
      colors: {
        base: {
          950: '#0b0f1a',
          900: '#111827',
          800: '#1b1f2b'
        },
        accent: {
          500: '#f97316',
          400: '#fb923c'
        },
        plasma: {
          400: '#38bdf8'
        }
      },
      boxShadow: {
        glow: '0 12px 30px rgba(249, 115, 22, 0.35)'
      }
    }
  },
  plugins: []
};
