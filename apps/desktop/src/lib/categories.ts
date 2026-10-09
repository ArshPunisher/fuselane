// File types for the list's type filter (B8.8). Same extensions as
// automation::category in the service, which sorts finished files into folders.

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
  video: 'Video',
  music: 'Music',
  pictures: 'Pictures',
  documents: 'Documents',
  archives: 'Archives',
  'disk-images': 'Disk images',
  apps: 'Apps',
  torrents: 'Torrents',
  other: 'Other',
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
