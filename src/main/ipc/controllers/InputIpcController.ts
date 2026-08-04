import { BaseIpcController } from '../BaseIpcController'
import { native } from '../../napi'
import type { MainWindow } from '../../app/Main'

export class InputIpcController extends BaseIpcController {
  constructor(private mainWindow: MainWindow) {
    super()

    this.defineInvoke('input:setKeybinds', (_, keybinds) => native.keybinds.setKeybinds(keybinds))
  }
}
