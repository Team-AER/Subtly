const { defineConfig } = require('vite');
const react = require('@vitejs/plugin-react');
const path = require('path');

module.exports = defineConfig(({ mode }) => ({
  root: __dirname,
  plugins: [react()],
  base: './',
  resolve: {
    alias: {
      '@': path.join(__dirname, 'src', 'renderer')
    }
  },
  build: {
    outDir: path.join(__dirname, 'dist', 'renderer'),
    emptyOutDir: false,
    sourcemap: mode !== 'production'
  }
}));
