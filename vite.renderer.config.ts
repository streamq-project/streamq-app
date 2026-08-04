import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import ViteYaml from '@modyfi/vite-plugin-yaml'

// https://vitejs.dev/config
export default defineConfig({
  plugins: [react(), ViteYaml()],
  root: 'src/app_bootstrap',
  publicDir: '../../static',
  base: './',
  build: {
    outDir: '../../.vite/renderer/bootstrap_window',
    rollupOptions: {
      input: 'src/app_bootstrap/index.html',
    },
  },
});
