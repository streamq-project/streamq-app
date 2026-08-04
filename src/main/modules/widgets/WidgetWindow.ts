import { app, Rectangle, screen } from 'electron'
import { WidgetIpcController } from './WidgetIpcController'
import { logger } from '../../utils/Logger'
import { native } from '../../napi'
import { BaseBrowserWindow } from '../../windows/BaseBrowserWindow'
import { MainWindow } from '../../app/Main'
import { getWM } from '../../utils/wm'
import type { WidgetOptions } from '../../../shared/ipc-types'
import path from 'node:path'
import os from 'os'

const nativeWidgets = native.widgets

export class WidgetWindow {
  readonly id: number
  private readonly electronWindow: BaseBrowserWindow | null
  private readonly log = logger.child('WidgetWindow')
  private readonly ipcController: WidgetIpcController | null = null
  private readonly closedListeners = new Set<() => void>()
  private nativeOverlayId: number | null = null
  private wm = getWM()
  private isMapped = false

  get window(): BaseBrowserWindow {
    if (!this.electronWindow) throw new Error('Native overlay widgets do not have an Electron window')
    return this.electronWindow
  }

  constructor(url: string, options: WidgetOptions, private mainWindow: MainWindow) {
    const bounds: Rectangle = (() => {
      if (options.mode === 'overlay') {
        const display = screen.getAllDisplays().find(d => d.id === options.displayId)
        if (!display) throw new Error('Display not found')
        return display.bounds
      }
      if (os.platform() === 'linux') return screen.getDisplayMatching(this.mainWindow.window.getBounds()).bounds
      return this.findAutomaticBounds(1, 1)
    })()

    if (options.mode === 'overlay' && os.platform() === 'linux') {
      this.electronWindow = null
      this.nativeOverlayId = nativeWidgets.createOverlay(url, bounds)
      this.id = this.nativeOverlayId
      return
    }

    this.electronWindow = new BaseBrowserWindow({
      ...bounds,
      show: false,
      frame: false,
      transparent: true,
      backgroundColor: '#00000000',
      focusable: false,
      resizable: false,
      movable: false,
      minimizable: false,
      maximizable: false,
      fullscreenable: false,
      autoHideMenuBar: true,
      ...options.mode === 'overlay' ? {
        alwaysOnTop: true,
        skipTaskbar: true
      } : {},
      webPreferences: {
        backgroundThrottling: false,
        ...(options.mode === 'window' ? {
          preload: path.join(app.getAppPath(), '.vite', 'build', 'widget-preload.js')
        } : {})
      }
    })
    this.id = this.window.id

    if (options.mode === 'window') {
      this.ipcController = new WidgetIpcController(this)
    }
    if (options.mode === 'overlay') {
      this.window.setAlwaysOnTop(true, 'screen-saver')
      this.window.setIgnoreMouseEvents(true)
    }

    this.window.setIgnoreMouseEvents(true)
    this.window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }))
    this.window.webContents.once('did-finish-load', () => {
      if (options.mode === 'overlay') this.window.setBounds(bounds)
      this.window.showInactive()
    })
    this.window.once('closed', () => this.ipcController?.destroy())
    this.window.loadURL(url).catch(err => {
      this.log.error('Unable to load widget', err)
      if (!this.window.isDestroyed()) this.window.destroy()
    })
  }

  resize(width: number, height: number) {
    if (!this.electronWindow || this.electronWindow.isDestroyed()) return

    this.electronWindow.setBounds(this.findAutomaticBounds(width, height))

    // Tiling window managers workaround
    if (this.wm?.isTiling && !this.isMapped) {
      this.electronWindow.hide()
      this.electronWindow.show()
      this.isMapped = true
    }
  }

  private findAutomaticBounds(width: number, height: number): Rectangle {
    if (os.platform() === 'linux') return { x: 0, y: 0, width, height }

    const displays = screen.getAllDisplays()

    for (const display of displays) {
      const { x, y, width: displayWidth, height: displayHeight } = display.bounds
      const candidates: Rectangle[] = [
        { x: x - width + 1, y: y - height + 1, width, height },
        { x: x + displayWidth - 1, y: y - height + 1, width, height },
        { x: x - width + 1, y: y + displayHeight - 1, width, height },
        { x: x + displayWidth - 1, y: y + displayHeight - 1, width, height }
      ]

      const candidate = candidates.find(bounds => displays.every(otherDisplay => {
        const intersectionWidth = Math.max(0, Math.min(bounds.x + bounds.width, otherDisplay.bounds.x + otherDisplay.bounds.width) - Math.max(bounds.x, otherDisplay.bounds.x))
        const intersectionHeight = Math.max(0, Math.min(bounds.y + bounds.height, otherDisplay.bounds.y + otherDisplay.bounds.height) - Math.max(bounds.y, otherDisplay.bounds.y))
        const intersectionArea = intersectionWidth * intersectionHeight
        return otherDisplay.id === display.id ? intersectionArea === 1 : intersectionArea === 0
      }))

      if (candidate) return candidate
    }

    this.log.warn('Unable to find an off-screen widget position, using fallback')
    const { x, y } = screen.getPrimaryDisplay().bounds
    return { x: x - width, y: y - height, width, height }
  }

  onClosed(listener: () => void): void {
    if (this.electronWindow) {
      this.electronWindow.once('closed', listener)
      return
    }
    this.closedListeners.add(listener)
  }

  destroy(): void {
    if (this.nativeOverlayId !== null) {
      const id = this.nativeOverlayId
      this.nativeOverlayId = null
      try {
        nativeWidgets.destroy(id)
      } finally {
        this.closedListeners.forEach(listener => listener())
        this.closedListeners.clear()
      }
      return
    }

    this.ipcController?.destroy()
    if (this.electronWindow && !this.electronWindow.isDestroyed()) this.electronWindow.destroy()
  }
}
