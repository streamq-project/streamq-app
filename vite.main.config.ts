import { defineConfig } from 'vite'
import { getAppConfig } from './tools/getAppConfig'

const appConfig = getAppConfig()

export default defineConfig(async () => {
  const { default: checker } = await import('vite-plugin-checker');

  return {
    plugins: [
      checker({
        typescript: true,
        terminal: true,
        overlay: false,
        enableBuild: true
      }),
      {
        name: 'hot-restart',
        writeBundle() {
          if (process.env.WATCH === 'true') process.stdin.emit('data', Buffer.from('rs'))
        }
      }
    ],
    define: {
      __APP_CONFIG__: JSON.stringify(appConfig)
    },
    build: {
      sourcemap: true,
      minify: false,
      rollupOptions: {
        external: [
          /\.node$/
        ],
        output: {
          entryFileNames: 'main.js'
        }
      }
    }
  }
})
