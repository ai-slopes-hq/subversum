/** A tap is a short press that barely moved; anything else is a hold or a drag. */
export const TAP_MAX_S = 0.25;
/** Largest travel (fraction of the larger viewport side) that still counts as a tap. */
export const TAP_MAX_MOVE = 0.03;

export interface TapInput {
  /** Pointer events ended by the browser (`pointercancel`) are never taps. */
  cancelled: boolean;
  heldS: number;
  /** Straight-line distance between down and up, in pixels. */
  movedPx: number;
  /** Larger side of the viewport, in pixels. */
  viewportPx: number;
}

export function isTap({ cancelled, heldS, movedPx, viewportPx }: TapInput): boolean {
  if (cancelled) return false;
  if (!(heldS < TAP_MAX_S)) return false;
  return movedPx <= TAP_MAX_MOVE * viewportPx;
}
