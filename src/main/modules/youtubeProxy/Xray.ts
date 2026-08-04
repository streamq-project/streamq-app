import { app } from 'electron'
import { spawn, ChildProcess } from 'child_process'
import { MODULES_DIR } from '../../constants'
import { YoutubeProxyConnectionXray } from './YoutubeProxyConnection'
import path from 'path'
import fs from 'fs/promises'
import { logger } from '../../utils/Logger'

const CONFIG_FILE = path.join(app.getPath('userData'), 'xray-client.json')

export class Xray {
  private log = logger.child('Xray')
  process: ChildProcess | null
  async start({ port, config }: YoutubeProxyConnectionXray) {
    await fs.writeFile(CONFIG_FILE, config.replace('{PORT}', String(port)))
    return new Promise<void>((res, rej) => {
      const onExit = (err?: unknown) => {
        if (!err) return
        this.log.error('Xray process failed', err)
        rej(err)
      }
      const xrayBinary = process.platform === 'win32' ? 'xray.exe' : 'xray'
      this.process = spawn(path.join(MODULES_DIR, 'proxy', xrayBinary), ['-c', CONFIG_FILE])
      this.process.stdout?.on('data', (data) => this.log.info(data.toString().trim()))
      this.process.stderr?.on('data', (data) => this.log.error(data.toString().trim()))
      this.process.on('spawn', res)
      this.process.on('error', onExit)
      this.process.on('exit', onExit)
    })
  }
  stop() {
    this.process?.kill('SIGTERM')
    this.process = null
    this.log.info('Stopped')
  }
}
