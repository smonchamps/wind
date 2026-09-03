<script>
  // THE product's menu (PLAN-AUDIT-V2 E11 — closes D-47: eight copies
  // of the drawing and the mechanics, three shadows, three z-indexes,
  // a nonexistent `--ombre` token; A8 held: `role="menu"` promised a
  // keyboard that was absent). One single component: anchoring,
  // ↑/↓ arrows, Home/End, Escape and Tab close, an outside click
  // closes, the focus lands on the first item on opening and RETURNS
  // to the trigger on closing. The parent supplies only the items
  // (snippet) and the `isOpen` state.
  let {
    isOpen = false,
    x = 0,
    y = 0,
    testid = 'menu',
    width = 240,
    // `absolute`: anchored under its trigger (position:absolute inside a
    // relative parent) instead of fixed coordinates.
    absolute = false,
    onclose = () => {},
    children,
  } = $props();

  let mailbox = $state(null);
  const items = () =>
    mailbox ? [...mailbox.querySelectorAll('[role^="menuitem"]:not([disabled])')] : [];

  $effect(() => {
    if (!isOpen) return;
    const trigger = document.activeElement;
    // After render: the menu BOUNDS itself to the window (its real
    // size, not a constant copied seven times — review), then the
    // first item takes the focus.
    queueMicrotask(() => {
      if (mailbox && !absolute) {
        const r = mailbox.getBoundingClientRect();
        if (r.right > window.innerWidth - 8) mailbox.style.left = `${Math.max(8, window.innerWidth - r.width - 8)}px`;
        if (r.bottom > window.innerHeight - 8) mailbox.style.top = `${Math.max(8, window.innerHeight - r.height - 8)}px`;
      }
      items()[0]?.focus();
    });
    const click = (e) => {
      // The OPENING click arrives here too (the effect runs during
      // its propagation): a click on the trigger is never “outside”
      // — the parent decides its toggle.
      if (trigger?.contains?.(e.target)) return;
      if (mailbox && !mailbox.contains(e.target)) onclose();
    };
    const key = (e) => {
      const list = items();
      if (list.length === 0) return;
      const i = list.indexOf(document.activeElement);
      switch (e.key) {
        case 'Escape':
          e.preventDefault();
          onclose();
          break;
        case 'Tab':
          onclose();
          break;
        case 'ArrowDown':
          e.preventDefault();
          list[(i + 1) % list.length].focus();
          break;
        case 'ArrowUp':
          e.preventDefault();
          list[(i - 1 + list.length) % list.length].focus();
          break;
        case 'Home':
          e.preventDefault();
          list[0].focus();
          break;
        case 'End':
          e.preventDefault();
          list[list.length - 1].focus();
          break;
        default:
      }
    };
    window.addEventListener('click', click);
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('click', click);
      window.removeEventListener('keydown', key);
      // The focus returns where it left from — the trigger, if it still lives.
      if (trigger?.isConnected && typeof trigger.focus === 'function') trigger.focus();
    };
  });
</script>

{#if isOpen}
  <div class="menu" class:absolute={absolute} role="menu" data-testid={testid} bind:this={mailbox}
       style={absolute ? `min-width:${width}px` : `left:${x}px; top:${y}px; min-width:${width}px`}>
    {@render children()}
  </div>
{/if}

<style>
  /* The single drawing (D-47 family): the List's gestures' floating
     card — z-index 30 (above the sticky bars, below nothing else: a
     menu is always the topmost object), the --shadow token's shadow,
     the controls' radius. */
  .menu {
    position:fixed; z-index:30; padding:6px; display:flex; flex-direction:column;
    background:var(--surface); border:1px solid var(--border);
    border-radius:var(--r-control); box-shadow:var(--shadow);
  }
  .menu.absolute { position:absolute; top:calc(100% + 6px); left:0; }
  .menu :global(button[role^="menuitem"]) {
    display:flex; align-items:center; gap:10px; width:100%;
    height:32px; padding:0 8px; font-size:13px; color:var(--ink);
    background:none; border:1px solid transparent;
    border-radius:var(--r-control); cursor:pointer; text-align:left;
    white-space:nowrap;
  }
  .menu :global(button[role^="menuitem"]:hover),
  .menu :global(button[role^="menuitem"]:focus-visible) { background:var(--hover); outline:none; }
  .menu :global(button[aria-checked="true"]) { font-weight:600; }
  .menu :global(.net-menu), .menu :global(.net) { height:1px; background:var(--border); margin:4px 0; }
  .menu :global(.title-menu) {
    margin:4px 8px 2px; font-size:11px; font-weight:600; letter-spacing:.02em;
    text-transform:uppercase; color:var(--muted);
  }
</style>
