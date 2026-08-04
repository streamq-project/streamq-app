import { app } from 'electron'
import { BaseIpcController } from '../BaseIpcController'
import { settings } from '../../settings'
import type { BootstrapWindow } from '../../app/Bootstrap'

export class BootstrapIpcController extends BaseIpcController {
  constructor(private bootstrapWindow: BootstrapWindow) {
    super()

    this.defineSync('bootstrap:init', () => ({
      version: app.getVersion(),
      settings: settings.data
    }))
    this.defineInvoke('bootstrap:ready', () => this.bootstrapWindow.update())
  }
}
