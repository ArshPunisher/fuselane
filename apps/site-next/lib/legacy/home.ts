// Home: the live Fuse Core in the hero, and the download button for the
// visitor's system.
import { detectOs, fileUrl, latestVersion } from './common'
import { startHero } from './hero'
import { startProof, startStory } from './story'
import { startRace } from './race'
import { startTools } from './tools'
import { startSend } from './send'

/** Every Download button on the page names the visitor's system and, where one file fits, links to it. */
async function primaryDownload() {
  const os = detectOs(navigator.userAgent, navigator.platform)
  const names: Record<string, string> = { mac: 'macOS', windows: 'Windows', linux: 'Linux' }
  if (names[os])
    document
      .querySelectorAll<HTMLElement>('[data-dl-label]')
      .forEach((l) => (l.textContent = `Download for ${names[os]}`))
  const version = await latestVersion('/')
  if (!version) return
  const file =
    os === 'mac' ? 'macos-universal.dmg' : os === 'windows' ? 'windows-x64-setup.exe' : null
  if (file)
    document
      .querySelectorAll<HTMLAnchorElement>('a[data-dl]')
      .forEach((a) => (a.href = fileUrl(version, file)))
}

export function initHome() {
  startHero()
  startProof()
  startStory()
  startRace()
  startTools()
  startSend()
  void primaryDownload()
}
