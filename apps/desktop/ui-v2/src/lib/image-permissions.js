import { onDestroy } from 'svelte';

let generation = 0;
const readers = new Set();
export const imagePermissionGeneration = () => generation;

export function imagePermissionsChanged() {
  generation += 1;
  for (const reload of readers) reload();
}

export function watchImagePermissions(reload) {
  const subscription = () => reload();
  readers.add(subscription);
  onDestroy(() => readers.delete(subscription));
}
