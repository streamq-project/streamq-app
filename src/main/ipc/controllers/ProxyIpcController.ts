import { dialog } from 'electron'
import { BaseIpcController } from '../BaseIpcController'
import { YoutubeProxy } from '../../modules/youtubeProxy/YoutubeProxy'
import type { MainWindow } from '../../app/Main'

export class ProxyIpcController extends BaseIpcController {
  private youtubeProxy: YoutubeProxy

  constructor(private mainWindow: MainWindow) {
    super()
    this.youtubeProxy = new YoutubeProxy(this.mainWindow.window)

    this.defineInvoke('proxy:setYoutubeConnectionMethod', async (_, connection) => {
      try {
        await this.youtubeProxy.set(connection)
      } catch (err) {
        dialog.showErrorBox('Proxy Error', String(err))
        throw err
      }
    })
  }

  public override destroy(): void {
    void this.youtubeProxy.stop()
    super.destroy()
  }
}
