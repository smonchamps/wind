| Raw ceiling / fixture size | Content | Decoded attachment | Parser-only peak WS | With conversion peak WS | With conversion peak paged memory | Parser-only total / converted total | MIME decode / conversion phase |
|---|---|---:|---:|---:|---:|---:|---:|
| 8 MiB | Plain text | 0 MiB | 14.06 MiB | 54.36 MiB | 49.00 MiB | 14.4 / 207.4 ms | 2.2 / 193.6 ms |
| 8 MiB | HTML text | 0 MiB | 14.05 MiB | 54.38 MiB | 48.96 MiB | 716.4 / 99.5 ms | 2.1 / 86.6 ms |
| 8 MiB | HTML + base64 attachment | 4 MiB | 18.17 MiB | 36.51 MiB | 32.52 MiB | 54.6 / 75.7 ms | 15.2 / 29.5 ms |
| 16 MiB | Plain text | 0 MiB | 22.05 MiB | 102.34 MiB | 97.10 MiB | 47.6 / 204.5 ms | 5.3 / 181.4 ms |
| 16 MiB | HTML text | 0 MiB | 22.05 MiB | 102.39 MiB | 97.12 MiB | 40.9 / 217.8 ms | 4.3 / 194.9 ms |
| 16 MiB | HTML + base64 attachment | 8 MiB | 30.16 MiB | 65.54 MiB | 64.22 MiB | 108.4 / 160.1 ms | 33.3 / 62.4 ms |
| 32 MiB | Plain text | 0 MiB | 38.16 MiB | 198.37 MiB | 193.39 MiB | 104.0 / 431.7 ms | 10.0 / 385.5 ms |
| 32 MiB | HTML text | 0 MiB | 38.17 MiB | 198.40 MiB | 193.40 MiB | 88.8 / 394.7 ms | 8.5 / 354.3 ms |
| 32 MiB | HTML + base64 attachment | 16 MiB | 54.16 MiB | 121.04 MiB | 121.77 MiB | 195.5 / 309.5 ms | 62.3 / 117.5 ms |
