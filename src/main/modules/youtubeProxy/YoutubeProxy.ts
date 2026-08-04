import { BrowserWindow } from 'electron'
import { MODULES_DIR } from '../../constants'
import { Xray } from './Xray'
import { YoutubeProxyConnection } from './YoutubeProxyConnection'
import path from 'path'
import fs from 'fs/promises'

type Connection<K extends keyof YoutubeProxyConnection> = { start: (cn: YoutubeProxyConnection[K]) => Promise<void>, stop: () => void }

export class YoutubeProxy {
  private connection: Connection<keyof YoutubeProxyConnection> | null = null
  private methods: { [key in keyof YoutubeProxyConnection]?: new () => Connection<key> } = {
    xray: Xray
  } as const
  constructor(private window: BrowserWindow) {}
  async set<K extends keyof YoutubeProxyConnection>(connection: YoutubeProxyConnection[K]) {
    await this.stop()
    if (connection.method === 'direct') return
    if (connection.method in this.methods) {
      const cn = new this.methods[connection.method as K]!
      await cn.start(connection)
      this.connection = cn
    }
    await this.window.webContents.session.setProxy({
      pacScript: 'data:text/plain;base64,' + Buffer.from(
        Buffer.from(await fs.readFile(path.join(MODULES_DIR, 'proxy', 'proxy.pac'), 'utf8'), 'utf8')
          .toString('utf8')
          .replace('{HOST}', 'host' in connection ? connection.host : '127.0.0.1')
          .replace('{PORT}', String(connection.port))
      ).toString('base64')
    })
  }
  async stop() {
    if (!this.window.isDestroyed()) {
      await this.window.webContents.session.setProxy({})
    }
    this.connection?.stop()
    this.connection = null
  }
}
