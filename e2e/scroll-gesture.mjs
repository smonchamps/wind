// The gesture of the field finding (PLAN-DEFILEMENT-PROFOND): the
// scrollbar HELD on click — a scrollTop ramp at ~60 events
// per second. Shared between the spec (redesign-scroll.spec.js) and
// the bench (measure-scroll.mjs): the same gesture on both sides, or
// else the numbers of one wouldn't describe what the other tracks.
export async function holdBar(page, { step = 60, fraction = 1 / 3, intervalMs = 16 } = {}) {
  await page.evaluate(
    async ({ step, fraction, intervalMs }) => {
      const frame = document.querySelector('[data-testid="list"] .frame');
      const target = frame.scrollHeight * fraction;
      for (let k = 1; k <= step; k++) {
        frame.scrollTop = (target * k) / step;
        await new Promise((resolve) => setTimeout(resolve, intervalMs));
      }
    },
    { step, fraction, intervalMs },
  );
}
