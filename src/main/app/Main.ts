import { app, screen, shell } from 'electron'
import path from 'path'
import windowStateKeeper from 'electron-window-state'
import { AcrylicBrowserWindow } from '../windows/AcrylicBrowserWindow'
import { config } from '../config'
import { logger } from '../utils/Logger'

import { BaseIpcController } from '../ipc/BaseIpcController'
import { AppIpcController } from '../ipc/controllers/AppIpcController'
import { WindowIpcController } from '../ipc/controllers/WindowIpcController'
import { YoutubeIpcController } from '../ipc/controllers/YoutubeIpcController'
import { ProxyIpcController } from '../ipc/controllers/ProxyIpcController'
import { InputIpcController } from '../ipc/controllers/InputIpcController'
import { MediaIpcController } from '../ipc/controllers/MediaIpcController'
import { WidgetsIpcController } from '../ipc/controllers/WidgetsIpcController'

type MainWindowHooks = {
  onStarting: () => void
  onReady: () => void
}

export class MainWindow {
  private log = logger.child('MainWindow')
  window: AcrylicBrowserWindow
  private hooks: MainWindowHooks
  isLoaded = false
  isMaximized = false

  private ipcControllers: BaseIpcController[] = []

  instanceEvents: Record<string, (args: Record<string, string>) => void> = {
    auth: ({ code }: { code: string }) => this.window.webContents.send('app:auth', code)
  }

  constructor(hooks: MainWindowHooks) {
    this.hooks = hooks
  }

  async init() {
    this.hooks.onStarting()
    const height = ~~Math.min(800, screen.getPrimaryDisplay().workAreaSize.height / 1.2)
    const { isMaximized, ...windowState } = windowStateKeeper({
      defaultWidth: ~~Math.min(height * 1.8, screen.getPrimaryDisplay().workAreaSize.width - 20),
      defaultHeight: height,
      maximize: false
    })
    this.isMaximized = isMaximized

    this.window = new AcrylicBrowserWindow({
      ...windowState,
      minWidth: 420,
      minHeight: 580,
      autoHideMenuBar: true,
      show: false,
      webPreferences: {
        preload: path.join(app.getAppPath(), '.vite', 'build', 'main-preload.js')
      }
    })

    this.ipcControllers = [
      new AppIpcController(this),
      new WindowIpcController(this),
      new YoutubeIpcController(this),
      new ProxyIpcController(this),
      new InputIpcController(this),
      new MediaIpcController(this),
      new WidgetsIpcController(this)
    ]


    app.on('before-quit', () => {
      this.ipcControllers.forEach(controller => controller.destroy())
    })

    windowState.manage(this.window)

    this.window.webContents.setWindowOpenHandler(details => {
      if (details.features.includes('internal=true')) return { action: 'allow' }
      shell.openExternal(details.url)
      return { action: 'deny' }
    })

    this.window.webContents.on('did-navigate', (_, __, statusCode) => statusCode === 200 && (this.isLoaded = true))

    app.on('second-instance', (_, argv) => {
      if (!this.window) return
      if (this.window.isMinimized()) this.window.restore()
      this.window.focus()

      const lastArg = argv.at(-1)
      if (lastArg?.startsWith('streamq://')) {
        const url = new URL(lastArg)
        const params = Object.fromEntries([...url.searchParams.entries()])
        this.log.info(`[second-instance] hostname: ${url.hostname} | params: ${{ ...params, ...params.code ? { code: '[hidden]' } : {} }}`)
        this.instanceEvents[url.hostname as keyof typeof this.instanceEvents]?.(params)
      }
    })

    this.load()
  }

  load() {
    this.window.loadURL(config.url, { extraHeaders: 'Cache-Control: no-cache' })
      .then(async () => {
        if (!this.isLoaded) return this.retry()
        this.start()
      })
      .catch(async e => {
        this.log.error('URL loading error', e)
        this.retry()
      })
  }

  start() {
    this.window.on('maximize', () => this.window.webContents.send('window:updateIsMaximized', true))
    this.window.on('unmaximize', () => this.window.webContents.send('window:updateIsMaximized', false))
    this.hooks.onReady()
    this.window.show()
    if (this.isMaximized) this.window.maximize()
  }

  async retry() {
    setTimeout(() => this.load(), 5000)
  }
}
