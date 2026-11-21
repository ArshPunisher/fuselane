import type { History } from './store'

/**
 * The rate history with one more sample, newest last, at most `keep` long. When the
 * network set is the same and both the newest sample and this one are all zero there
 * is nothing to add, and the same object comes back, so React doesn't re-render a
 * paused or finished download five times a second for a flat line.
 */
export function appendRates(prev: History, names: string[], rates: number[], keep = 300): History {
  const same = prev.names.join() === names.join()
  const last = prev.rates[prev.rates.length - 1]
  if (same && last && last.every((r) => r === 0) && rates.every((r) => r === 0)) return prev
  return { names, rates: [...(same ? prev.rates : []), rates].slice(-keep) }
}
