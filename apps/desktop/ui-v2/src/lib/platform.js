// The platform probe (backlog 109): macOS changes the delete key and
// the shortcut labels — nothing else reads the OS in the UI. e2e seam
// (the __e2eLinks pattern): the harness impersonates a platform so one
// machine proves both directions; compiled out of releases.
export function isMac() {
  const seam = import.meta.env.VITE_E2E === '1' ? globalThis.window?.__e2ePlatform : undefined;
  if (seam) return seam === 'mac';
  const probe = navigator.userAgentData?.platform ?? navigator.platform ?? '';
  return /mac/i.test(probe);
}
