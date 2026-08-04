import { BaseIpcController } from '../../ipc/BaseIpcController'
import type { WidgetWindow } from './WidgetWindow'

export class WidgetIpcController extends BaseIpcController {
  constructor(widget: WidgetWindow) {
    super(widget.window.webContents.ipc)

    this.defineRendererSend('widget:resize', (_, width, height) => widget.resize(width, height))
  }
}
