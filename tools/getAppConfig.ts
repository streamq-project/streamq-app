import fs from 'fs'
import path from 'path'
import yaml from 'yaml'
import type { AppConfig } from '../src/shared/appConfig'

export type { AppConfig }

export const getAppConfig = (): AppConfig => {
  const isProd = process.argv.includes('--prod')
  const root = path.join(__dirname, '..')
  const localPath = path.join(root, 'config.local.yml')
  const configPath =
    !isProd && fs.existsSync(localPath)
      ? localPath
      : path.join(root, `config.${isProd ? 'prod' : 'dev'}.yml`)
  return yaml.parse(fs.readFileSync(configPath, 'utf-8')) as AppConfig
}
