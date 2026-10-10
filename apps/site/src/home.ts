// Home: the live Fuse Core in the hero, and the download button for the
// visitor's system.
import { detectOs, fileUrl, latestVersion } from './common'
import './site.css'
import './css/home.css'
import { startHero } from './hero'

async function primaryDownload() {
  const btn = document.querySelector<HTMLAnchorElement>('#primary-download')
  const label = document.querySelector<HTMLElement>('#primary-label')
  const os = detectOs(navigator.userAgent, navigator.platform)
  const version = await latestVersion('./')
  const names: Record<string, string> = { mac: 'macOS', windows: 'Windows', linux: 'Linux' }
  if (label && names[os]) label.textContent = `Download for ${names[os]}`
  if (btn && version) {
    if (os === 'mac') btn.href = fileUrl(version, 'macos-universal.dmg')
    else if (os === 'windows') btn.href = fileUrl(version, 'windows-x64-setup.exe')
  }
}

startHero()
void primaryDownload()
