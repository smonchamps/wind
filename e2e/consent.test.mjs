import test from 'node:test';
import assert from 'node:assert/strict';
import { createConsent } from '../apps/desktop/ui-v2/src/lib/consent.js';

const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};

test('closing while begin is pending cancels that id without starting authorization', async () => {
  const begin = deferred();
  const calls = [];
  const consent = createConsent((command, args) => {
    calls.push([command, args?.flowId]);
    return command === 'oauth_begin' ? begin.promise : Promise.resolve(true);
  }, () => {});
  const running = consent.run('add_account', {});
  consent.dispose();
  begin.resolve('old');
  assert.equal(await running, false);
  assert.equal(calls.some(([command]) => command === 'add_account'), false);
  assert.equal(calls.some(([command, id]) => command === 'oauth_cancel' && id === 'old'), true);
});

test('accepted cancellation suppresses the worker rejection without reporting success', async () => {
  const worker = deferred();
  const started = deferred();
  const consent = createConsent((command) => {
    if (command === 'oauth_begin') return Promise.resolve('one');
    if (command === 'oauth_status') return Promise.resolve(null);
    if (command === 'oauth_cancel') {
      worker.reject(new Error('cancelled'));
      return Promise.resolve(true);
    }
    started.resolve();
    return worker.promise;
  }, () => {});
  const running = consent.run('add_account', {});
  await started.promise;
  assert.equal(await consent.cancel(), true);
  assert.equal(await running, false);
});

test('a refused late cancellation waits for and reports the published connection', async () => {
  const worker = deferred();
  const started = deferred();
  let view;
  const consent = createConsent((command) => {
    if (command === 'oauth_begin') return Promise.resolve('one');
    if (command === 'oauth_status') return Promise.resolve(null);
    if (command === 'oauth_cancel') return Promise.resolve(false);
    started.resolve();
    return worker.promise;
  }, value => { view = value; });
  const running = consent.run('add_account', {});
  await started.promise;
  assert.equal(await consent.cancel(), false);
  assert.equal(view.finishing, true);
  assert.equal(view.active, true);
  consent.dispose();
  worker.resolve();
  assert.equal(await running, true);
});

test('a delayed status from the previous attempt cannot overwrite the next link', async () => {
  const oldStatus = deferred();
  const workers = [deferred(), deferred()];
  const starts = [deferred(), deferred()];
  let id = 0, view;
  const consent = createConsent((command) => {
    if (command === 'oauth_begin') return Promise.resolve(String(++id));
    if (command === 'oauth_status') return id === 1 ? oldStatus.promise
      : Promise.resolve({ url: 'new-link' });
    if (command === 'oauth_cancel') return Promise.resolve(false);
    starts[id - 1].resolve();
    return workers[id - 1].promise;
  }, value => { view = value; });
  const first = consent.run('add_account', {});
  await starts[0].promise;
  workers[0].resolve();
  await first;
  const second = consent.run('add_account', {});
  await starts[1].promise;
  oldStatus.resolve({ url: 'old-link' });
  await Promise.resolve();
  assert.equal(view.url, 'new-link');
  workers[1].resolve();
  await second;
});

test('a cancel asked while begin is pending is honored once the id exists', async () => {
  const begin = deferred();
  const calls = [];
  let view;
  const consent = createConsent((command, args) => {
    calls.push([command, args?.flowId]);
    if (command === 'oauth_begin') return begin.promise;
    if (command === 'oauth_cancel') return Promise.resolve(true);
    return Promise.resolve(true);
  }, value => { view = value; });
  const running = consent.run('add_account', {});
  assert.equal(await consent.cancel(), false);
  assert.equal(view.cancelling, true, 'the click is acknowledged at once');
  begin.resolve('one');
  assert.equal(await running, false);
  assert.equal(calls.some(([command]) => command === 'add_account'), false, 'no browser is opened');
  assert.equal(calls.some(([command, id]) => command === 'oauth_cancel' && id === 'one'), true);
});
