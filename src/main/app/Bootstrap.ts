import { app } from 'electron'
import { AcrylicBrowserWindow } from '../windows/AcrylicBrowserWindow'
import isDev from 'electron-is-dev'
import path from 'path'
import { AppUpdater, LoadingStatus } from '../modules/appUpdater/AppUpdater'
import { BootstrapIpcController } from '../ipc/controllers/BootstrapIpcController'

const preloadPath = () => path.join(app.getAppPath(), '.vite', 'build', 'bootstrap-preload.js')

type BootstrapWindowHooks = {
  onReady: () => void
}

export class BootstrapWindow {
  private window: AcrylicBrowserWindow | null = null
  private ipcController: BootstrapIpcController

  constructor(private hooks: BootstrapWindowHooks) {
    this.ipcController = new BootstrapIpcController(this)
  }

  init() {
    this.window = new AcrylicBrowserWindow({
      width: 440,
      height: 200,
      maxWidth: 440,
      maxHeight: 200,
      autoHideMenuBar: true,
      show: false,
      resizable: isDev,
      webPreferences: {
        preload: preloadPath()
      }
    })


    if (BOOTSTRAP_WINDOW_VITE_DEV_SERVER_URL) {
      this.window.loadURL(`${BOOTSTRAP_WINDOW_VITE_DEV_SERVER_URL}/index.html`)
        .then(() => this.window!.show())
    } else {
      const rendererPath = path.join(__dirname, `../renderer/${BOOTSTRAP_WINDOW_VITE_NAME}/index.html`)
      this.window.loadFile(rendererPath)
        .then(() => this.window!.show())
    }
  }

  setStatus(status: LoadingStatus) {
    this.window?.webContents.send('bootstrap:status', status)
  }

  async update() {
    const updater = new AppUpdater({
      onStatus: (status: LoadingStatus) => this.setStatus(status),
      onProgress: (p: number) => this.window?.webContents.send('bootstrap:progress', p)
    })
    await updater.run()
    this.hooks.onReady()
  }

  close() {
    this.ipcController.destroy()
    this.window?.close()
  }
}