const stack = [];
const blocked = new Map();
const stops = 'button,input,textarea,select,a[href],[tabindex],[contenteditable="true"]';

export const modalOpen = () => stack.length > 0;

function isolate() {
  const top = stack.at(-1);
  const wanted = new Set();
  if (top) {
    for (let branch = top.root; branch?.parentElement; branch = branch.parentElement) {
      for (const sibling of branch.parentElement.children) {
        // A menu is the modal's own popover (Settings > Screener "Edit"):
        // rendered beside the scrim, never a background to block — inert
        // would make it transparent to the click, which then lands on
        // what lies beneath.
        if (sibling === branch || sibling === top.backdrop || sibling.matches('[role="menu"]')) continue;
        wanted.add(sibling);
      }
    }
  }
  // Diff, not clear-and-reapply: an element already blocked keeps its
  // recorded state; only newcomers and leavers are touched.
  for (const [element, wasInert] of blocked) {
    if (!wanted.has(element)) {
      element.inert = wasInert;
      blocked.delete(element);
    }
  }
  for (const element of wanted) {
    if (!blocked.has(element)) {
      blocked.set(element, element.inert);
      element.inert = true;
    }
  }
}

// Svelte action shared by modal surfaces; the top owner alone controls focus.
export function modal(node, options = {}) {
  const trigger = document.activeElement;
  const previousTabindex = node.getAttribute('tabindex');
  node.tabIndex = -1;
  const owner = {
    node,
    root: node.closest('.scrim') ?? node,
    backdrop: options.backdrop ? document.querySelector(options.backdrop) : null,
  };
  const current = () => stack.at(-1) === owner;
  const tabbable = () => [...node.querySelectorAll(stops)].filter(element =>
    element.tabIndex >= 0 && !element.matches(':disabled') && !element.closest('[inert]')
      && element.getClientRects().length && getComputedStyle(element).visibility !== 'hidden');
  const focus = () => {
    if (!current() || node.contains(document.activeElement)) return;
    options.initial?.();
    if (!node.contains(document.activeElement)) (tabbable()[0] ?? node).focus();
  };
  const keydown = event => {
    if (!current()) return;
    // The shared menu owns its own Escape/Tab close and trigger restoration.
    if (event.target.closest?.('[role="menu"]')) return;
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      options.close?.();
    } else if (event.key === 'Tab') {
      const list = tabbable();
      const active = document.activeElement;
      const edge = event.shiftKey ? list[0] : list.at(-1);
      if (!list.includes(active) || active === edge) {
        event.preventDefault();
        (event.shiftKey ? list.at(-1) ?? node : list[0] ?? node).focus();
      }
    }
  };
  stack.push(owner);
  isolate();
  queueMicrotask(focus);
  document.addEventListener('keydown', keydown);
  document.addEventListener('focusin', focus);
  // New overlays/controls must not reopen the background while a modal is up.
  // Mutations inside the modal itself (typing, re-rendering) change nothing.
  const observer = new MutationObserver(records => {
    if (current() && records.some(record => !owner.root.contains(record.target))) isolate();
  });
  observer.observe(document.body, { childList: true, subtree: true });
  return {
    update(next) { options = next; },
    destroy() {
      const wasTop = current();
      observer.disconnect();
      document.removeEventListener('keydown', keydown);
      document.removeEventListener('focusin', focus);
      stack.splice(stack.indexOf(owner), 1);
      isolate();
      if (previousTabindex === null) node.removeAttribute('tabindex');
      else node.setAttribute('tabindex', previousTabindex);
      if (wasTop) queueMicrotask(() => {
        if (trigger?.isConnected && !trigger.closest('[inert]')) trigger.focus();
        else {
          const remaining = stack.at(-1)?.node;
          (remaining ?? document.querySelector('button:not(:disabled)'))?.focus();
        }
      });
    },
  };
}
