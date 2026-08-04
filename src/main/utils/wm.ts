import os from 'os'

const getWindowManager = () => {
  if (process.env.HYPRLAND_INSTANCE_SIGNATURE) return 'hyprland'
  if (process.env.SWAYSOCK) return 'sway'
  if (process.env.I3SOCK) return 'i3'

  return (
    process.env.XDG_CURRENT_DESKTOP ||
    process.env.XDG_SESSION_DESKTOP ||
    'unknown'
  ).toLowerCase()
}

const tilingWMs = ['hyprland', 'sway', 'i3', 'niri', 'river', 'bspwm', 'qtile']

export const getWM = () => {
  if (os.platform() !== 'linux') return null
  const wm = getWindowManager()
  return { wm, isTiling: tilingWMs.some(name => wm.includes(name)) }
}
