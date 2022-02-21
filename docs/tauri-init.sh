#!/usr/bin/env sh
# shellcheck shell=dash

set -u

# we expect to be passed the version as the first argument
if [ $# -eq 0 ]; then
  echo "no version argument provided, this is supposed to be the git tag of the release"
  exit 1
fi

# take the first argument as the tag
TAG="$1"
shift
if ! echo "$TAG" | grep -q "^v[0-9]*\.[0-9]*\.[0-9]*"; then
  echo "version provided does not match expected format v<major>.<minor>.<patch>"
  exit 1
fi

# handle all other arguments to this shell script (none so far)
if [ $# -gt 0 ]; then
  while true; do
    case "$1" in
    --)
      shift
      break
      ;;

    *)
      echo "argument not supported: $1" >&2
      exit 1
      ;;

    esac
  done
fi

# allow specifying forks easily, only need gh user/repo because this is currently attached to GitHub Releases anyways
REPO="${TAURI_INIT_REPO:-chippers/tauri-init}"

main() {
  need_cmd uname
  need_cmd mktemp
  need_cmd chmod
  need_cmd mkdir
  need_cmd rm
  need_cmd rmdir

  get_arch || return 1
  local _arch="$RETVAL"
  assert_nz "$_arch" "arch"

  local _ext=""
  case "$_arch" in
  *windows*)
    _ext=".exe"
    ;;
  esac

  local _url="https://github.com/$REPO/releases/download/$TAG/tauri-init_${_arch}${_ext}"

  local _dir
  _dir="$(ensure mktemp -d)"
  local _file="${_dir}/tauri-init${_ext}"

  local _ansi_escapes_are_valid=false
  if [ -t 2 ]; then
    if [ "${TERM+set}" = 'set' ]; then
      case "$TERM" in
      xterm* | rxvt* | urxvt* | linux* | vt*)
        _ansi_escapes_are_valid=true
        ;;
      esac
    fi
  fi

  if $_ansi_escapes_are_valid; then
    printf "\33[1minfo:\33[0m downloading installer\n" 1>&2
  else
    printf '%s\n' 'info: downloading installer' 1>&2
  fi

  ensure mkdir -p "$_dir"
  download "$_url" "$_file"
  ensure chmod u+x "$_file"
  if [ ! -x "$_file" ]; then
    printf '%s\n' "Cannot execute $_file (likely because of mounting /tmp as noexec)." 1>&2
    printf '%s\n' "Please copy the file to a location where you can execute binaries and run ./tauri-init${_ext}." 1>&2
    exit 1
  fi

  ignore "$_file" "$@"
  local _retval=$?

  ignore rm "$_file"
  ignore rmdir "$_dir"

  return "$_retval"
}

get_arch() {
  local _os _cpu _arch
  _os=$(uname -s)
  _cpu=$(uname -m)

  case "$_os" in
  Linux)
    _os=unknown-linux-gnu
    ;;

  FreeBSD)
    _os=unknown-freebsd
    ;;

  NetBSD)
    _os=unknown-netbsd
    ;;

  DragonFly)
    _os=unknown-dragonfly
    ;;

  Darwin)
    _os=apple-darwin
    ;;

  MINGW* | MSYS* | CYGWIN*)
    _os=pc-windows-gnu
    ;;

  *)
    err "unrecognized OS: $_os"
    ;;
  esac

  case "$_cpu" in
  aarch64 | arm64)
    _cpu=aarch64
    ;;

  x86_64 | x86-64 | x64 | amd64)
    _cpu=x86_64
    ;;

  *)
    err "unsupported CPU: $_cpu"
    ;;

  esac

  _arch="${_cpu}-${_os}"
  RETVAL="$_arch"
}

say() {
  printf 'tauri-init: %s\n' "$1"
}

err() {
  say "$1" >&2
  exit 1
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    err "need '$1' (command not found)"
  fi
}

assert_nz() {
  if [ -z "$1" ]; then err "assert_nz $2"; fi
}

# Run a command that should never fail. If the command fails execution
# will immediately terminate with an error showing the failing
# command.
ensure() {
  if ! "$@"; then err "command failed: $*"; fi
}

# This is just for indicating that commands' results are being
# intentionally ignored. Usually, because it's being executed
# as part of error handling.
ignore() {
  "$@"
}

download() {
  local _status _err
  get_ciphersuites
  _ciphersuites="$RETVAL"

  if [ -n "$_ciphersuites" ]; then
    _err=$(curl --retry 3 --proto '=https' --tlsv1.2 --ciphers "$_ciphersuites" --silent --show-error --fail --location "$1" --output "$2" 2>&1)
    _status=$?
  else
    echo "Warning: Not enforcing strong cipher suites for TLS, this is potentially less secure"
    _err=$(curl --retry 3 --silent --show-error --fail --location "$1" --output "$2" 2>&1)
    _status=$?
  fi

  if [ -n "$_err" ]; then
    echo "$_err" >&2
    if echo "$_err" | grep -q 404$; then
      err "installer not found at %s"
    fi
  fi

  return $_status
}

# Adapted and slightly simplified from Rustup. We only use curl and support more modern operating systems.
#
# Return cipher suite string specified by user, otherwise return strong TLS 1.2-1.3 cipher suites
# if support by local tools is detected. Detection currently supports these curl backends:
# GnuTLS and OpenSSL (possibly also LibreSSL and BoringSSL). Return value can be empty.
get_ciphersuites() {
  if [ -n "${TAURI_INIT_TLS_CIPHERSUITES-}" ]; then
    # user specified custom cipher suites, assume they know what they're doing
    RETVAL="$TAURI_INIT_TLS_CIPHERSUITES"
    return
  fi

  local _openssl_syntax="no"
  local _gnutls_syntax="no"
  local _backend_supported="yes"
  if curl -V | grep -q ' OpenSSL/'; then
    _openssl_syntax="yes"
  elif curl -V | grep -iq ' LibreSSL/'; then
    _openssl_syntax="yes"
  elif curl -V | grep -iq ' BoringSSL/'; then
    _openssl_syntax="yes"
  elif curl -V | grep -iq ' GnuTLS/'; then
    _gnutls_syntax="yes"
  else
    _backend_supported="no"
  fi

  # Return strong TLS 1.2-1.3 cipher suites in OpenSSL or GnuTLS syntax. TLS 1.2
  # excludes non-ECDHE and non-AEAD cipher suites. DHE is excluded due to bad
  # DH params often found on servers (see RFC 7919). Sequence matches or is
  # similar to Firefox 68 ESR with weak cipher suites disabled via about:config.
  # $1 must be openssl or gnutls.
  local _cs=""
  if [ "$_backend_supported" = "yes" ]; then

    if [ "$_openssl_syntax" = "yes" ]; then
      # OpenSSL is forgiving of unknown values, no problems with TLS 1.3 values on versions that don't support it yet.
      _cs="TLS_AES_128_GCM_SHA256:TLS_CHACHA20_POLY1305_SHA256:TLS_AES_256_GCM_SHA384:ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384"

    elif [ "$_gnutls_syntax" = "yes" ]; then
      # GnuTLS isn't forgiving of unknown values, so this may require a GnuTLS version that supports TLS 1.3 even if wget doesn't.
      # Begin with SECURE128 (and higher) then remove/add to build cipher suites. Produces same 9 cipher suites as OpenSSL but in slightly different order.
      _cs="SECURE128:-VERS-SSL3.0:-VERS-TLS1.0:-VERS-TLS1.1:-VERS-DTLS-ALL:-CIPHER-ALL:-MAC-ALL:-KX-ALL:+AEAD:+ECDHE-ECDSA:+ECDHE-RSA:+AES-128-GCM:+CHACHA20-POLY1305:+AES-256-GCM"
    fi
  fi

  RETVAL="$_cs"
}

main "$@" || exit 1
