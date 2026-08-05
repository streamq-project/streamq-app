import { BrowserWindowConstructorOptions, screen } from 'electron'
import { native } from '../napi'
import os from 'os'
import { settings } from '../settings'
import { BaseBrowserWindow } from './BaseBrowserWindow'

export class AcrylicBrowserWindow extends BaseBrowserWindow {
  lastResize = 0
  constructor(options: BrowserWindowConstructorOptions) {
    super({
      ...options,
      frame: false,
      thickFrame: os.platform() !== 'win32',
      transparent: os.platform() === 'linux',
      backgroundColor: '#00000000'
    })
    if (os.platform() === 'win32') {
      const hwnd = this.getNativeWindowHandle().readInt32LE(0)

      // https://github.com/electron/electron/issues/51679
      native.window.restoreNativeFrame(hwnd)

      native.window.setAcrylic(hwnd, settings.data.acrylic, [0, 0, 0, 0])
      native.window.disableRounds(hwnd)

      // Win10 workaround
      // https://github.com/Seo-Rii/electron-acrylic-window/issues/6
      const frameTime = () => 1000 / screen.getDisplayMatching(this.getBounds()).displayFrequency
      this.on('will-move', () => native.sleep(frameTime()))
      this.on('will-resize', (e) => {
        if (this.lastResize >= Date.now() - frameTime() * 2) e.preventDefault()
        else this.lastResize = Date.now()
      })
    }
  }

  setAcrylic(enable: boolean) {
    if (os.platform() === 'win32') {
      const hwnd = this.getNativeWindowHandle().readInt32LE(0)
      native.window.setAcrylic(hwnd, enable, [0, 0, 0, 0])
    }
  }
}
