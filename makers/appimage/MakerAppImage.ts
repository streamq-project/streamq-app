import path from 'node:path'
import fs from 'fs-extra'
import yaml from 'js-yaml'
import { MakerBase, MakerOptions } from '@electron-forge/maker-base'
import { ForgeArch, ForgePlatform } from '@electron-forge/shared-types'
import { Arch } from 'builder-util'
import { buildStaticRuntimeAppImage, type AppImageBuilderOptions } from 'app-builder-lib/out/targets/appimage/appImageUtil'

import { MakerAppImageConfig } from './Config'

const APPIMAGE_TOOLSET = '1.0.3' as const

export function appImageArch(nodeArch: ForgeArch): string {
  switch (nodeArch) {
    case 'ia32':
      return 'i686'
    case 'x64':
      return 'x86_64'
    case 'armv7l':
      return 'arm'
    case 'arm64':
      return 'aarch64'
    default:
      return nodeArch
  }
}

function forgeArchToBuilderArch(arch: ForgeArch): Arch {
  switch (arch) {
    case 'ia32':
      return Arch.ia32
    case 'x64':
      return Arch.x64
    case 'armv7l':
      return Arch.armv7l
    case 'arm64':
      return Arch.arm64
    default:
      return Arch.x64
  }
}

export default class MakerAppImage extends MakerBase<MakerAppImageConfig> {
  name = 'appimage'

  defaultPlatforms: ForgePlatform[] = ['linux']

  isSupportedOnCurrentPlatform(): boolean {
    return process.platform === 'linux'
  }

  async make({ dir, makeDir, targetArch, appName, packageJSON }: MakerOptions): Promise<string[]> {
    const arch = appImageArch(targetArch)
    const builderArch = forgeArchToBuilderArch(targetArch)
    const outDir = path.resolve(makeDir, 'appimage', arch)

    await this.ensureDirectory(outDir)

    const options = this.config.options || {}
    const id = options.id || ''
    const productName = options.productName || appName
    const executableName = options.bin || appName.toLowerCase()
    const categories = options.categories || ['AudioVideo', 'Player']
    const mimeTypes = options.mimeType || []

    const stageDir = path.join(outDir, 'stage')

    if (await fs.pathExists(stageDir)) {
      await fs.remove(stageDir)
    }

    await fs.ensureDir(stageDir)

    const desktopEntry = [
      '[Desktop Entry]',
      `Name=${productName}`,
      `Exec=${executableName} %U`,
      `Icon=${executableName}`,
      `StartupWMClass=${id}`,
      'Type=Application',
      `Categories=${categories.join(';')};`,
      ...(mimeTypes.length > 0 ? [`MimeType=${mimeTypes.join(';')};`] : []),
      'StartupNotify=true'
    ].join('\n')

    if (!options.icon || !(await fs.pathExists(options.icon)))
      throw new Error(`[MakerAppImage] Icon not found at ${options.icon}`)

    const appImageName = `${productName}-${packageJSON.version}-${targetArch}.AppImage`
    const outputPath = path.join(outDir, appImageName)

    console.log(`[MakerAppImage] Creating ${appImageName}...`)

    const buildOptions: AppImageBuilderOptions = {
      appDir: dir,
      stageDir: stageDir,
      arch: builderArch,
      output: outputPath,
      options: {
        productName,
        productFilename: productName.replace(/\s+/g, ''),
        executableName,
        desktopEntry,
        icons: [{ file: options.icon, size: 256 }],
        license: null,
        fileAssociations: [],
        compression: 'zstd'
      }
    }

    if (this.config.updater) {
      const appUpdatePath = path.join(dir, 'resources', 'app-update.yml')
      const appUpdateConfig = {
        provider: 'generic',
        url: this.config.updater.url,
        channel: this.config.updater.channel || 'latest',
        updaterCacheDirName: this.config.updater.updaterCacheDirName || `${appName.toLowerCase()}-updater`,
      }

      await fs.outputFile(appUpdatePath, yaml.dump(appUpdateConfig))
      console.log(`[MakerAppImage] Created: ${appUpdatePath}`)
    }

    const blockMap = await buildStaticRuntimeAppImage(APPIMAGE_TOOLSET, buildOptions)
    console.log(`[MakerAppImage] Created: ${outputPath}`)

    const updateInfo = {
      version: packageJSON.version,
      files: [
        {
          url: appImageName,
          sha512: blockMap.sha512,
          size: blockMap.size,
          blockMapSize: blockMap.blockMapSize,
        },
      ],
      path: appImageName,
      sha512: blockMap.sha512,
      releaseDate: new Date().toISOString()
    }

    const ymlPath = path.join(outDir, 'latest-linux.yml')
    await fs.writeFile(ymlPath, yaml.dump(updateInfo))
    console.log(`[MakerAppImage] Created: ${ymlPath}`)

    await fs.remove(stageDir)

    return [outputPath, ymlPath]
  }
}

export { MakerAppImage, MakerAppImageConfig }
