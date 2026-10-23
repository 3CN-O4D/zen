# Web automation tests

Four working approaches to the same task: scrape metadata for a public TikTok
video and download the MP4.

Walkthrough: [Chapter 44](../projects/44-web-automation.md).

## Setup

```bash
# From book-v2/tests
./setup.sh
cd web-automation
```

Then, for this shell:

```bash
export PYTHONPATH="$(../.venv/bin/python -c 'import sysconfig; print(sysconfig.get_paths()["purelib"])')"
export ZEN_NO_PY_BOOTSTRAP=1
ZEN=../../../target/release/zen
```

Alternatively, install this directory's own requirements and let Zen manage
them:

```bash
../../../target/release/zen py-sync
```

## Run

```bash
$ZEN run bs4_tiktok.z          # native requests + bs4 + urllib
$ZEN run ms_tiktok.z           # MechanicalSoup
timeout 300 $ZEN run selenium_tiktok.z   # Selenium (Chromium + chromedriver)
timeout 180 $ZEN run cdp_tiktok.z        # raw sockets + CDP
```

Or run all four with validation:

```bash
../run-all.sh --network
```

Artifacts are written to `out/` (git-ignored):

```
out/tiktok_bs4.mp4
out/tiktok_ms.mp4
out/tiktok_selenium.mp4
out/cdp-profile/          # throwaway Chromium profile for the CDP test
```

Validate with:

```bash
file out/*.mp4
# ISO Media, MP4 Base Media v1 [ISO 14496-12:2003]
```

## Notes

- The tests target `@scout2015/video/6718335390845095173` because it has
  remained publicly accessible.
- Plain HTTP requests to TikTok **video/profile** pages are often served a WAF
  challenge; `/embed/v2/<id>` is not. That is why the static tests use the
  embed route, and Selenium uses a real browser for the video page.
- `cdp_tiktok.z` launches Chromium itself with `pymport subprocess` on port
  `9223` and terminates it in a `finally` block. If a run is killed hard,
  remove leftovers with `pkill -x chromium`.
- `chromedriver` must match the installed Chromium major version. On Debian:
  `apt install chromium-driver`.
- In containers, `--no-sandbox` and `--disable-dev-shm-usage` are already
  passed by the Selenium and CDP examples.
