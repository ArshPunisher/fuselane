// Fuselane UI kit. Components arrive in Phase 3 (STEPS 3.2); the design tokens
// already live in ./tokens.css. Source of truth: docs/07-design/DESIGN-SYSTEM.md.
export const LANES = ['tide', 'volt', 'iris', 'rose', 'mint', 'sky', 'lilac', 'steel'] as const
export type Lane = (typeof LANES)[number]
