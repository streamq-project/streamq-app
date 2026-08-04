import type { WidgetOptions } from '../../../shared/ipc-types'
import { MainWindow } from '../../app/Main'
import { config } from '../../config'
import { WidgetWindow } from './WidgetWindow'

export class WidgetsManager {
  private windows = new Map<number, WidgetWindow>()
  private readonly allowedUrl = new URL(config.url)

  constructor(private mainWindow: MainWindow) {}

  create(url: string, options: WidgetOptions): number {
    const widgetUrl = new URL(url)
    if (widgetUrl.protocol !== this.allowedUrl.protocol || widgetUrl.origin !== this.allowedUrl.origin) {
      throw new Error('Widget URL must use the configured protocol and origin')
    }

    const widget = new WidgetWindow(url, options, this.mainWindow)

    this.windows.set(widget.id, widget)
    widget.onClosed(() => this.windows.delete(widget.id))
    return widget.id
  }

  remove(id: number): void {
    this.windows.get(id)?.destroy()
  }

  destroyAll(): void {
    this.windows.forEach(window => window.destroy())
    this.windows.clear()
  }
}
