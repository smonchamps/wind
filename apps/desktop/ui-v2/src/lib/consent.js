// One UI owner per consent. Closing before begin resolves still cancels its id.
export function createConsent(call, changed) {
  let id = null;
  let disposed = false;
  let running = false;
  let cancelled = false;
  let timer;
  let cancelAttempt;
  let view;
  const update = (values) => {
    view = { ...view, ...values };
    if (!disposed) changed(view);
  };
  let cancelRequested = false;
  const cancel = () => {
    if (!id) {
      // Asked during `oauth_begin`: recorded, honored as soon as the id exists.
      if (running && !cancelRequested) {
        cancelRequested = true;
        update({ cancelling: true, cancelError: false });
      }
      return Promise.resolve(false);
    }
    if (cancelAttempt) return cancelAttempt;
    update({ cancelling: true, cancelError: false });
    cancelAttempt = call('oauth_cancel', { flowId: id }).then((accepted) => {
      cancelled = accepted;
      update({ cancelled: accepted, cancelling: false, finishing: !accepted });
      return accepted;
    }, () => {
      update({ cancelling: false, cancelError: true });
      return false;
    }).finally(() => { cancelAttempt = null; });
    return cancelAttempt;
  };
  const poll = async () => {
    const current = id;
    try {
      const status = await call('oauth_status', { flowId: current });
      if (running && !disposed && id === current && status) update(status);
    } catch { /* Cancellation remains available when a status read fails. */ }
    if (running && !disposed && id === current) timer = setTimeout(poll, 250);
  };
  return {
    cancel,
    dispose() {
      disposed = true;
      clearTimeout(timer);
      void cancel();
    },
    async run(command, args) {
      if (running || disposed) return false;
      running = true;
      cancelled = false;
      cancelRequested = false;
      view = {};
      update({ active: true });
      try {
        id = await call('oauth_begin');
        if (disposed || cancelRequested) { await cancel(); return false; }
        void poll();
        await call(command, { ...args, flowId: id });
        if (cancelAttempt) await cancelAttempt;
        return !cancelled;
      } catch (err) {
        if (cancelAttempt) await cancelAttempt;
        if (!cancelled && !disposed) throw err;
        return false;
      } finally {
        running = false;
        clearTimeout(timer);
        // Releases an unclaimed preparation too (e.g. validation failed).
        if (id) await call('oauth_cancel', { flowId: id }).catch(() => {});
        id = null;
        update({ active: false, url: null });
      }
    },
  };
}
