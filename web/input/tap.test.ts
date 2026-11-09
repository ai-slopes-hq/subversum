import { describe, expect, it } from 'vitest';
import { isTap } from './tap';

const base = { cancelled: false, heldS: 0.1, movedPx: 3, viewportPx: 800 };

describe('isTap', () => {
  it('quick small press is a tap', () => expect(isTap(base)).toBe(true));
  it('cancel is never a tap', () => expect(isTap({ ...base, cancelled: true })).toBe(false));
  it('too long is not a tap', () => expect(isTap({ ...base, heldS: 0.25 })).toBe(false));
  it('moved too far is not a tap', () => expect(isTap({ ...base, movedPx: 100 })).toBe(false));
  it('NaN hold time is not a tap', () => expect(isTap({ ...base, heldS: NaN })).toBe(false));
});
