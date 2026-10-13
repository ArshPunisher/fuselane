import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MAX_ITEMS, mediaList, type Found } from '../lib/media.ts'

const PAGE = 'https://files.example.org/downloads/42'

test('file links are listed in page order, each once', () => {
  const found: Found[] = [
    { url: '/files/talk.pdf', kind: 'link', label: ' Slides (PDF) ' },
    { url: '/files/talk.pdf#page=2', kind: 'link', label: 'Slides' },
    { url: '/about', kind: 'link', label: 'About' },
    { url: 'https://example.org/download/app.dmg?x=1', kind: 'link', label: 'Mac' },
  ]
  const list = mediaList(found, PAGE)
  assert.deepEqual(
    list.map((m) => [m.kind, m.name]),
    [
      ['link', 'talk.pdf'],
      ['link', 'app.dmg'],
    ],
  )
  assert.equal(list[0]!.url, 'https://files.example.org/files/talk.pdf')
  assert.equal(list[0]!.label, 'Slides (PDF)')
})

test('streams and non-web links are left out', () => {
  const found: Found[] = [
    { url: 'blob:https://files.example.org/1234', kind: 'link', label: 'x.zip' },
    { url: 'data:application/zip;base64,AAAA', kind: 'link', label: '' },
    { url: 'javascript:void(0)', kind: 'link', label: 'x.zip' },
    { url: 'mailto:me@example.org', kind: 'link', label: '' },
    { url: 'http://[bad', kind: 'link', label: '' },
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
