from pathlib import Path
import json
p = Path(__file__).parent
r = json.loads((p/'results.json').read_text(encoding='utf-8-sig'))
assert len(r) == 34
for x in r:
    assert x['unrelated_after_replacement_complete']
    if x['bounded']:
        assert x['body_command_admitted_bytes'] <= 2105344
        if x['case'] in ('missing-8m','lying-8m','long-line','aggregate'):
            assert x['status'] == 'error: quota' and x['returned_body_max'] == 0
            assert x['poisoned'] and not x['noop_reuse_succeeded']
        if x['case'] == 'deadline':
            assert x['poison_reason'] == 'deadline' and not x['noop_reuse_succeeded']
        if x['case'] == 'honest-8m':
            assert x['status'] == 'preflight-refused' and x['body_command_admitted_bytes'] == 0
        if x['case'] == 'missing-cap-plus':
            assert x['status'] == 'raw-ceiling-refused' and x['parsed_messages'] == 0 and x['noop_reuse_succeeded']
    if x['case']=='nested':
        assert {(a['name'],a['decoded_bytes'],a['equal']) for a in x['attachments']} == {('inner.bin',620,True),('outer.bin',4096,True)}
table = '| Case | Bounded outcome | Admitted response bytes | Max returned raw bytes | Peak working set bounded / control (bytes) | Peak paged memory bounded / control (bytes) | Elapsed bounded / control (ms) |\n|---|---|---:|---:|---:|---:|---:|\n'
for b in r:
    if not b['bounded']: continue
    u = next(x for x in r if not x['bounded'] and x['case']==b['case'])
    table += f"| {b['case']} | {b['status']} | {b['body_command_admitted_bytes']} | {b['returned_body_max']} | {b['os_peak_working_set_bytes']} / {u['os_peak_working_set_bytes']} | {b['os_peak_paged_memory_bytes']} / {u['os_peak_paged_memory_bytes']} | {b['elapsed_ms']:.3f} / {u['elapsed_ms']:.3f} |\n"
(p/'measured-table.md').write_text(table,encoding='utf-8')
print('34 cases checked; all quota, poison, independent progress and attachment equality assertions pass.')
