import { app } from 'electron'
import path from 'path'
import isDev from 'electron-is-dev'

export const MODULES_DIR = path.join(isDev ? app.getAppPath() : path.dirname(process.resourcesPath), 'modules')
