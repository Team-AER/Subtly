const { defineConfig } = require('vitest/config');
const react = require('@vitejs/plugin-react');
const path = require('path');

module.exports = defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.join(__dirname, 'src', 'renderer')
    }
  },
  test: {
    environment: 'jsdom',
    setupFiles: [path.join(__dirname, 'tests', 'setup.js')],
    clearMocks: true,
    restoreMocks: true,
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
      include: ['src/**/*.js', 'src/**/*.jsx'],
      exclude: [
        'src/renderer/styles.css',
        'src/main/**',
        'src/renderer/App.jsx',
        'src/renderer/components/ModelManager.jsx',
        'src/renderer/components/ui/collapsible.jsx',
        'src/renderer/components/ui/progress-modal.jsx'
      ],
      thresholds: {
        lines: 100,
        functions: 100,
        branches: 100,
        statements: 100
      }
    }
  }
});
