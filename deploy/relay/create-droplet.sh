#!/usr/bin/env bash
# Creates one relay droplet per region with doctl, each zero-config:
#
#   deploy/relay/create-droplet.sh nyc3 fra1 sgp1
#
# Needs `doctl auth init` first. Optional environment:
#   RELAY_SIZE     droplet size (default s-1vcpu-1gb)
#   RELAY_SSH_KEYS comma-separated SSH key ids or fingerprints from
#                  `doctl compute ssh-key list` (default: none, console only)
#   RELAY_DOMAIN_SUFFIX  e.g. relay.example.org gives nyc3.relay.example.org;
#                  point those names at the droplets yourself. Without it,
#                  each relay is served at IP-WITH-DASHES.sslip.io.
#   RELAY_OPEN=1   let any computer listen. By default each relay gets its
#                  own listen token, printed at the end, and only computers
#                  given it may listen; phones never need it.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
if [ "$#" -eq 0 ]; then
  echo "usage: $0 REGION [REGION...]" >&2
  exit 2
fi
command -v doctl >/dev/null || { echo "doctl is not installed" >&2; exit 1; }

size=${RELAY_SIZE:-s-1vcpu-1gb}
compose=$(base64 < "$here/compose.yml" | tr -d '\n')
caddyfile=$(base64 < "$here/Caddyfile" | tr -d '\n')
data=$(mktemp)
trap 'rm -f "$data"' EXIT

# 32 random bytes, base64url without padding: 43 characters that need no
# escaping in YAML, sed, or a URL.
listen_token() {
  head -c 32 /dev/urandom | base64 | tr '+/' '-_' | tr -d '=\n'
}

# Percent-encodes $1 for a URL's query.
encoded() {
  local text=$1 out="" char i
  for ((i = 0; i < ${#text}; i++)); do
    char=${text:i:1}
    case $char in
      [A-Za-z0-9._~-]) out+=$char ;;
      *) out+=$(printf '%%%02X' "'$char") ;;
    esac
  done
  printf '%s' "$out"
}

# This run's droplets, by id, with each one's region and listen token.
ids=()
regions=()
tokens=()

for region in "$@"; do
  domain=""
  if [ -n "${RELAY_DOMAIN_SUFFIX:-}" ]; then
    domain="$region.$RELAY_DOMAIN_SUFFIX"
  fi
  token=open
  if [ "${RELAY_OPEN:-}" != 1 ]; then
    token=$(listen_token)
  fi
  # The token is filled in by the shell, not sed, so it is never in a
  # process's arguments; it has no characters a replacement treats
  # specially.
  rendered=$(sed -e "s|__COMPOSE_B64__|$compose|" \
      -e "s|__CADDYFILE_B64__|$caddyfile|" \
      -e "s|__RELAY_DOMAIN__|$domain|" \
      "$here/cloud-init.yml")
  printf '%s\n' "${rendered//__RELAY_LISTEN_TOKEN__/$token}" > "$data"
  keys=()
  if [ -n "${RELAY_SSH_KEYS:-}" ]; then
    keys=(--ssh-keys "$RELAY_SSH_KEYS")
  fi
  created=$(doctl compute droplet create "sheprd-relay-$region" \
    --region "$region" \
    --size "$size" \
    --image ubuntu-24-04-x64 \
    --enable-monitoring \
    --tag-name sheprd-relay \
    --user-data-file "$data" \
    "${keys[@]}" \
    --wait \
    --format ID,Name,PublicIPv4,Region)
  printf '%s\n' "$created"
  ids+=("$(awk 'NR == 2 { print $1 }' <<<"$created")")
  regions+=("$region")
  tokens+=("$token")
done

cat <<'NOTE'

Each droplet starts its relay within a few minutes. The dashboard login and
the listen token are in /root/sheprd-relay-login.txt on each droplet
(`doctl compute ssh NAME`, or the web console).

Add each relay in Sheprd, under Settings > Remote control > Relays, with its
listen token when Settings asks for it, or open its link on the computer
running Sheprd:
NOTE

for i in "${!ids[@]}"; do
  region=${regions[i]}
  token=${tokens[i]}
  # The address as the droplet serves it: the certificate is for the name
  # with dashes, so the IP with dots would be refused.
  if [ -n "${RELAY_DOMAIN_SUFFIX:-}" ]; then
    address="https://$region.$RELAY_DOMAIN_SUFFIX"
  else
    ip=$(doctl compute droplet get "${ids[i]}" --format PublicIPv4 --no-header 2>/dev/null) || ip=""
    if [ -z "$ip" ]; then
      echo "  $region: https://IP-WITH-DASHES.sslip.io (its address could not be read)"
      address=""
    else
      address="https://${ip//./-}.sslip.io"
    fi
  fi
  echo
  echo "  $region"
  if [ -n "$address" ]; then
    echo "    address:       $address"
  fi
  if [ "$token" = open ]; then
    echo "    open: any computer may listen"
    continue
  fi
  echo "    listen token:  $token"
  if [ -n "$address" ]; then
    echo "    add to Sheprd: sheprd://relay?endpoint=$(encoded "$address")&token=$token"
  fi
done
if [ -n "${RELAY_DOMAIN_SUFFIX:-}" ]; then
  echo
  echo "Point each name at its droplet's address above first."
fi
