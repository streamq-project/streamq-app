import { BaseIpcController } from '../BaseIpcController'
import { YoutubeManager } from '../../modules/youtubeUtils/YoutubeManager'
import type { MainWindow } from '../../app/Main'

export class YoutubeIpcController extends BaseIpcController {
  private youtubeUtils: YoutubeManager

  constructor(private mainWindow: MainWindow) {
    super()
    this.youtubeUtils = new YoutubeManager(this.mainWindow.window.webContents)

    this.defineInvoke('youtube:setVideoVolume', (_, vol) => this.youtubeUtils.setVideoVolume(vol))
    this.defineInvoke('youtube:getAspectRatio', () => this.youtubeUtils.getAspectRatio())
    this.defineInvoke('youtube:setYoutubePiPMode', (_, isActive, isOnTop) => this.youtubeUtils.setPiPMode(isActive, isOnTop))
    this.defineInvoke('youtube:setYoutubePiPAlwaysOnTopMode', (_, isOnTop) => this.youtubeUtils.setPiPAlwaysOnTopMode(isOnTop))
  }
}
