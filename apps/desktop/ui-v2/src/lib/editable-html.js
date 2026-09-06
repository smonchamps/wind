import { call } from './transport.js';

// Keep URLs outside the live document. Stored/sent HTML restores only the
// image nodes still present; this map never names a local message.
const editors = new WeakMap();
function stateFor(field) {
  if (!editors.has(field)) {
    const state = { generation: 0, revision: 0, pastes: 0, images: {}, pending: new Set() };
    field.addEventListener('input', () => { state.revision += 1; });
    editors.set(field, state);
  }
  return editors.get(field);
}

function track(state, flight) {
  state.pending.add(flight);
  const settled = () => state.pending.delete(flight);
  flight.then(settled, settled);
  return flight;
}

export function imageSources(field) {
  return field ? { ...stateFor(field).images } : {};
}

export function markHtmlEdited(field) {
  if (field) stateFor(field).revision += 1;
}

export async function waitForHtml(field) {
  if (!field) return;
  const state = stateFor(field);
  while (state.pending.size) await Promise.all([...state.pending]);
}

export function setSafeHtml(field, html, images = {}, { overwrite = false } = {}) {
  const state = stateFor(field);
  if (!overwrite && (state.revision > 0 || state.pastes > 0)) return Promise.resolve(false);
  const mine = ++state.generation;
  const before = field.innerHTML;
  const revision = state.revision;
  if (!html) {
    field.innerHTML = '';
    state.images = {};
    return Promise.resolve(true);
  }
  return track(state, call('prepare_composer_html', { html, imageSources: images }).then((prepared) => {
    if (!field.isConnected || mine !== state.generation || state.revision !== revision
        || field.innerHTML !== before) return false;
    field.innerHTML = prepared.html;
    state.images = prepared.image_sources;
    return true;
  }));
}

const textHtml = (text) => text.replaceAll('&', '&amp;').replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;').replace(/\r\n|\r|\n/g, '<br>');

export function pasteSafeHtml(event) {
  event.preventDefault();
  const field = event.currentTarget;
  const data = event.clipboardData ?? event.dataTransfer;
  const html = data?.getData('text/html') || textHtml(data?.getData('text/plain') ?? '');
  if (!html) return Promise.resolve();
  const selection = window.getSelection();
  if (!selection?.rangeCount) return Promise.resolve();
  const range = selection.getRangeAt(0).cloneRange();
  if (!field.contains(range.commonAncestorContainer)) return Promise.resolve();
  const state = stateFor(field);
  const mine = state.generation;
  const before = field.innerHTML;
  state.pastes += 1;
  const flight = call('prepare_composer_html', { html, imageSources: state.images }).then((prepared) => {
    if (!field.isConnected || mine !== state.generation) return;
    // A delayed paste must never replace text typed while it was being prepared.
    if (field.innerHTML !== before || !field.contains(range.commonAncestorContainer)) {
      throw new Error('The selection changed while preparing the paste.');
    }
    const focused = document.activeElement;
    field.focus({ preventScroll: true });
    selection.removeAllRanges();
    selection.addRange(range);
    Object.assign(state.images, prepared.image_sources);
    // Native insertion retains the editor's undo history.
    if (!document.execCommand('insertHTML', false, prepared.html)) {
      throw new Error('The editor could not insert the prepared content.');
    }
    if (focused !== field && focused?.isConnected) focused.focus({ preventScroll: true });
  });
  const settled = () => { state.pastes -= 1; };
  flight.then(settled, settled);
  return track(state, flight);
}
