import 'source-map-support/register'
import packageJson from '../../package.json'
import banner from './banner.txt?raw'
import { initSentry } from './utils/sentry'

console.log(`\n${banner}\n Version: ${packageJson.version}\n`)
initSentry()

import { App } from './app/App'
new App().boot()
