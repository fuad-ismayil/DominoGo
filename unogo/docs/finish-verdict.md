## verdict
1. Resolved — password submission now has one busy guard and one BUSY assignment before the request; both outcomes release BUSY. The new account capture visibly shows the cleared password field and enabled Update password control. The browser test explicitly checks a forced 503 error, enabled retry, successful retry, cleared prior error, success feedback, and shared credentials accepted by DominoGo while the old credentials are rejected. Success feedback is below the sheet's captured scroll position, so its assertion is supplied by the browser test rather than this image.
2. All 18 required current captures were opened and remain valid. No regression attributable to this fix was observed.

## remaining
Clear. This ship verdict covers the scored password fix, not a new review of the whole surface.

disposition: ship
