// File types for the list's type filter (B8.8). Same extensions as
// automation::category in the service, which sorts finished files into folders.
import { mark, t } from './i18n'

export type FileType =
  | 'video'
  | 'music'
  | 'pictures'
  | 'documents'
  | 'archives'
  | 'disk-images'
  | 'apps'
  | 'torrents'
  | 'other'

export const TYPE_LABEL: Record<FileType, string> = {
  video: mark('Video'),
  music: mark('Music'),
  pictures: mark('Pictures'),
  documents: mark('Documents'),
  archives: mark('Archives'),
  'disk-images': mark('Disk images'),
  apps: mark('Apps'),
  torrents: mark('Torrents'),
  other: mark('Other'),
}

/** A type's name, in the language in use. */
export function typeLabel(type: FileType): string {
  return t(TYPE_LABEL[type])
}

const BY_EXT: Record<string, FileType> = {}
const add = (t: FileType, exts: string) => exts.split(' ').forEach((e) => (BY_EXT[e] = t))
add('video', 'mp4 mkv avi mov webm m4v wmv flv mpg mpeg ts 3gp')
add('music', 'mp3 flac wav aac m4a ogg opus wma aiff alac')
add('pictures', 'jpg jpeg png gif webp heic svg bmp tif tiff raw')
add('documents', 'pdf doc docx xls xlsx ppt pptx txt rtf odt ods odp epub csv md pages numbers key')
add('archives', 'zip rar 7z tar gz tgz bz2 xz zst lz cab')
add('disk-images', 'iso img dmg vhd vhdx qcow2')
add('apps', 'exe msi pkg deb rpm appimage apk msix flatpak')

/** The type of a file from its name ("movie.MKV" → video). */
export function fileType(name: string): FileType {
  const dot = name.lastIndexOf('.')
  if (dot < 0) return 'other'
  return BY_EXT[name.slice(dot + 1).toLowerCase()] ?? 'other'
}
