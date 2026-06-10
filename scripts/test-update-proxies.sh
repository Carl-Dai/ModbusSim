#!/usr/bin/env bash
# 无头测试 GitHub 反代是否能正确透传 Tauri updater 所需的 manifest 与 bundle。
# 用途:更换 endpoint 名单前先跑一遍验证。手动执行,不接入 CI。
# 退出码:0 = 至少一个 proxy 同时通过 manifest+bundle 透传;1 = 基线失败或无可用 proxy。
set -u

REPO="Karl-Dai/ModbusSim"
MANIFEST_PATH="releases/latest/download/latest-master.json"
ORIGIN="https://github.com/$REPO/$MANIFEST_PATH"

PROXIES=(
  "https://ghfast.top"
  "https://gh-proxy.com"
  "https://gh.idayer.com"
)

now_ms() { python3 -c 'import time;print(int(time.time()*1000))'; }

TMP=$(mktemp -d)
echo "=== 1) 基线: 直连 github.com 拉 manifest ==="
T0=$(now_ms)
HTTP_CODE=$(curl -sSL -o "$TMP/origin.json" -w "%{http_code}" --max-time 30 "$ORIGIN" || echo "ERR")
T1=$(now_ms)
ORIGIN_MS=$((T1-T0))
ORIGIN_SIZE=$(wc -c < "$TMP/origin.json" | tr -d ' ')
ORIGIN_SHA=$(shasum -a 256 "$TMP/origin.json" | awk '{print $1}')
echo "  $ORIGIN -> HTTP $HTTP_CODE  ${ORIGIN_MS}ms  ${ORIGIN_SIZE}B  sha256=${ORIGIN_SHA:0:16}…"

if [ "$HTTP_CODE" != "200" ] || [ "$ORIGIN_SIZE" = "0" ]; then
  echo "::error:: 基线失败"; head -5 "$TMP/origin.json"; exit 1
fi

BUNDLE_URL=$(node -e "const j=JSON.parse(require('fs').readFileSync('$TMP/origin.json','utf8'));const p=j.platforms||{};const k=p['windows-x86_64']||p['darwin-aarch64']||p['linux-x86_64']||Object.values(p)[0];process.stdout.write(k.url||'')")
echo "  bundle URL: $BUNDLE_URL"

T0=$(now_ms)
curl -sSL -o "$TMP/origin.bin" -H "Range: bytes=0-1023" --max-time 30 "$BUNDLE_URL"
T1=$(now_ms)
ORIGIN_BIN_MS=$((T1-T0))
ORIGIN_BIN_SHA=$(shasum -a 256 "$TMP/origin.bin" | awk '{print $1}')
ORIGIN_BIN_SIZE=$(wc -c < "$TMP/origin.bin" | tr -d ' ')
echo "  bundle[0..1023] ${ORIGIN_BIN_SIZE}B  ${ORIGIN_BIN_MS}ms  sha256=${ORIGIN_BIN_SHA:0:16}…"
echo

echo "=== 2) 各 proxy 透传测试 ==="
printf "%-28s %-7s %-8s %-9s %-11s %-7s %-11s\n" "PROXY" "MFT_HC" "MFT_MS" "MFT_SZ" "MFT_OK" "BIN_MS" "BIN_OK"
OK_COUNT=0
for P in "${PROXIES[@]}"; do
  MURL="$P/$ORIGIN"
  T0=$(now_ms)
  HC=$(curl -sSL -o "$TMP/proxy.json" -w "%{http_code}" --max-time 20 "$MURL" 2>/dev/null || echo "ERR")
  T1=$(now_ms); MMS=$((T1-T0))
  SZ=$(wc -c < "$TMP/proxy.json" 2>/dev/null | tr -d ' ')
  SHA=$(shasum -a 256 "$TMP/proxy.json" 2>/dev/null | awk '{print $1}')
  [ "$SHA" = "$ORIGIN_SHA" ] && MOK="OK" || MOK="FAIL"

  BURL="$P/$BUNDLE_URL"
  T0=$(now_ms)
  curl -sSL -o "$TMP/proxy.bin" -H "Range: bytes=0-1023" --max-time 30 "$BURL" 2>/dev/null
  T1=$(now_ms); BMS=$((T1-T0))
  BSHA=$(shasum -a 256 "$TMP/proxy.bin" 2>/dev/null | awk '{print $1}')
  BSZ=$(wc -c < "$TMP/proxy.bin" 2>/dev/null | tr -d ' ')
  if [ "$BSHA" = "$ORIGIN_BIN_SHA" ]; then BOK="OK"; else BOK="FAIL(${BSZ}B)"; fi

  printf "%-28s %-7s %-8s %-9s %-11s %-7s %-11s\n" "$P" "$HC" "${MMS}ms" "${SZ}B" "$MOK" "${BMS}ms" "$BOK"
  if [ "$MOK" = "OK" ] && [ "$BOK" = "OK" ]; then OK_COUNT=$((OK_COUNT+1)); fi
done

echo
if [ $OK_COUNT -eq 0 ]; then
  echo "::error:: 没有任何 proxy 同时通过 manifest+bundle 透传测试"
  exit 1
fi
exit 0
