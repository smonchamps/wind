from pathlib import Path
import json
p=Path(__file__).parent
r=json.loads((p/'candidate-results.json').read_text(encoding='utf-8-sig'))
assert len(r)==51,len(r)
for x in r:
    cap=x['raw_cap']
    assert cap in (8388608,16777216,33554432)
    assert x['body_command_admitted_bytes']<=cap+8192
    assert x['unrelated_after_replacement_complete']
    if x['case'].endswith('abuse64'):
        if x['case'].startswith('honest'):
            assert x['status']=='preflight-refused' and x['body_command_admitted_bytes']==0
        else:
            assert x['status']=='error: quota' and x['body_command_admitted_bytes']==cap+8192
            assert x['poisoned'] and not x['noop_reuse_succeeded'] and x['returned_body_max']==0
    elif x['case'].endswith('cap-plus'):
        assert x['status']==('preflight-refused' if x['case'].startswith('honest') else 'raw-ceiling-refused')
        assert x['parsed_messages']==0
    else:
        assert x['status']=='complete' and x['parsed_messages']==1
    if x['case'].endswith('mixed'):
        assert x['decoded_attachment_bytes']==cap//2
        assert len(x['attachments'])==1 and x['attachments'][0]['equal']
    if x['conversion_enabled']:
        assert x['owned_text_bytes']>0 and x['owned_html_bytes']>0 and x['sanitized_html_bytes']>0
        assert x['copied_attachment_bytes']==x['decoded_attachment_bytes']
table='| Raw ceiling / fixture size | Content | Decoded attachment | Parser-only peak WS | With conversion peak WS | With conversion peak paged memory | Parser-only total / converted total | MIME decode / conversion phase |\n|---|---|---:|---:|---:|---:|---:|---:|\n'
for cap in (8388608,16777216,33554432):
    for case,label in [('honest-cap-exact','Plain text'),('honest-html','HTML text'),('honest-mixed','HTML + base64 attachment')]:
        a=next(x for x in r if x['raw_cap']==cap and x['case']==case and not x['conversion_enabled'])
        b=next(x for x in r if x['raw_cap']==cap and x['case']==case and x['conversion_enabled'])
        table+=f"| {cap//1048576} MiB | {label} | {b['decoded_attachment_bytes']/1048576:.0f} MiB | {a['os_peak_working_set_bytes']/1048576:.2f} MiB | {b['os_peak_working_set_bytes']/1048576:.2f} MiB | {b['os_peak_paged_memory_bytes']/1048576:.2f} MiB | {a['elapsed_ms']:.1f} / {b['elapsed_ms']:.1f} ms | {b['parse_decode_ms']:.1f} / {b['conversion_ms']:.1f} ms |\n"
(p/'candidate-table.md').write_text(table)
print('51 candidate cases checked: boundary outcomes, strict admitted response cap, poisoned reuse refusal, unrelated progress and all attachment bytes pass.')
