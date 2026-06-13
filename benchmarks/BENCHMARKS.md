# fnx bench history

Appended by `fnx bench`. cold = fresh Gradle user home; warm = same home with
build outputs wiped (the CI-with-cache scenario). Timings are machine-local —
the host is recorded per run in history.jsonl.

| date | commit | target | cold | warm |
|------|--------|--------|-----:|-----:|
| 2026-06-10T19:50:00+1000 | 4dc78211b206 | lock | 236.1s | 225.9s |
| 2026-06-10T19:50:00+1000 | 4dc78211b206 | gradle-build | 21.1s | 5.9s |
| 2026-06-10T19:50:00+1000 | 4dc78211b206 | flutter-build | 39.5s | 13.5s |
| 2026-06-10T19:53:46+1000 | c28540126a05 | gradle-build | 21.4s | 5.8s |
| 2026-06-10T22:52:00+1000 | 5903aa3f3f20 | lock | 153.3s | 10.9s |
| 2026-06-10T23:38:10+1000 | b6e92a760db7 | lock | 164.4s | 9.9s |
| 2026-06-11T20:19:22+1000 | 95d6b771a479 | ios-lock | 3.4s | 0.1s |
| 2026-06-11T20:19:58+1000 | 688ff1b067d9 | ios-build | 19.2s | 13.9s |
| 2026-06-11T20:41:53+1000 | ec8d2c3a28a1 | ios-lock | 12.9s | 0.3s |
| 2026-06-11T20:52:50+1000 | e48a3770c588 | ios-build | 29.2s | 22.6s |
| 2026-06-12T17:35:23+1000 | 338a5d0c9bf0 | lock | 137.2s | 15.9s |
| 2026-06-12T17:35:23+1000 | 338a5d0c9bf0 | gradle-build | 33.3s | 9.5s |
| 2026-06-12T17:35:23+1000 | 338a5d0c9bf0 | flutter-build | 60.8s | 23.5s |
| 2026-06-12T17:35:23+1000 | 338a5d0c9bf0 | ios-lock | 12.9s | 0.3s |
| 2026-06-12T17:35:23+1000 | 338a5d0c9bf0 | ios-build | 31.9s | 23.8s |
| 2026-06-13T13:58:01+1000 | a0cf14cf1797 | lock | 147.1s | 13.5s |
| 2026-06-13T13:58:01+1000 | a0cf14cf1797 | gradle-build | 35.8s | 9.5s |
| 2026-06-13T13:58:01+1000 | a0cf14cf1797 | flutter-build | 59.2s | 21.1s |
| 2026-06-13T13:58:01+1000 | a0cf14cf1797 | ios-lock | 4.9s | 0.0s |
