import { BaseIpcController } from '../BaseIpcController'
import type { MainWindow } from '../../app/Main'
import { WidgetsManager } from '../../modules/widgets/WidgetsManager'

export class WidgetsIpcController extends BaseIpcController {
  private manager: WidgetsManager

  constructor(private mainWindow: MainWindow) {
    super()
    this.manager = new WidgetsManager(mainWindow)

    this.defineInvoke('widget:create', (_, url, options) => this.manager.create(url, options))
    this.defineInvoke('widget:remove', (_, id) => this.manager.remove(id))
    this.defineInvoke('widget:destroyAll', () => this.manager.destroyAll())
    this.mainWindow.window.once('closed', this.destroyWindows)
  }

  private destroyWindows = (): void => {
    this.manager.destroyAll()
  }

  public override destroy(): void {
    this.mainWindow.window.off('closed', this.destroyWindows)
    this.destroyWindows()
    super.destroy()
  }
}
