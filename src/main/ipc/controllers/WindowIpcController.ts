import { BaseIpcController } from '../BaseIpcController'
import type { MainWindow } from '../../app/Main'
import { settings } from '../../settings'

export class WindowIpcController extends BaseIpcController {
  constructor(private mainWindow: MainWindow) {
    super()

    this.defineInvoke('window:minimize', () => this.mainWindow.window.minimize())
    this.defineInvoke('window:unmaximize', () => this.mainWindow.window.unmaximize())
    this.defineInvoke('window:maximize', () => this.mainWindow.window.maximize())
    this.defineInvoke('window:setAcrylic', (_, enable) => {
      settings.set('acrylic', enable)
      this.mainWindow.window.setAcrylic(enable)
    })
  }
}
