#!/bin/sh
set -eu

: "${OUT_DIR:=out}"
CONFIG="${1:-${OUT_DIR}/device/pinephone/kernel.config}"
REQUIREMENTS="kernel/device-requirements.txt"

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${REPO_ROOT}"

if [ ! -f "${CONFIG}" ]; then
  echo "kernel config not found: ${CONFIG} (run scripts/fetch-pm-kernel.sh first)"
  exit 1
fi

get_config_value() {
  name=$1
  value="$(grep -E "^${name}=" "${CONFIG}" | head -1 | cut -d= -f2)"
  if [ -z "${value}" ]; then
    if grep -qE "^# ${name} is not set" "${CONFIG}"; then
      printf 'n\n'
    else
      printf 'missing\n'
    fi
  else
    printf '%s\n' "${value}"
  fi
}

errors=0
warnings=0
while read -r level spec; do
  [ -n "${level}" ] || continue
  name="${spec%%=*}"
  allowed="${spec#*=}"
  value="$(get_config_value "${name}")"
  ok=0
  for want in $(printf '%s\n' "${allowed}" | tr '|' ' '); do
    if [ "${value}" = "${want}" ]; then
      ok=1
      break
    fi
  done
  if [ "${ok}" != "1" ]; then
    if [ "${level}" = "REQUIRED" ]; then
      echo "FAIL ${spec} (kernel: ${value})"
      errors=$((errors + 1))
    else
      echo "WARN ${spec} (kernel: ${value})"
      warnings=$((warnings + 1))
    fi
  fi
done < "${REQUIREMENTS}"

total=$(grep -cE "^(REQUIRED|PREFERRED) " "${REQUIREMENTS}")
echo "config check: ${total} requirements, ${errors} failed, ${warnings} warnings"
if [ "${errors}" != "0" ]; then
  exit 1
fi
echo "device kernel satisfies Afflictio userspace requirements"
