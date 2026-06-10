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
