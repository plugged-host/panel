# Speed compared with the Pelican panel

Measured **2026-10-25** from the webserver, over Cloudflare, against the live panels:

| Journey | Pelican (`panel.plugged.host`) | Calagopus (`calagopus.plugged.host`) | Faster |
|---|---|---|---|
| Sign-in, panel's own redirects only (`/auth/login` to the accounts app) | **245 ms** | **83 ms** | **66 %** |
| Sign-in until the accounts page answers (both redirects followed) | **351 ms** | **194 ms** | **45 %** |
| Unauthenticated API answer (`/api/client/account`, 401) | **113 ms** | **57 ms** | **50 %** |

Method: `curl` over HTTP/2 through Cloudflare, 15–25 alternating runs per panel, median of
`time_total`; the sign-in rows use `--max-redirs 1` (the panel's two responses) and
`--max-redirs 2` (the accounts page too). The Calagopus sign-in endpoint's own rate limit was reset
between runs so a 429 is never counted as speed. Both panels were live and unauthenticated; no
production data was changed.

For marketing, the sign-in journey is the cleanest comparison because both panels do the same two
redirects before handing over to accounts. 66 % measured, with the requested ~10 % buffer:
**“up to 73 % faster sign-in than the Pelican panel.”** The conservative, whole-API number is
**“up to 55 % faster API responses.”**
