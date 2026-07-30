#!/usr/bin/env bash

set -euo pipefail

LABEL="tech.kyber.server"
PLIST="/Library/LaunchAgents/${LABEL}.plist"

log() { echo "[kyber-server-agent] $*"; }
die() { echo "[kyber-server-agent] ERROR: $*" >&2; exit 1; }

require_root() {
    [[ $EUID -eq 0 ]] || die "Must be run as root (use sudo)."
}

get_gui_uid() {
    local uid
    uid=$(scutil <<< "show State:/Users/ConsoleUser" 2>/dev/null \
          | awk '/^ *UID *:/{print $3}')
    [[ -z "$uid" || "$uid" == "0" ]] && uid="${SUDO_UID:-}"
    [[ -z "$uid" || "$uid" == "0" ]] && uid=""
    echo "$uid"
}

require_gui_uid() {
    local uid
    uid=$(get_gui_uid)
    [[ -n "$uid" ]] || die "No logged-in GUI user found."
    echo "$uid"
}

is_bootstrapped() {
    launchctl print "gui/${1}/${LABEL}" &>/dev/null
}

get_app_path() {
    [[ -f "$PLIST" ]] || die "Plist not found: ${PLIST}"
    local binary
    binary=$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments:0" "$PLIST" 2>/dev/null) \
        || die "Could not read ProgramArguments from plist."
    dirname "$(dirname "$(dirname "$binary")")"
}

get_working_dir() {
    [[ -f "$PLIST" ]] || die "Plist not found: ${PLIST}"
    /usr/libexec/PlistBuddy -c "Print :WorkingDirectory" "$PLIST" 2>/dev/null \
        || die "Could not read WorkingDirectory from plist."
}

cmd_enable() {
    require_root
    [[ -f "$PLIST" ]] || die "Plist not found: ${PLIST}"
    local binary
    binary=$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments:0" "$PLIST")
    [[ -f "$binary" ]] || die "Binary not found: ${binary}"
    [[ -x "$binary" ]] || die "Binary not executable: ${binary}"

    local uid
    uid=$(require_gui_uid)

    if is_bootstrapped "$uid"; then
        log "Already bootstrapped — re-bootstrapping."
        launchctl bootout "gui/${uid}/${LABEL}" 2>/dev/null || true
    fi

    launchctl bootstrap "gui/${uid}" "$PLIST" || die "bootstrap failed."
    log "Agent enabled."
}

cmd_disable() {
    require_root
    local uid
    uid=$(require_gui_uid)

    is_bootstrapped "$uid" || { log "Agent not loaded — nothing to do."; exit 0; }
    launchctl bootout "gui/${uid}/${LABEL}" 2>/dev/null || die "bootout failed."
    log "Agent disabled. Plist remains at ${PLIST}."
}

cmd_uninstall() {
    require_root
    [[ -f "$PLIST" ]] || die "Plist not found: ${PLIST}"

    local uid app working_dir
    uid=$(get_gui_uid)
    app=$(get_app_path)
    working_dir=$(get_working_dir)

    if [[ -n "$uid" ]] && is_bootstrapped "$uid"; then
        launchctl bootout "gui/${uid}/${LABEL}" 2>/dev/null || true
    fi

    # macOS TCC database retains stale permission entries across reinstalls,
    # causing new permission grants to be silently ignored. Resetting forces a clean prompt.
    tccutil reset All ${LABEL} || true

    rm -Rf "$app"
    log "App bundle removed: ${app}"

    rm -f "$PLIST"
    log "Plist removed. Logs and working directory left at ${working_dir}."

    pkgutil --forget "${LABEL}" 2>/dev/null || true
}

cmd_start() {
    require_root
    local uid
    uid=$(require_gui_uid)

    is_bootstrapped "$uid" || die "Agent not bootstrapped. Run 'enable' first."
    launchctl kickstart -k "gui/${uid}/${LABEL}" || die "kickstart failed."
    log "Agent started."
}

cmd_stop() {
    require_root
    local uid
    uid=$(require_gui_uid)

    launchctl kill SIGTERM "gui/${uid}/${LABEL}" 2>/dev/null || log "Agent may not be running."
    log "Stop signal sent."
}

cmd_restart() {
    require_root
    local uid
    uid=$(require_gui_uid)

    is_bootstrapped "$uid" || die "Agent not bootstrapped. Run 'enable' first."
    launchctl kickstart -k "gui/${uid}/${LABEL}" || die "kickstart failed."
    log "Agent restarted."
}

cmd_status() {
    local uid
    uid=$(get_gui_uid)

    local binary="" working_dir=""
    if [[ -f "$PLIST" ]]; then
        binary=$(/usr/libexec/PlistBuddy -c "Print :ProgramArguments:0" "$PLIST" 2>/dev/null || true)
        working_dir=$(/usr/libexec/PlistBuddy -c "Print :WorkingDirectory" "$PLIST" 2>/dev/null || true)
    fi

    echo "Plist   : $( [[ -f "$PLIST"    ]] && echo "installed" || echo "NOT found" ) (${PLIST})"
    echo "Binary  : $( [[ -n "$binary" && -x "$binary" ]] && echo "found" || echo "NOT found" ) (${binary:-n/a})"
    echo "Workdir : $( [[ -n "$working_dir" && -d "$working_dir" ]] && echo "exists" || echo "NOT found" ) (${working_dir:-n/a})"

    if [[ -z "$uid" ]]; then
        echo "Runtime : no active GUI session"
        return
    fi

    if ! is_bootstrapped "$uid"; then
        echo "Runtime : not bootstrapped (uid ${uid})"
        return
    fi

    local list_line pid exit_code
    list_line=$(launchctl list 2>/dev/null \
                | grep -E "^[[:space:]]*[0-9-]+[[:space:]]+[0-9-]+[[:space:]]+${LABEL}$" \
                || true)

    if [[ -z "$list_line" ]]; then
        echo "Runtime : bootstrapped, not yet listed"
    else
        pid=$(echo "$list_line"       | awk '{print $1}')
        exit_code=$(echo "$list_line" | awk '{print $2}')
        if [[ "$pid" == "-" ]]; then
            echo "Runtime : stopped (last exit: ${exit_code})"
        else
            echo "Runtime : running (pid: ${pid}, last exit: ${exit_code})"
        fi
    fi

    if [[ -n "$working_dir" ]]; then
        for f in "${working_dir}/out.log" "${working_dir}/err.log"; do
            [[ -f "$f" && -s "$f" ]] || continue
            echo ""
            echo "--- $(basename "$f") (last 10 lines) ---"
            tail -n 10 "$f"
        done
    fi
}

cmd_help() {
    echo "Usage: $(basename "$0") <command>"
    echo ""
    echo "  enable     Bootstrap the agent into the current GUI session"
    echo "  disable    Bootout the agent (plist kept, reloads at next login)"
    echo "  uninstall  Bootout the agent, remove the app bundle and plist"
    echo "  start      Kickstart the agent (must be bootstrapped)"
    echo "  stop       Send SIGTERM to the agent"
    echo "  restart    Kickstart the agent, killing any running instance"
    echo "  status     Show install and runtime state"
}

case "${1:-help}" in
    enable)         cmd_enable    ;;
    disable)        cmd_disable   ;;
    uninstall)      cmd_uninstall ;;
    start)          cmd_start     ;;
    stop)           cmd_stop      ;;
    restart)        cmd_restart   ;;
    status)         cmd_status    ;;
    help|-h|--help) cmd_help      ;;
    *) echo "Unknown command: $1" >&2; cmd_help; exit 1 ;;
esac
