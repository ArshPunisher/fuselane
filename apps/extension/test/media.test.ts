import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MAX_ITEMS, mediaList, type Found } from '../lib/media.ts'

const PAGE = 'https://videos.example.org/watch/42'

test('media and file links are listed, media first, each once', () => {
  const found: Found[] = [
    { url: '/files/talk.pdf', kind: 'link', label: ' Slides (PDF) ' },
    { url: 'https://cdn.example.org/v/talk.mp4', kind: 'video', label: 'The talk' },
    { url: 'https://cdn.example.org/v/talk.mp4#t=10', kind: 'video', label: 'The talk' },
    { url: 'https://cdn.example.org/a/talk%20audio.m4a', kind: 'audio', label: '' },
    { url: '/about', kind: 'link', label: 'About' },
    { url: 'https://example.org/download/app.dmg?x=1', kind: 'link', label: 'Mac' },
  ]
  const list = mediaList(found, PAGE)
  assert.deepEqual(
    list.map((m) => [m.kind, m.name]),
    [
      ['video', 'talk.mp4'],
      ['audio', 'talk audio.m4a'],
      ['link', 'talk.pdf'],
      ['link', 'app.dmg'],
    ],
  )
  assert.equal(list[2]!.url, 'https://videos.example.org/files/talk.pdf')
  assert.equal(list[2]!.label, 'Slides (PDF)')
})

test('streams and non-web links are left out', () => {
  const found: Found[] = [
    { url: 'blob:https://videos.example.org/1234', kind: 'video', label: '' },
    { url: 'data:video/mp4;base64,AAAA', kind: 'video', label: '' },
    { url: 'javascript:void(0)', kind: 'link', label: 'x.zip' },
    { url: 'mailto:me@example.org', kind: 'link', label: '' },
    { url: 'http://[bad', kind: 'video', label: '' },
  ]
  assert.deepEqual(mediaList(found, PAGE), [])
})

test('a page full of links is capped', () => {
  const found: Found[] = Array.from({ length: 500 }, (_, i) => ({
    url: `/f/${i}.zip`,
    kind: 'link' as const,
    label: '',
  }))
  assert.equal(mediaList(found, PAGE).length, MAX_ITEMS)
})
