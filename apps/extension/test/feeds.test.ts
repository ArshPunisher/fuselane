import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MAX_FEEDS, feedList } from '../lib/feeds.ts'

const PAGE = 'https://blog.example.org/posts/42'

test('feeds become absolute, each once, at most three', () => {
  const list = feedList(
    [
      { href: '/feed.xml', title: ' Posts ' },
      { href: 'https://blog.example.org/feed.xml', title: 'Posts again' },
      { href: 'https://blog.example.org/comments.atom', title: 'Comments' },
      { href: 'javascript:alert(1)', title: 'x' },
      { href: 'http://[bad', title: 'x' },
      { href: '/a.xml', title: 'A' },
      { href: '/b.xml', title: 'B' },
    ],
    PAGE,
  )
  assert.equal(list.length, MAX_FEEDS)
  assert.deepEqual(list[0], { href: 'https://blog.example.org/feed.xml', title: 'Posts' })
  assert.equal(list[1]!.title, 'Comments')
})
