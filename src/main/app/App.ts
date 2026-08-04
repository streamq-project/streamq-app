import { app, BrowserWindow, net, protocol } from 'electron'
import os from 'os'
import path from 'path'
import fs from 'fs'
import { BootstrapWindow } from './Bootstrap'
import { MainWindow } from './Main'
import { settings } from '../settings'
import { native } from '../napi'
import { exec } from 'child_process'
import { logger } from '../utils/Logger'
import { pathToFileURL } from 'url'

export class App {
  private bootstrap: BootstrapWindow | null = null
  private main: MainWindow | null = null

  boot() {
    this.configure()

    if (!app.requestSingleInstanceLock()) {
      return app.quit()
    }

    app.on('ready', () => this.start())

    app.on('window-all-closed', () => {
      native.cleanup()
      if (process.platform !== 'darwin') app.quit()
    })

    app.on('activate', () => {
      if (BrowserWindow.getAllWindows().length === 0) this.start()
    })
  }

  private configure() {
    logger.info('Application is starting...')

    app.name = 'streamq'
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ;(app as any).setDesktopName('io.streamq.StreamQ.desktop')

    app.commandLine.appendSwitch('class', 'io.streamq.StreamQ')
    app.commandLine.appendSwitch('disable-site-isolation-trials')

    const disabledFeatures: string[] = []

    if (os.platform() === 'linux') {
      app.commandLine.appendSwitch('enable-blink-features', 'MiddleClickAutoscroll')
      disabledFeatures.push('WaylandWpColorManagerV1')

      // Keep WebKitGTK on its accelerated DMA-BUF path while avoiding the
      // NVIDIA explicit-sync protocol error on Wayland.
      // https://bugs.webkit.org/show_bug.cgi?id=280210
      if (process.env.WAYLAND_DISPLAY && process.env.__NV_DISABLE_EXPLICIT_SYNC === undefined) {
        process.env.__NV_DISABLE_EXPLICIT_SYNC = '1'
      }

      this.setupAppImageDesktopIntegration()
    }

    if (os.platform() === 'linux' || !settings.data.systemMediaControlsSession) {
      disabledFeatures.push('HardwareMediaKeyHandling', 'MediaSessionService')
    }

    if (disabledFeatures.length > 0) {
      app.commandLine.appendSwitch('disable-features', disabledFeatures.join(','))
    }

    if (process.defaultApp) {
      if (process.argv.length >= 2) {
        app.setAsDefaultProtocolClient('streamq', process.execPath, [path.resolve(process.argv[1])])
      }
    } else {
      app.setAsDefaultProtocolClient('streamq')
    }

    protocol.registerSchemesAsPrivileged([
      {
        scheme: 'streamq-local',
        privileges: { standard: true, secure: true, supportFetchAPI: true, bypassCSP: true, corsEnabled: true }
      }
    ])
  }

  private start() {
    protocol.handle('streamq-local', request => {
      // streamq-local://thumbs/{id}{ext} → {tmpdir}/streamq/thumb_{id}{ext}
      let url: URL
      try {
        url = new URL(request.url)
      } catch {
        return new Response('Access denied: Invalid URL', { status: 400 })
      }

      if (url.hostname !== 'thumbs') {
        return new Response('Access denied: Invalid host', { status: 403 })
      }

      const name = decodeURIComponent(url.pathname.replace(/^\/+|\/+$/g, ''))

      if (!name || name.includes('/') || name.includes('\\') || name.includes('..')) {
        return new Response('Access denied: Invalid filename', { status: 403 })
      }

      const fileName = `thumb_${name}`
      const tempDir = path.resolve(os.tmpdir(), 'streamq')
      const absolutePath = path.resolve(tempDir, fileName)

      if (!absolutePath.startsWith(tempDir + path.sep)) {
        return new Response('Access denied: Invalid path', { status: 403 })
      }

      return net.fetch(pathToFileURL(absolutePath).toString())
    })

    this.bootstrap = new BootstrapWindow({
      onReady: () => this.main?.init()
    })

    this.main = new MainWindow({
      onStarting: () => this.bootstrap?.setStatus('starting'),
      onReady: () => this.bootstrap?.close()
    })

    this.bootstrap.init()
  }

  private setupAppImageDesktopIntegration() {
    if (!process.env.APPIMAGE) return

    const appImagePath = process.env.APPIMAGE
    const desktopFilePath = path.join(os.homedir(), '.local', 'share', 'applications', 'io.streamq.StreamQ.desktop')

    const desktopFileContent = [
      '[Desktop Entry]',
      'Name=StreamQ',
      `Exec="${appImagePath}" %U`,
      'Terminal=false',
      'Type=Application',
      'Icon=streamq',
      'StartupWMClass=io.streamq.StreamQ',
      'Categories=AudioVideo;Music;',
      'MimeType=x-scheme-handler/streamq;'
    ].join('\n')

    try {
      if (fs.existsSync(desktopFilePath)) {
        const currentContent = fs.readFileSync(desktopFilePath, 'utf-8')
        if (currentContent === desktopFileContent) return
      }

      fs.mkdirSync(path.dirname(desktopFilePath), { recursive: true })
      fs.writeFileSync(desktopFilePath, desktopFileContent, 'utf-8')

      exec(`update-desktop-database "${path.dirname(desktopFilePath)}"`)
      logger.info('AppImage desktop integration updated successfully.')
    } catch (e) {
      logger.error('Failed to setup AppImage desktop integration', e)
    }
  }
}
