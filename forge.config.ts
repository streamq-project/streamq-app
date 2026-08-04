import type { ForgeArch, ForgeConfig, ForgePlatform } from '@electron-forge/shared-types'
import { execFileSync } from 'child_process'
import path from 'node:path'
import { VitePlugin } from '@electron-forge/plugin-vite'
import { FusesPlugin } from '@electron-forge/plugin-fuses'
import { FuseV1Options, FuseVersion } from '@electron/fuses'
import { MakerAppImage } from './makers/appimage'
import { MakerNsis } from './makers/nsis'
import { getAppConfig } from './tools/getAppConfig'

const appConfig = getAppConfig()

const buildNativeAddon = (platform: ForgePlatform, arch: ForgeArch, release: boolean) => {
  console.log(`[native] Building ${release ? 'release' : 'debug'} addon for ${platform}-${arch}...`)

  const target = ({
    'linux-arm64': 'aarch64-unknown-linux-gnu',
    'win32-x64': 'x86_64-pc-windows-msvc'
  } as Record<string, string>)[`${platform}-${arch}`]
  const crossCompile = platform !== process.platform

  execFileSync(
    process.execPath,
    [
      path.join(path.dirname(require.resolve('@napi-rs/cli/package.json')), 'dist', 'cli.js'),
      'build',
      '--platform',
      '--no-const-enum',
      '--runtime-string-enum',
      '--cwd', 'native',
      ...(crossCompile ? ['--cross-compile'] : []),
      ...(release ? ['--release'] : []),
      '--package-json-path', '../package.json',
      '--output-dir', 'dist',
      ...(target ? ['--target', target] : []),
    ],
    { stdio: 'inherit' }
  )
}

const config: ForgeConfig = {
  packagerConfig: {
    asar: true,
    icon: './build/icon.ico',
    executableName: 'streamq',
  },
  rebuildConfig: {},
  hooks: {
    preStart: async () => {
      const platform = (process.env.npm_config_platform || process.platform) as ForgePlatform
      const arch = (process.env.npm_config_arch || process.arch) as ForgeArch

      buildNativeAddon(platform, arch, false)
    },
    prePackage: async (_forgeConfig, platform, arch) => {
      buildNativeAddon(platform, arch, true)
    },
    postPackage: async (_forgeConfig, options) => {
      const fs = await import('fs')

      const sourceDir = path.join(__dirname, 'native', 'dist')
      const targetDir = path.join(options.outputPaths[0], 'modules', 'native')
      const addonMarker = `.${options.platform}-${options.arch}`

      await fs.promises.mkdir(targetDir, { recursive: true })

      const files = await fs.promises.readdir(sourceDir)
      for (const file of files) {
        if (!file.endsWith('.node') || !file.includes(addonMarker)) continue
        await fs.promises.copyFile(path.join(sourceDir, file), path.join(targetDir, file))
      }

      const proxySourceDir = path.join(__dirname, 'modules', 'proxy')
      const proxyTargetDir = path.join(options.outputPaths[0], 'modules', 'proxy')

      await fs.promises.mkdir(proxyTargetDir, { recursive: true })

      const proxyFiles = await fs.promises.readdir(proxySourceDir)
      for (const file of proxyFiles) {
        if (options.platform === 'win32' && file === 'xray') continue
        if (options.platform !== 'win32' && file === 'xray.exe') continue

        await fs.promises.copyFile(
          path.join(proxySourceDir, file),
          path.join(proxyTargetDir, file)
        )
      }
    },
  },

  makers: [
    new MakerNsis({
      getAppBuilderConfig: async () => ({
        artifactName: '${productName}-${version}-${arch}.${ext}'
      }),
      ...(appConfig.updatesUrl ? { updater: { url: appConfig.updatesUrl } } : {})
    }),
    new MakerAppImage({
      options: {
        id: 'io.streamq.StreamQ',
        productName: 'StreamQ',
        categories: ['AudioVideo', 'Music'],
        mimeType: ['x-scheme-handler/streamq'],
        icon: 'build/icon.png'
      },
      ...(appConfig.updatesUrl ? { updater: { url: appConfig.updatesUrl } } : {})
    })
  ],
  plugins: [
    new VitePlugin({
      build: [
        {
          entry: 'src/main/index.ts',
          config: 'vite.main.config.ts',
          target: 'main'
        },
        {
          entry: 'src/preload/main-preload.ts',
          config: 'vite.preload.config.ts',
          target: 'preload'
        },
        {
          entry: 'src/preload/bootstrap-preload.ts',
          config: 'vite.preload.config.ts',
          target: 'preload'
        },
        {
          entry: 'src/preload/widget-preload.ts',
          config: 'vite.preload.config.ts',
          target: 'preload'
        }
      ],
      renderer: [
        {
          name: 'bootstrap_window',
          config: 'vite.renderer.config.ts',
        }
      ]
    }),
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true
    })
  ]
}

export default config
