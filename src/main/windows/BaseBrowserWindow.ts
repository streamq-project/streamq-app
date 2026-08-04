import { app, BrowserWindow, BrowserWindowConstructorOptions } from 'electron'
import os from 'os'
import path from 'path'

export class BaseBrowserWindow extends BrowserWindow {
  constructor(options: BrowserWindowConstructorOptions) {
    super({
      ...options,
      icon: os.platform() === 'linux'
        ? process.env.APPDIR
          ? path.join(process.env.APPDIR, 'streamq.png')
          : path.join(app.getAppPath(), 'build', 'icon.png')
        : options.icon
    })
  }
}
