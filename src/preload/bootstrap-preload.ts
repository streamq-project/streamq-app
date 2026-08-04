import { contextBridge } from 'electron'
import { createInvoker, createSubscriber, sendSync } from './utils'

const api = {
  bootstrap: {
    init: () => sendSync('bootstrap:init'),
    ready: createInvoker('bootstrap:ready'),
    onStatus: createSubscriber('bootstrap:status'),
    onProgress: createSubscriber('bootstrap:progress'),
  }
}

contextBridge.exposeInMainWorld('appAPI', api)

export type BootstrapAPI = typeof api
