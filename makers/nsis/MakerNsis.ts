import MakerNsisBase from '@felixrieseberg/electron-forge-maker-nsis';

export default class MakerNsis extends MakerNsisBase {
  isSupportedOnCurrentPlatform() {
    return true
  }
}
