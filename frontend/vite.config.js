import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  build: {
    outDir: '../src/backtide/ui/static',
    emptyOutDir: true,
    sourcemap: false
  },
  server: {
    proxy: {
      '/api': 'http://localhost:8501'
    }
  }
})
